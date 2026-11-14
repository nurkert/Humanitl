//! Stream sockets of the family `AF_VSOCK`, for the guest mode (HUM-465,
//! ADR-0021).
//!
//! In the kernel level the shim is PID 1 of a guest kernel, and the host is
//! reachable only through the one vsock device of the session. Two uses, both
//! of the shim alone and never of the agent, whose filter refuses the family:
//!
//! - the door: the bridge connects to CID 2 (the host), on the port the plan
//!   names, once per connection of the agent ([`connect`]); the host side
//!   hands it to the proxy;
//! - the report and the control stream: the shim listens on a port of its
//!   own and takes exactly two connections from the host, in this order
//!   ([`accept`]): the first carries the report, the second the frames of
//!   the terminals (HUM-443). The host asks for each with `CONNECT <port>`
//!   at the vsock device (HUM-442, Q10) and opens the second only after the
//!   device answered `OK` for the first, so the order is the host's. The
//!   listener closes after the second; the guest itself never opens a way
//!   out for either.
//!
//! `std` has no vsock type, so this module wraps the descriptor itself; the
//! calls are the same as for any stream socket.

use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

/// The CID of the host as seen from a guest (`VMADDR_CID_HOST`).
pub const HOST_CID: u32 = 2;

/// How long a connect to the host may take. The device answers an unknown
/// port with a timeout, not a refusal (HUM-442); the door exists before the
/// guest starts, so two seconds are plenty.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// `SO_VM_SOCKETS_CONNECT_TIMEOUT_NEW` of `<linux/vm_sockets.h>`, which the
/// `libc` crate does not carry: a `struct __kernel_sock_timeval`.
const SO_VM_SOCKETS_CONNECT_TIMEOUT_NEW: libc::c_int = 8;

/// One connected vsock stream.
#[derive(Debug)]
pub struct Stream(OwnedFd);

impl Stream {
    /// A second handle on the same connection, for the other direction.
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    /// Half-closes the writing direction.
    pub fn shutdown_write(&self) {
        // SAFETY: shutdown takes a descriptor this value owns and a constant.
        unsafe {
            libc::shutdown(self.0.as_raw_fd(), libc::SHUT_WR);
        }
    }

    /// The descriptor, given up.
    pub fn into_fd(self) -> OwnedFd {
        self.0
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // SAFETY: the buffer is valid for `buf.len()` bytes and outlives the call.
        let n = unsafe { libc::read(self.0.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        usize::try_from(n).map_err(|_| io::Error::last_os_error())
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // SAFETY: the buffer is valid for `buf.len()` bytes and outlives the call.
        let n = unsafe { libc::write(self.0.as_raw_fd(), buf.as_ptr().cast(), buf.len()) };
        usize::try_from(n).map_err(|_| io::Error::last_os_error())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A fresh stream socket of the family, with `CLOEXEC`.
fn socket() -> io::Result<OwnedFd> {
    // SAFETY: socket takes three integers.
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: socket just returned this descriptor, and nothing else owns it.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// The address of `port` at `cid`.
fn address(cid: u32, port: u32) -> libc::sockaddr_vm {
    // SAFETY: sockaddr_vm is plain data; all zero is a valid value.
    let mut addr: libc::sockaddr_vm = unsafe { std::mem::zeroed() };
    addr.svm_family = libc::sa_family_t::try_from(libc::AF_VSOCK).unwrap_or(0);
    addr.svm_cid = cid;
    addr.svm_port = port;
    addr
}

fn addr_len() -> libc::socklen_t {
    libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_vm>()).unwrap_or(0)
}

/// Connects to `port` at `cid`, waiting at most [`CONNECT_TIMEOUT`].
pub fn connect(cid: u32, port: u32) -> io::Result<Stream> {
    let fd = socket()?;
    let timeout = libc::timeval {
        tv_sec: CONNECT_TIMEOUT.as_secs().try_into().unwrap_or(2),
        tv_usec: 0,
    };
    // SAFETY: setsockopt reads `size_of::<timeval>()` bytes from a valid
    // value. A kernel without the option keeps its own default; the connect
    // below still ends.
    unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            libc::AF_VSOCK,
            SO_VM_SOCKETS_CONNECT_TIMEOUT_NEW,
            (&raw const timeout).cast(),
            libc::socklen_t::try_from(std::mem::size_of::<libc::timeval>()).unwrap_or(0),
        );
    }
    let addr = address(cid, port);
    loop {
        // SAFETY: connect reads `addr_len()` bytes from a valid sockaddr_vm.
        let rc = unsafe { libc::connect(fd.as_raw_fd(), (&raw const addr).cast(), addr_len()) };
        if rc == 0 {
            return Ok(Stream(fd));
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

/// Listens on `port` of this guest and takes exactly `count` connections,
/// in the order they arrive, waiting at most `wait` for all of them; the
/// listener is closed before this returns, so nobody can open one more.
pub fn accept(port: u32, count: usize, wait: Duration) -> io::Result<Vec<Stream>> {
    let listener = socket()?;
    let addr = address(libc::VMADDR_CID_ANY, port);
    // SAFETY: bind reads `addr_len()` bytes from a valid sockaddr_vm.
    if unsafe { libc::bind(listener.as_raw_fd(), (&raw const addr).cast(), addr_len()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let backlog = libc::c_int::try_from(count).unwrap_or(1);
    // SAFETY: listen takes a descriptor this function owns and a backlog.
    if unsafe { libc::listen(listener.as_raw_fd(), backlog) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let deadline = std::time::Instant::now() + wait;
    let mut taken = Vec::with_capacity(count);
    while taken.len() < count {
        taken.push(accept_next(&listener, port, deadline, wait)?);
    }
    drop(listener);
    Ok(taken)
}

/// Waits until `deadline` for the next connection on `listener` and takes it.
fn accept_next(
    listener: &OwnedFd,
    port: u32,
    deadline: std::time::Instant,
    wait: Duration,
) -> io::Result<Stream> {
    let mut poll = libc::pollfd {
        fd: listener.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let millis = libc::c_int::try_from(left.as_millis()).unwrap_or(libc::c_int::MAX);
        // SAFETY: one pollfd, valid for the call.
        let ready = unsafe { libc::poll(&raw mut poll, 1, millis) };
        if ready > 0 {
            break;
        }
        if ready == 0 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("nobody connected to port {port} within {}s", wait.as_secs()),
            ));
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
    // SAFETY: accept4 with null address pointers only returns a descriptor.
    let fd = unsafe {
        libc::accept4(
            listener.as_raw_fd(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            libc::SOCK_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: accept4 just returned this descriptor, and nothing else owns it.
    Ok(Stream(unsafe { OwnedFd::from_raw_fd(fd) }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn the_address_names_family_cid_and_port() {
        let addr = address(HOST_CID, 5000);
        assert_eq!(i32::from(addr.svm_family), libc::AF_VSOCK);
        assert_eq!((addr.svm_cid, addr.svm_port), (2, 5000));
        assert_eq!(
            addr_len() as usize,
            std::mem::size_of::<libc::sockaddr_vm>()
        );
    }
}
