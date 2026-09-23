//! The channel between the shim's child and its parent (HUM-138).
//!
//! A `SOCK_SEQPACKET` pair made right before the fork. The child, which
//! installs the agent's filter, sends over it first the listener of that
//! filter (`SCM_RIGHTS`) or the errno with which the kernel refused one, and
//! later a single byte once its own probes are over and the agent is next.
//! The parent receives the first message on its main thread and checks for
//! the second from the thread that serves the listener (`refusals::serve`).
//!
//! Every function the child calls here allocates nothing: the child of a
//! process may call them between `fork` and `exec`.

use std::ffi::c_int;
use std::io;
use std::mem::zeroed;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;

/// The child has a listener; the descriptor rides along.
const MARK_LISTENER: u8 = b'L';
/// The child has no listener; four bytes of errno follow.
const MARK_UNAVAILABLE: u8 = b'E';
/// The child's probes are over; the agent is next.
const MARK_EXEC: u8 = b'X';

/// What the parent learns from the child's first message.
#[derive(Debug)]
pub enum Received {
    /// The agent's filter parks refused calls; this is its listener.
    Listener(OwnedFd),
    /// The agent's filter answers `EPERM` itself; the errno says why the
    /// listener was refused.
    Unavailable(i32),
    /// The child ended before it said either.
    Closed,
}

/// A `SOCK_SEQPACKET` pair with `CLOEXEC`: `(parent's end, child's end)`.
///
/// # Errors
///
/// `socketpair(2)` failed.
pub fn channel() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as c_int; 2];
    // SAFETY: `fds` is a valid two-element array for socketpair.
    let rc = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both descriptors were just created and belong to nobody else.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Control buffer for one descriptor, aligned for `cmsghdr`.
#[repr(C, align(8))]
struct CmsgBuf([u8; 64]);

/// Child: hands `listener` to the parent. Allocates nothing.
pub fn send_listener(control: RawFd, listener: RawFd) -> bool {
    let mut payload = [MARK_LISTENER];
    let mut iov = libc::iovec {
        iov_base: payload.as_mut_ptr().cast(),
        iov_len: payload.len(),
    };
    let mut buf = CmsgBuf([0; 64]);
    // SAFETY: CMSG_SPACE is arithmetic on a constant.
    let space = unsafe { libc::CMSG_SPACE(4) };
    // SAFETY: plain data.
    let mut msg: libc::msghdr = unsafe { zeroed() };
    msg.msg_iov = &raw mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = buf.0.as_mut_ptr().cast();
    msg.msg_controllen = space.try_into().unwrap_or_default();
    // SAFETY: `msg` describes `buf`, which has room for one header and one
    // descriptor; CMSG_FIRSTHDR points into it.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&raw const msg);
        if cmsg.is_null() {
            return false;
        }
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(4).try_into().unwrap_or_default();
        ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<c_int>(), listener);
        libc::sendmsg(control, &raw const msg, libc::MSG_NOSIGNAL) == 1
    }
}

/// Child: says that the kernel refused the listener with `errno`.
pub fn send_unavailable(control: RawFd, errno: i32) {
    let bytes = errno.to_le_bytes();
    let message = [MARK_UNAVAILABLE, bytes[0], bytes[1], bytes[2], bytes[3]];
    send_bytes(control, &message);
}

/// Child: says that the probes are over and the agent comes next.
pub fn send_exec_marker(control: RawFd) {
    send_bytes(control, &[MARK_EXEC]);
}

fn send_bytes(control: RawFd, bytes: &[u8]) {
    // SAFETY: the pointer and length describe `bytes`.
    unsafe {
        libc::send(
            control,
            bytes.as_ptr().cast(),
            bytes.len(),
            libc::MSG_NOSIGNAL,
        );
    }
}

/// Parent: the child's first message, waiting for it.
pub fn receive(control: &OwnedFd) -> Received {
    let mut payload = [0u8; 8];
    let mut iov = libc::iovec {
        iov_base: payload.as_mut_ptr().cast(),
        iov_len: payload.len(),
    };
    let mut buf = CmsgBuf([0; 64]);
    loop {
        // SAFETY: plain data.
        let mut msg: libc::msghdr = unsafe { zeroed() };
        msg.msg_iov = &raw mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = buf.0.as_mut_ptr().cast();
        // `usize` with glibc, `socklen_t` with musl: the conversion is a no-op
        // on one target and needed on the other.
        #[allow(clippy::useless_conversion)]
        {
            msg.msg_controllen = buf.0.len().try_into().unwrap_or_default();
        }
        // SAFETY: `msg` describes `payload` and `buf`, both valid for writes.
        let n = unsafe { libc::recvmsg(control.as_raw_fd(), &raw mut msg, libc::MSG_CMSG_CLOEXEC) };
        if n < 0 {
            if errno() == libc::EINTR {
                continue;
            }
            return Received::Closed;
        }
        if n == 0 {
            return Received::Closed;
        }
        return match payload[0] {
            MARK_LISTENER => received_descriptor(&msg).map_or(Received::Closed, Received::Listener),
            MARK_UNAVAILABLE if n >= 5 => Received::Unavailable(i32::from_le_bytes([
                payload[1], payload[2], payload[3], payload[4],
            ])),
            _ => Received::Closed,
        };
    }
}

fn received_descriptor(msg: &libc::msghdr) -> Option<OwnedFd> {
    // SAFETY: `msg` was filled by recvmsg; the macros walk its control buffer
    // within `msg_controllen`.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(msg);
        if cmsg.is_null()
            || (*cmsg).cmsg_level != libc::SOL_SOCKET
            || (*cmsg).cmsg_type != libc::SCM_RIGHTS
        {
            return None;
        }
        let fd = ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<c_int>());
        (fd >= 0).then(|| OwnedFd::from_raw_fd(fd))
    }
}

/// Parent: whether the child's marker (or the end of its side of the pair)
/// is visible yet. Never blocks.
///
/// The marker, or the child's end closed: either way the probes are over.
/// The child closes its end with the `exec` (`CLOEXEC`), so after that point
/// the answer is always yes.
pub fn exec_marker_arrived(control: &OwnedFd) -> bool {
    let mut byte = [0u8; 8];
    // SAFETY: the buffer is valid for its length.
    let n = unsafe {
        libc::recv(
            control.as_raw_fd(),
            byte.as_mut_ptr().cast(),
            byte.len(),
            libc::MSG_DONTWAIT,
        )
    };
    n == 0 || (n > 0 && byte[0] == MARK_EXEC)
}

fn errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// The three messages arrive as the parent expects them, in order, and
    /// the listener arrives as a descriptor of its own.
    #[test]
    fn the_messages_cross_the_pair_in_their_shape() {
        let (parent, child) = channel().unwrap();
        assert!(!exec_marker_arrived(&parent), "nothing sent yet");

        let (probe, _keep) = channel().unwrap();
        assert!(send_listener(child.as_raw_fd(), probe.as_raw_fd()));
        let Received::Listener(fd) = receive(&parent) else {
            panic!("no descriptor arrived");
        };
        assert_ne!(fd.as_raw_fd(), probe.as_raw_fd(), "a new descriptor");

        send_unavailable(child.as_raw_fd(), libc::EBUSY);
        assert!(matches!(receive(&parent), Received::Unavailable(e) if e == libc::EBUSY));

        send_exec_marker(child.as_raw_fd());
        assert!(exec_marker_arrived(&parent));
        drop(child);
        assert!(exec_marker_arrived(&parent), "a closed end counts as over");
        assert!(matches!(receive(&parent), Received::Closed));
    }
}
