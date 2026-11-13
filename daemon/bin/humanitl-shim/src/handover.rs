//! The descriptors through a socket instead of inheritance (HUM-434).
//!
//! When the container engine's client runs as a transient unit of the
//! user's service manager, it is no child of the daemon and inherits nothing
//! from it. The daemon then binds one `SOCK_SEQPACKET` socket per session
//! and mounts it read-only into the sandbox; `--handover <path>` names it
//! and `--handover-token <hex>` the one-time token. Before anything else of
//! the start the shim connects, sends the token as one message, and gets
//! every descriptor in one reply: one byte with their count and as many
//! `SCM_RIGHTS` descriptors. It lays them on 3, 4, 5, ... in that order
//! ([`place`]), exactly where `--preserve-fds` used to put them, so the
//! rest of the start (report, environment memfd, lifeline, mount table,
//! shell door, terminal) reads them as before.
//!
//! The daemon accepts exactly one connection and closes its listening
//! socket right after the `accept`, so a second connect is refused. When
//! the daemon is gone before the handover, nobody listens: the connect is
//! refused or the reply is empty, and the shim exits 126 without starting
//! the agent. The daemon removes the socket file before it sends anything
//! and the sandbox sees only its directory, so after the handover there is
//! no second socket in the sandbox; `single_socket` stays strict
//! (`linux.rs`).

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// The most descriptors one handover carries; the daemon's limit.
pub const MAX_FDS: usize = 8;

/// The token's length: 32 hex digits.
pub const TOKEN_LEN: usize = 32;

/// How long the shim waits for the daemon's reply, in seconds. Kept as an
/// `i32` and widened where the `timeval` is built: `libc::time_t` is
/// deprecated on musl, where the shim ships (HUM-012).
const REPLY_WAIT_SECS: i32 = 30;

/// Received descriptors are first moved at or above this number, so that
/// laying them on 3, 4, 5, ... never overwrites one not yet laid.
const HIGH_FD: RawFd = 64;

/// Connects to `path`, names `token`, and takes the descriptors.
///
/// # Errors
///
/// A sentence for the report: the socket refused or is missing (the daemon
/// is gone or the handover already happened), the reply was empty (wrong
/// token, another uid) or malformed.
pub fn receive(path: &Path, token: &str) -> Result<Vec<OwnedFd>, String> {
    if token.len() != TOKEN_LEN || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("handover-token-malformed".to_owned());
    }
    let socket = connect(path)?;
    receive_on(&socket, token)
}

/// The exchange on a connected socket; [`receive`] without the connect.
///
/// # Errors
///
/// As [`receive`].
pub fn receive_on(socket: &OwnedFd, token: &str) -> Result<Vec<OwnedFd>, String> {
    let timeout = libc::timeval {
        tv_sec: REPLY_WAIT_SECS.into(),
        tv_usec: 0,
    };
    // SAFETY: setsockopt with a pointer to a timeval and its size.
    unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            (&raw const timeout).cast(),
            socklen_of::<libc::timeval>(),
        );
    }
    // SAFETY: send of a buffer of its own length.
    let sent = unsafe {
        libc::send(
            socket.as_raw_fd(),
            token.as_ptr().cast(),
            token.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    if sent < 0 || sent.cast_unsigned() != token.len() {
        return Err(format!(
            "handover-send-failed:{}",
            io::Error::last_os_error().raw_os_error().unwrap_or(0)
        ));
    }
    let mut count = [0u8; 1];
    let mut iov = libc::iovec {
        iov_base: count.as_mut_ptr().cast(),
        iov_len: count.len(),
    };
    // Room for MAX_FDS descriptors, aligned for cmsghdr.
    let mut control = [0u64; 16];
    // SAFETY: CMSG_SPACE is a pure computation.
    let space = unsafe { libc::CMSG_SPACE(u32::try_from(MAX_FDS * 4).unwrap_or(u32::MAX)) };
    debug_assert!(space as usize <= std::mem::size_of_val(&control));
    // SAFETY: plain data; every field is set below or zero.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &raw mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    // `socklen_t` on musl, `usize` on glibc: the same conversion as in
    // `channel.rs`.
    // `socklen_t` on musl, `usize` on glibc.
    #[allow(clippy::useless_conversion)]
    {
        message.msg_controllen = space.try_into().unwrap_or_default();
    }
    // SAFETY: recvmsg into the buffers just described.
    let received =
        unsafe { libc::recvmsg(socket.as_raw_fd(), &raw mut message, libc::MSG_CMSG_CLOEXEC) };
    if received < 0 {
        return Err(format!(
            "handover-no-reply:{}",
            io::Error::last_os_error().raw_os_error().unwrap_or(0)
        ));
    }
    let fds = collect_rights(&message);
    if received == 0 {
        return Err("handover-refused:the_daemon_closed_without_descriptors".to_owned());
    }
    if message.msg_flags & libc::MSG_CTRUNC != 0 {
        return Err("handover-truncated".to_owned());
    }
    if received != 1 || usize::from(count[0]) != fds.len() || fds.len() > MAX_FDS {
        return Err(format!(
            "handover-malformed:count={}:received={}",
            count[0],
            fds.len()
        ));
    }
    Ok(fds)
}

/// Every descriptor in the `SCM_RIGHTS` messages of `message`, owned.
fn collect_rights(message: &libc::msghdr) -> Vec<OwnedFd> {
    let mut fds = Vec::new();
    // SAFETY: walking the control buffer recvmsg filled, with the macros
    // made for it; every descriptor in it is new in this process.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(message);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                // Read with `read_unaligned` below, so the alignment of the
                // cast pointer never matters.
                #[allow(clippy::cast_ptr_alignment)]
                let data = libc::CMSG_DATA(header).cast::<RawFd>();
                let bytes = (*header).cmsg_len as usize - libc::CMSG_LEN(0) as usize;
                for index in 0..bytes / std::mem::size_of::<RawFd>() {
                    let fd = data.add(index).read_unaligned();
                    fds.push(OwnedFd::from_raw_fd(fd));
                }
            }
            header = libc::CMSG_NXTHDR(message, header);
        }
    }
    fds
}

/// Lays `fds` on 3, 4, 5, ... in their order, without `CLOEXEC`, as if
/// inherited; the received numbers are closed. Must run single-threaded,
/// before anything else of the start opens a descriptor.
///
/// # Errors
///
/// The failed `fcntl` or `dup2`, as a sentence for the message.
pub fn place(fds: Vec<OwnedFd>) -> Result<(), String> {
    // First every one above the range, so that no target is a source that
    // still waits for its turn.
    let mut high = Vec::with_capacity(fds.len());
    for fd in &fds {
        // SAFETY: F_DUPFD_CLOEXEC duplicates an open descriptor.
        let copy = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, HIGH_FD) };
        if copy < 0 {
            return Err(format!("handover-dup:{}", io::Error::last_os_error()));
        }
        // SAFETY: fcntl just returned this new descriptor.
        high.push(unsafe { OwnedFd::from_raw_fd(copy) });
    }
    drop(fds);
    for (target, fd) in (3..).zip(&high) {
        // SAFETY: dup2 between two descriptor numbers this process owns;
        // the target loses CLOEXEC, as an inherited descriptor has none.
        if unsafe { libc::dup2(fd.as_raw_fd(), target) } < 0 {
            return Err(format!(
                "handover-dup2:{}->{target}:{}",
                fd.as_raw_fd(),
                io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

/// A `SOCK_SEQPACKET` socket connected to `path`, above the range the
/// handover lays its descriptors on.
fn connect(path: &Path) -> Result<OwnedFd, String> {
    let bytes = path.as_os_str().as_bytes();
    // SAFETY: plain data; every byte is set below or zero.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
        return Err("handover-path-too-long".to_owned());
    }
    address.sun_family = libc::sa_family_t::try_from(libc::AF_UNIX).unwrap_or(0);
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = byte.cast_signed();
    }
    // SAFETY: socket(2) with constant arguments.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    if raw < 0 {
        return Err(format!(
            "handover-socket:{}",
            io::Error::last_os_error().raw_os_error().unwrap_or(0)
        ));
    }
    // SAFETY: socket just returned it.
    let low = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: F_DUPFD_CLOEXEC on the descriptor just made.
    let moved = unsafe { libc::fcntl(low.as_raw_fd(), libc::F_DUPFD_CLOEXEC, HIGH_FD) };
    if moved < 0 {
        return Err(format!(
            "handover-socket:{}",
            io::Error::last_os_error().raw_os_error().unwrap_or(0)
        ));
    }
    drop(low);
    // SAFETY: fcntl just returned it.
    let socket = unsafe { OwnedFd::from_raw_fd(moved) };
    // SAFETY: connect with a sockaddr_un and its size.
    let rc = unsafe {
        libc::connect(
            socket.as_raw_fd(),
            (&raw const address).cast(),
            socklen_of::<libc::sockaddr_un>(),
        )
    };
    if rc < 0 {
        return Err(format!(
            "handover-connect-failed:{}",
            io::Error::last_os_error().raw_os_error().unwrap_or(0)
        ));
    }
    Ok(socket)
}

fn socklen_of<T>() -> libc::socklen_t {
    libc::socklen_t::try_from(std::mem::size_of::<T>()).unwrap_or(libc::socklen_t::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::{Read as _, Write as _};

    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn pair() -> (OwnedFd, OwnedFd) {
        let mut fds = [0; 2];
        // SAFETY: socketpair into a two-element array.
        let rc = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        };
        assert_eq!(rc, 0);
        // SAFETY: socketpair just returned both.
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
    }

    /// The daemon's half: reads the token, then replies with `count` and
    /// `fds`.
    fn reply(daemon: &OwnedFd, count: u8, fds: &[RawFd]) -> Vec<u8> {
        let mut token = [0u8; 64];
        // SAFETY: recv into a buffer of its own length.
        let n = unsafe {
            libc::recv(
                daemon.as_raw_fd(),
                token.as_mut_ptr().cast(),
                token.len(),
                0,
            )
        };
        assert!(n > 0);
        let mut payload = [count];
        let mut iov = libc::iovec {
            iov_base: payload.as_mut_ptr().cast(),
            iov_len: 1,
        };
        let mut control = [0u64; 16];
        // SAFETY: plain data.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &raw mut iov;
        message.msg_iovlen = 1;
        if !fds.is_empty() {
            let bytes = u32::try_from(std::mem::size_of_val(fds)).unwrap();
            // SAFETY: building one SCM_RIGHTS message in a buffer large
            // enough for it, with the macros made for it.
            unsafe {
                message.msg_control = control.as_mut_ptr().cast();
                // `socklen_t` on musl, `usize` on glibc.
                #[allow(clippy::useless_conversion)]
                {
                    message.msg_controllen = libc::CMSG_SPACE(bytes).try_into().unwrap_or_default();
                }
                let header = libc::CMSG_FIRSTHDR(&raw const message);
                (*header).cmsg_level = libc::SOL_SOCKET;
                (*header).cmsg_type = libc::SCM_RIGHTS;
                // `socklen_t` on musl, `usize` on glibc.
                #[allow(clippy::useless_conversion)]
                {
                    (*header).cmsg_len = libc::CMSG_LEN(bytes).try_into().unwrap();
                }
                std::ptr::copy_nonoverlapping(
                    fds.as_ptr().cast::<u8>(),
                    libc::CMSG_DATA(header),
                    std::mem::size_of_val(fds),
                );
            }
        }
        // SAFETY: sendmsg with the message just built.
        let sent = unsafe { libc::sendmsg(daemon.as_raw_fd(), &raw const message, 0) };
        assert_eq!(sent, 1);
        token[..usize::try_from(n).unwrap()].to_vec()
    }

    #[test]
    fn the_token_goes_out_and_the_descriptors_come_back_in_order() {
        let (shim, daemon) = pair();
        let (mut read_a, write_a) = std::io::pipe().unwrap();
        let (mut read_b, write_b) = std::io::pipe().unwrap();
        let server = std::thread::spawn(move || {
            let sent = [write_a.as_raw_fd(), write_b.as_raw_fd()];
            let token = reply(&daemon, 2, &sent);
            drop((write_a, write_b));
            token
        });
        let fds = receive_on(&shim, TOKEN).unwrap();
        assert_eq!(server.join().unwrap(), TOKEN.as_bytes());
        assert_eq!(fds.len(), 2);
        let mut first = std::fs::File::from(fds.into_iter().next().unwrap());
        first.write_all(b"a").unwrap();
        drop(first);
        let mut text = String::new();
        read_a.read_to_string(&mut text).unwrap();
        assert_eq!(text, "a", "the first descriptor is the first sent");
        let mut rest = Vec::new();
        read_b.read_to_end(&mut rest).unwrap();
        assert!(rest.is_empty());
    }

    /// The daemon closing without descriptors (wrong token, another uid,
    /// or a daemon that died) refuses the start.
    #[test]
    fn an_empty_reply_refuses() {
        let (shim, daemon) = pair();
        let server = std::thread::spawn(move || {
            let mut token = [0u8; 64];
            // SAFETY: recv into a buffer of its own length.
            unsafe {
                libc::recv(daemon.as_raw_fd(), token.as_mut_ptr().cast(), 64, 0);
            }
            drop(daemon);
        });
        let err = receive_on(&shim, TOKEN).unwrap_err();
        server.join().unwrap();
        assert!(err.starts_with("handover-refused"), "{err}");
    }

    #[test]
    fn a_count_that_is_not_the_number_of_descriptors_refuses() {
        let (shim, daemon) = pair();
        let (_read, write) = std::io::pipe().unwrap();
        let server = std::thread::spawn(move || {
            reply(&daemon, 3, &[write.as_raw_fd()]);
        });
        let err = receive_on(&shim, TOKEN).unwrap_err();
        server.join().unwrap();
        assert!(err.starts_with("handover-malformed"), "{err}");
    }

    #[test]
    fn a_missing_socket_refuses() {
        let path = Path::new("/nonexistent/humanitl-handover-test");
        let err = receive(path, TOKEN).unwrap_err();
        assert!(err.starts_with("handover-connect-failed"), "{err}");
        assert!(receive(path, "short").unwrap_err().contains("malformed"));
    }
}
