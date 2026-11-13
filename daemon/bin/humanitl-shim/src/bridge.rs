//! The bridges: a TCP port inside the sandbox, forwarded to a Unix socket the
//! launcher bind-mounted (ADR-002, `docs/SECURITY.md` Satz 2 and 3).
//!
//! The sandbox has no network interface but `lo`, so the only way out is a
//! socket the launcher put there. The agent speaks TCP to `127.0.0.1:3128`
//! because that is what every HTTP client understands; this module turns each
//! such connection into a connection on the Unix socket and copies bytes both
//! ways. The bridge lives in the parent shim, which is why the parent may
//! open `AF_UNIX` sockets and the agent may not.
//!
//! The list of bridges comes from the launcher as `HUMANITL_BRIDGES`, a JSON
//! array of `{"name","dir","listen","socket"}` rendered from the profile's
//! `[network].bridges`. Only direction `in` (sandbox TCP -> host Unix socket)
//! exists; `out` (host -> sandbox TCP, later the browser's CDP channel) is a
//! variant of [`Direction`] that [`bind`] refuses, which the launcher
//! reports as `SANDBOX_007`. [`parse`] reads exactly that shape (with the
//! reader of `json.rs`) and nothing more: the shim has no serde, and the
//! launcher is the only writer.
//!
//! In a guest kernel (HUM-465) there is no socket file from the host; the
//! same listener forwards to a vsock port of the host instead
//! (`Upstream::Vsock`, `Bound::over_vsock`, both Linux only), and the bridge's filter
//! allows `AF_VSOCK` instead of `AF_UNIX`.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use crate::json::{self, Parser};

/// Where the proxy socket sits inside the sandbox when the launcher sets no
/// `HUMANITL_BRIDGES` (CONVENTIONS.md 3.4).
pub const DEFAULT_SOCKET: &str = "/run/humanitl/proxy.sock";

/// The name of the bridge that reaches the proxy.
pub const PROXY_BRIDGE: &str = "proxy";

/// How many connections one bridge forwards at the same time.
///
/// Every accepted connection costs two threads and their stacks in the shim,
/// and the shim shares its memory with the agent. Without a bound a process
/// inside the sandbox could open connections to `127.0.0.1:3128` until the
/// host runs out of threads or address space, which would be a denial of
/// service reached over the one door the sandbox has on purpose. The bound is
/// generous for the intended traffic: an agent rarely holds more than fifty
/// connections open (HUM-012 Fallstricke) and the acceptance test
/// `bridge_many_conns` asks for two hundred at once. A connection over the
/// limit is closed at once, which an HTTP client reports as "proxy connection
/// failed" and retries; that is the same answer it gets when the proxy socket
/// is gone.
pub const MAX_CONNECTIONS: usize = 256;

/// Who listens and who connects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// The sandbox connects out: TCP listener in the sandbox, Unix socket
    /// from the host.
    In,
    /// The host connects in: Unix listener in the sandbox, TCP in the
    /// sandbox. Modelled, not built (HUM-012 Nicht-Ziel).
    Out,
}

/// One bridge as the profile declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bridge {
    /// The name the UI and the report use.
    pub name: String,
    /// Who listens and who connects.
    pub dir: Direction,
    /// The address inside the sandbox; must be loopback.
    pub listen: SocketAddr,
    /// The Unix socket the bridge serves; must be absolute.
    pub socket: PathBuf,
}

/// Where a bridge forwards each connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Upstream {
    /// The Unix socket the launcher put into the sandbox.
    Unix(PathBuf),
    /// A port of the host over vsock, in a guest kernel (HUM-465).
    #[cfg(target_os = "linux")]
    Vsock {
        /// The host's CID, always 2.
        cid: u32,
        /// The door's port from the guest plan.
        port: u32,
    },
}

impl fmt::Display for Upstream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix(path) => write!(f, "{}", path.display()),
            #[cfg(target_os = "linux")]
            Self::Vsock { cid, port } => write!(f, "vsock:{cid}:{port}"),
        }
    }
}

/// A bridge whose listener exists; created before the fork so the agent never
/// sees `ECONNREFUSED`.
#[derive(Debug)]
pub struct Bound {
    bridge: Bridge,
    upstream: Upstream,
    listener: TcpListener,
    limit: usize,
    counters: Arc<Counters>,
}

/// What a serving bridge counts: how many connections it forwards right now
/// and how many it refused because the limit was reached.
///
/// Shared between the accept loop and the forwarding threads, so the numbers
/// are exact rather than sampled.
#[derive(Debug, Default)]
pub struct Counters {
    live: AtomicUsize,
    refused: AtomicUsize,
}

impl Counters {
    /// Connections currently being forwarded. Read by the tests; the shim
    /// itself only counts.
    #[cfg(test)]
    #[must_use]
    pub fn live(&self) -> usize {
        self.live.load(Ordering::Acquire)
    }

    /// Connections closed unserved because the limit was reached. Read by the
    /// tests; the running total reaches an operator on stderr.
    #[cfg(test)]
    #[must_use]
    pub fn refused(&self) -> usize {
        self.refused.load(Ordering::Acquire)
    }
}

/// Why a bridge list could not be read, or a bridge not started.
#[derive(Debug)]
pub enum Error {
    /// `HUMANITL_BRIDGES` is not the JSON shape the launcher writes.
    Json {
        /// Byte offset in the text.
        offset: usize,
        /// What was expected there.
        what: &'static str,
    },
    /// An object key other than `name`, `dir`, `listen`, `socket`.
    UnknownField(String),
    /// A key given twice in one object.
    DuplicateField(&'static str),
    /// A key missing from an object.
    MissingField(&'static str),
    /// A name that is empty.
    EmptyName,
    /// A direction other than `in` or `out`.
    BadDirection(String),
    /// A listen address that does not parse as `host:port`.
    BadListen(String),
    /// A socket path that is not absolute.
    SocketNotAbsolute(String),
    /// Direction `out`, which the shim cannot do yet (`SANDBOX_007`).
    OutNotSupported(String),
    /// A listen address that is not loopback: the sandbox has nothing else.
    NotLoopback(String, SocketAddr),
    /// `bind(2)` failed.
    Bind(String, SocketAddr, io::Error),
    /// The listener did not accept the shim's own connection.
    SelfConnect(String, SocketAddr, io::Error),
    /// `--proxy-port` names a port no `in` bridge listens on.
    NoBridgeForProxyPort(u16),
    /// More than one bridge: every one of them is a door out of the sandbox,
    /// and the guarantee is exactly one (`docs/SECURITY.md` Satz 2). The
    /// launcher already refuses such a profile (`CONFIG_003`); this is the
    /// second layer, in the process that would open them.
    TooManyBridges(usize),
}

impl From<json::Error> for Error {
    fn from(err: json::Error) -> Self {
        Self::Json {
            offset: err.offset,
            what: err.what,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { offset, what } => {
                write!(f, "HUMANITL_BRIDGES: {what} at byte {offset}")
            }
            Self::UnknownField(key) => write!(f, "HUMANITL_BRIDGES: unknown field {key:?}"),
            Self::DuplicateField(key) => write!(f, "HUMANITL_BRIDGES: field {key:?} given twice"),
            Self::MissingField(key) => write!(f, "HUMANITL_BRIDGES: field {key:?} missing"),
            Self::EmptyName => write!(f, "HUMANITL_BRIDGES: a bridge has an empty name"),
            Self::BadDirection(dir) => {
                write!(
                    f,
                    "HUMANITL_BRIDGES: direction {dir:?} is neither \"in\" nor \"out\""
                )
            }
            Self::BadListen(addr) => {
                write!(
                    f,
                    "HUMANITL_BRIDGES: listen address {addr:?} is not host:port"
                )
            }
            Self::SocketNotAbsolute(path) => {
                write!(f, "HUMANITL_BRIDGES: socket path {path:?} is not absolute")
            }
            Self::OutNotSupported(name) => {
                write!(
                    f,
                    "bridge {name:?}: bridge direction out not supported yet (SANDBOX_007)"
                )
            }
            Self::NotLoopback(name, addr) => {
                write!(f, "bridge {name:?}: listen address {addr} is not loopback")
            }
            Self::Bind(name, addr, err) => {
                write!(f, "bridge {name:?}: cannot listen on {addr}: {err}")
            }
            Self::SelfConnect(name, addr, err) => {
                write!(
                    f,
                    "bridge {name:?}: listener on {addr} does not accept: {err}"
                )
            }
            Self::NoBridgeForProxyPort(port) => {
                write!(
                    f,
                    "no bridge with direction in listens on --proxy-port {port}"
                )
            }
            Self::TooManyBridges(count) => {
                write!(
                    f,
                    "HUMANITL_BRIDGES: {count} bridges; the sandbox has exactly one door"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl Bridge {
    /// The bridge the shim assumes when the launcher sets no
    /// `HUMANITL_BRIDGES`: `proxy`, direction `in`, `127.0.0.1:<port>`,
    /// [`DEFAULT_SOCKET`].
    #[must_use]
    pub fn default_proxy(port: u16) -> Self {
        Self {
            name: PROXY_BRIDGE.to_owned(),
            dir: Direction::In,
            listen: SocketAddr::from(([127, 0, 0, 1], port)),
            socket: PathBuf::from(DEFAULT_SOCKET),
        }
    }

    fn new(name: String, dir: &str, listen: &str, socket: &str) -> Result<Self, Error> {
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        let dir = match dir {
            "in" => Direction::In,
            "out" => Direction::Out,
            other => return Err(Error::BadDirection(other.to_owned())),
        };
        let listen: SocketAddr = listen
            .parse()
            .map_err(|_| Error::BadListen(listen.to_owned()))?;
        let socket = PathBuf::from(socket);
        if !socket.is_absolute() {
            return Err(Error::SocketNotAbsolute(
                socket.to_string_lossy().into_owned(),
            ));
        }
        Ok(Self {
            name,
            dir,
            listen,
            socket,
        })
    }
}

/// Reads `HUMANITL_BRIDGES`.
///
/// Accepts a JSON array of flat objects whose four values are strings, with
/// the usual whitespace and string escapes, and nothing else. Every deviation
/// is an error: the launcher wrote the text, so a surprise means a bug, and a
/// bug in the bridge list is not something to guess around.
pub fn parse(text: &str) -> Result<Vec<Bridge>, Error> {
    let mut parser = Parser::new(text);
    parser.skip_ws();
    parser.expect('[', "expected '['")?;
    let mut bridges = Vec::new();
    parser.skip_ws();
    if parser.peek() == Some(']') {
        parser.next();
    } else {
        loop {
            parser.skip_ws();
            bridges.push(object(&mut parser)?);
            parser.skip_ws();
            match parser.next() {
                Some(',') => {}
                Some(']') => break,
                _ => return Err(parser.error("expected ',' or ']'").into()),
            }
        }
    }
    parser.end()?;
    Ok(bridges)
}

/// Checks the list before anything is bound: no bridge may point `out`
/// (`SANDBOX_007`), there is at most one bridge, and `port` (the
/// `--proxy-port` from the command line) must be served by an `in` bridge,
/// because the agent's `HTTP_PROXY` points there and a mismatch between two
/// renderings of the same profile is a launcher bug.
///
/// The count is the second guarantee: the shim opens every bridge it is
/// given, so a list with two entries would put two listeners into the sandbox,
/// each with its own Unix socket. The launcher refuses such a profile already
/// (`humanitl_sandbox::SandboxProfile::parse`); refusing it here as well means
/// no path into this process opens a second door.
pub fn validate(bridges: &[Bridge], port: u16) -> Result<(), Error> {
    if let Some(out) = bridges.iter().find(|bridge| bridge.dir == Direction::Out) {
        return Err(Error::OutNotSupported(out.name.clone()));
    }
    if bridges.len() > 1 {
        return Err(Error::TooManyBridges(bridges.len()));
    }
    if bridges
        .iter()
        .any(|bridge| bridge.dir == Direction::In && bridge.listen.port() == port)
    {
        Ok(())
    } else {
        Err(Error::NoBridgeForProxyPort(port))
    }
}

/// Opens the listener of an `in` bridge.
///
/// Refuses direction `out` and any address that is not loopback. The
/// listener carries `CLOEXEC` (Rust's default), and the child closes every
/// inherited descriptor anyway.
pub fn bind(bridge: Bridge) -> Result<Bound, Error> {
    if bridge.dir == Direction::Out {
        return Err(Error::OutNotSupported(bridge.name));
    }
    if !bridge.listen.ip().is_loopback() {
        return Err(Error::NotLoopback(bridge.name, bridge.listen));
    }
    let listener = TcpListener::bind(bridge.listen)
        .map_err(|err| Error::Bind(bridge.name.clone(), bridge.listen, err))?;
    Ok(Bound {
        upstream: Upstream::Unix(bridge.socket.clone()),
        bridge,
        listener,
        limit: MAX_CONNECTIONS,
        counters: Arc::new(Counters::default()),
    })
}

impl Bound {
    /// The bridge as declared.
    #[must_use]
    pub fn bridge(&self) -> &Bridge {
        &self.bridge
    }

    /// Where each connection goes.
    #[must_use]
    pub fn upstream(&self) -> &Upstream {
        &self.upstream
    }

    /// The same bridge, forwarding to `port` of the host over vsock instead of
    /// the Unix socket (HUM-465).
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn over_vsock(mut self, port: u32) -> Self {
        self.upstream = Upstream::Vsock {
            cid: crate::linux::vsock::HOST_CID,
            port,
        };
        self
    }

    /// The listener's descriptor, which the shim keeps open when it closes
    /// everything else it inherited.
    #[must_use]
    pub fn listener_fd(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd as _;
        self.listener.as_raw_fd()
    }

    /// The address the listener actually got (differs from the declared one
    /// only for port 0, which tests use).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap_or(self.bridge.listen)
    }

    /// Connects to the own listener once and accepts that connection, so
    /// the report can say "listening" from evidence rather than from `bind`
    /// having returned.
    pub fn self_connect(&self) -> Result<(), Error> {
        let addr = self.local_addr();
        let wrap = |err| Error::SelfConnect(self.bridge.name.clone(), addr, err);
        let client = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(wrap)?;
        let (accepted, _) = self.listener.accept().map_err(wrap)?;
        drop(accepted);
        drop(client);
        Ok(())
    }

    /// The counters of this bridge, shared with its forwarding threads.
    #[cfg(test)]
    #[must_use]
    pub fn counters(&self) -> Arc<Counters> {
        Arc::clone(&self.counters)
    }

    /// The same bridge with another connection limit. Tests use it; the shim
    /// itself serves with [`MAX_CONNECTIONS`].
    #[cfg(test)]
    #[must_use]
    fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// The accept loop: one thread per connection, each forwarding to the
    /// Unix socket, at most [`MAX_CONNECTIONS`] at a time. Returns only if
    /// the listener itself breaks for good.
    ///
    /// A connection that cannot be forwarded (the socket is gone, no thread
    /// could be started, or the limit is reached) is closed at once; the
    /// client sees a closed connection and an HTTP client reports "proxy
    /// connection failed". Out of descriptors is transient: back off briefly
    /// and keep accepting.
    ///
    /// The limit is what makes the loop bounded: the counter is raised before
    /// the thread starts and lowered when it ends, so a process inside the
    /// sandbox cannot buy more threads and stacks in the shim than the bound
    /// allows, however many connections it opens.
    pub fn serve(self) {
        let upstream = self.upstream;
        let name = self.bridge.name;
        for connection in self.listener.incoming() {
            match connection {
                Ok(tcp) => {
                    if self.counters.live.fetch_add(1, Ordering::AcqRel) >= self.limit {
                        self.counters.live.fetch_sub(1, Ordering::AcqRel);
                        refuse(&name, &self.counters, self.limit, tcp);
                        continue;
                    }
                    let upstream = upstream.clone();
                    let counters = Arc::clone(&self.counters);
                    let spawned =
                        thread::Builder::new()
                            .name("bridge-conn".to_owned())
                            .spawn(move || {
                                forward(tcp, &upstream);
                                counters.live.fetch_sub(1, Ordering::AcqRel);
                            });
                    if spawned.is_err() {
                        self.counters.live.fetch_sub(1, Ordering::AcqRel);
                    }
                }
                Err(err) => {
                    if matches!(
                        err.raw_os_error(),
                        Some(libc::EMFILE | libc::ENFILE | libc::ENOBUFS | libc::ENOMEM)
                    ) {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
    }
}

/// Closes one connection over the limit and counts it.
///
/// Says so on stderr the first time and then on every doubling, so that a
/// short burst leaves one line and a flood leaves a handful with the running
/// total instead of one line per refused connection, which would be the same
/// denial of service in the log.
fn refuse(name: &str, counters: &Counters, limit: usize, tcp: TcpStream) {
    let _ = tcp.shutdown(Shutdown::Both);
    drop(tcp);
    let refused = counters.refused.fetch_add(1, Ordering::AcqRel) + 1;
    if refused.is_power_of_two() {
        let _ = io::stderr().lock().write_all(
            format!(
                "humanitl-shim: bridge {name:?}: {limit} connections are open, closed {refused} more\n"
            )
            .as_bytes(),
        );
    }
}

/// Copies bytes between one accepted TCP connection and a fresh connection
/// to the upstream, in both directions, until both sides are done.
///
/// Each direction runs in its own thread and, when its source reaches EOF
/// (or fails), half-closes its destination with `shutdown(Write)`, so an
/// HTTP client's "request done" reaches the proxy and the proxy's "response
/// done" reaches the client. Both descriptors close when the last of the two
/// threads returns.
fn forward(tcp: TcpStream, upstream: &Upstream) {
    match upstream {
        Upstream::Unix(socket) => {
            if let Ok(unix) = UnixStream::connect(socket)
                && let Ok(reader) = unix.try_clone()
            {
                relay(tcp, unix, reader);
            }
        }
        #[cfg(target_os = "linux")]
        Upstream::Vsock { cid, port } => {
            if let Ok(stream) = crate::linux::vsock::connect(*cid, *port)
                && let Ok(reader) = stream.try_clone()
            {
                relay(tcp, stream, reader);
            }
        }
    }
}

/// Both directions between `tcp` and one upstream connection, given as a
/// writer and a reader on it.
///
/// When the upstream is done (EOF), so is the connection: the proxy will
/// read nothing more. The reading side of `tcp` is shut down, which wakes the
/// other thread from a read on a client that keeps its writing side open,
/// and the writing side of the upstream connection is shut down, which
/// wakes the other thread from a write the upstream no longer reads. Then
/// that thread is joined, so that no thread and no descriptor outlives the
/// connection (HUM-465 review). A client's half-close still crosses first:
/// the other thread ends on the client's EOF on its own.
fn relay<U, R>(tcp: TcpStream, upstream: U, mut upstream_reader: R)
where
    U: Write + HalfClose + Send + 'static,
    R: Read + HalfClose,
{
    // The proxy answers in small pieces; Nagle would hold them back.
    let _ = tcp.set_nodelay(true);
    let (Ok(tcp_reader), Ok(tcp_control)) = (tcp.try_clone(), tcp.try_clone()) else {
        return;
    };
    let inbound = thread::Builder::new()
        .name("bridge-in".to_owned())
        .spawn(move || pump(tcp_reader, upstream));
    let Ok(inbound) = inbound else {
        return;
    };
    pump(&mut upstream_reader, tcp);
    let _ = tcp_control.shutdown(Shutdown::Read);
    upstream_reader.half_close();
    let _ = inbound.join();
}

fn pump<R: Read, W: Write + HalfClose>(mut from: R, mut to: W) {
    let _ = io::copy(&mut from, &mut to);
    to.half_close();
}

trait HalfClose {
    fn half_close(&self);
}

impl HalfClose for TcpStream {
    fn half_close(&self) {
        let _ = self.shutdown(Shutdown::Write);
    }
}

impl HalfClose for UnixStream {
    fn half_close(&self) {
        let _ = self.shutdown(Shutdown::Write);
    }
}

#[cfg(target_os = "linux")]
impl HalfClose for crate::linux::vsock::Stream {
    fn half_close(&self) {
        self.shutdown_write();
    }
}

/// One bridge object of the list, its four fields exactly once each.
fn object(parser: &mut Parser<'_>) -> Result<Bridge, Error> {
    parser.expect('{', "expected '{'")?;
    let mut name = None;
    let mut dir = None;
    let mut listen = None;
    let mut socket = None;
    parser.skip_ws();
    if parser.peek() == Some('}') {
        return Err(Error::MissingField("name"));
    }
    loop {
        parser.skip_ws();
        let key = parser.string()?;
        parser.skip_ws();
        parser.expect(':', "expected ':'")?;
        parser.skip_ws();
        let value = parser.string()?;
        let (field, slot): (&'static str, &mut Option<String>) = match key.as_str() {
            "name" => ("name", &mut name),
            "dir" => ("dir", &mut dir),
            "listen" => ("listen", &mut listen),
            "socket" => ("socket", &mut socket),
            other => return Err(Error::UnknownField(other.to_owned())),
        };
        if slot.is_some() {
            return Err(Error::DuplicateField(field));
        }
        *slot = Some(value);
        parser.skip_ws();
        match parser.next() {
            Some(',') => {}
            Some('}') => break,
            _ => return Err(parser.error("expected ',' or '}'").into()),
        }
    }
    let name = name.ok_or(Error::MissingField("name"))?;
    let dir = dir.ok_or(Error::MissingField("dir"))?;
    let listen = listen.ok_or(Error::MissingField("listen"))?;
    let socket = socket.ok_or(Error::MissingField("socket"))?;
    Bridge::new(name, &dir, &listen, &socket)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::{BufRead, BufReader, ErrorKind};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use std::path::Path;

    use super::*;

    const MVP: &str = r#"[{"name":"proxy","dir":"in","listen":"127.0.0.1:3128","socket":"/run/humanitl/proxy.sock"}]"#;

    // ---- the JSON shape ----------------------------------------------------

    #[test]
    fn parses_the_bridge_list_the_launcher_writes() {
        let bridges = parse(MVP).unwrap();
        assert_eq!(bridges, vec![Bridge::default_proxy(3128)]);
    }

    #[test]
    fn tolerates_whitespace_key_order_and_escapes() {
        let text = " [ {\n\t\"socket\" : \"/run/h\\u0075manitl/pro\\/xy.sock\",\n \"listen\":\"[::1]:3128\" ,\"dir\":\"in\",\"name\":\"pr\\\"oxy\" } , {\"name\":\"b\",\"dir\":\"in\",\"listen\":\"127.0.0.1:9\",\"socket\":\"/s\"} ]\n";
        let bridges = parse(text).unwrap();
        assert_eq!(bridges.len(), 2);
        assert_eq!(bridges[0].name, "pr\"oxy");
        assert_eq!(bridges[0].listen, "[::1]:3128".parse().unwrap());
        assert_eq!(bridges[0].socket, Path::new("/run/humanitl/pro/xy.sock"));
        assert_eq!(bridges[1].listen.port(), 9);
        let astral = r#"[{"name":"😀","dir":"in","listen":"127.0.0.1:1","socket":"/s"}]"#;
        assert_eq!(parse(astral).unwrap()[0].name, "😀");
    }

    #[test]
    fn empty_list_parses_and_out_is_a_variant() {
        assert!(parse("[]").unwrap().is_empty());
        let out = r#"[{"name":"cdp","dir":"out","listen":"127.0.0.1:9222","socket":"/run/humanitl/cdp.sock"}]"#;
        assert_eq!(parse(out).unwrap()[0].dir, Direction::Out);
    }

    #[test]
    fn every_deviation_from_the_shape_is_an_error() {
        let cases: &[(&str, &str)] = &[
            ("", "expected '['"),
            ("{}", "expected '['"),
            ("[{}]", "field \"name\" missing"),
            (
                r#"[{"name":"p","dir":"in","listen":"127.0.0.1:1"}]"#,
                "field \"socket\" missing",
            ),
            (
                r#"[{"name":"p","dir":"in","listen":"127.0.0.1:1","socket":"/s","x":"y"}]"#,
                "unknown field \"x\"",
            ),
            (r#"[{"name":"p","name":"q"}]"#, "field \"name\" given twice"),
            (
                r#"[{"name":"p","dir":"in","listen":"127.0.0.1:1","socket":"/s"}] x"#,
                "trailing data",
            ),
            (
                r#"[{"name":"p","dir":"in","listen":"127.0.0.1:1","socket":"/s"},]"#,
                "expected '{'",
            ),
            (
                r#"[{"name":"p","dir":"in","listen":"127.0.0.1:1","socket":1}]"#,
                "expected a string",
            ),
            (
                r#"[{"name":"p","dir":"sideways","listen":"127.0.0.1:1","socket":"/s"}]"#,
                "direction \"sideways\"",
            ),
            (
                r#"[{"name":"p","dir":"in","listen":"localhost:1","socket":"/s"}]"#,
                "listen address \"localhost:1\"",
            ),
            (
                r#"[{"name":"p","dir":"in","listen":"127.0.0.1:1","socket":"proxy.sock"}]"#,
                "not absolute",
            ),
            (
                r#"[{"name":"","dir":"in","listen":"127.0.0.1:1","socket":"/s"}]"#,
                "empty name",
            ),
            (
                r#"[{"name":"p\x","dir":"in","listen":"127.0.0.1:1","socket":"/s"}]"#,
                "bad escape",
            ),
            (
                r#"[{"name":"\ud83d","dir":"in","listen":"127.0.0.1:1","socket":"/s"}]"#,
                "low surrogate",
            ),
            ("[{\"name\":\"a\nb\"}]", "control character"),
            (r#"[{"name":"p"#, "unterminated string"),
        ];
        for (text, needle) in cases {
            let err = parse(text).expect_err(text).to_string();
            assert!(err.contains(needle), "{text}: {err}");
        }
    }

    #[test]
    fn proxy_port_must_be_served_by_an_in_bridge_and_out_is_refused_first() {
        let bridges = parse(MVP).unwrap();
        validate(&bridges, 3128).unwrap();
        assert!(matches!(
            validate(&bridges, 3129),
            Err(Error::NoBridgeForProxyPort(3129))
        ));
        let out = r#"[{"name":"proxy","dir":"in","listen":"127.0.0.1:3128","socket":"/s"},{"name":"cdp","dir":"out","listen":"127.0.0.1:9222","socket":"/c"}]"#;
        assert!(matches!(
            validate(&parse(out).unwrap(), 3128),
            Err(Error::OutNotSupported(name)) if name == "cdp"
        ));
        assert!(matches!(
            validate(&[], 3128),
            Err(Error::NoBridgeForProxyPort(3128))
        ));
    }

    /// A second bridge is a second door, and the shim would open it; it stops
    /// at the list instead (`docs/SECURITY.md` Satz 2).
    #[test]
    fn more_than_one_bridge_is_refused() {
        let two = r#"[{"name":"proxy","dir":"in","listen":"127.0.0.1:3128","socket":"/run/humanitl/proxy.sock"},{"name":"side","dir":"in","listen":"127.0.0.1:9222","socket":"/run/humanitl/side.sock"}]"#;
        let bridges = parse(two).unwrap();
        assert_eq!(bridges.len(), 2);
        let err = validate(&bridges, 3128).unwrap_err();
        assert!(matches!(err, Error::TooManyBridges(2)), "{err}");
        assert!(err.to_string().contains("exactly one door"), "{err}");
    }

    #[test]
    fn bind_refuses_out_and_non_loopback() {
        let mut out = Bridge::default_proxy(0);
        out.dir = Direction::Out;
        let err = bind(out).unwrap_err().to_string();
        assert!(
            err.contains("bridge direction out not supported yet"),
            "{err}"
        );
        let mut public = Bridge::default_proxy(0);
        public.listen = "0.0.0.0:0".parse().unwrap();
        assert!(matches!(bind(public), Err(Error::NotLoopback(..))));
    }

    // ---- bytes both ways ---------------------------------------------------

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A short, unique path: Unix socket paths are limited to 108 bytes.
    fn socket_path(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("humanitl-shim-{}-{tag}-{n}.sock", std::process::id());
        let dir = std::env::temp_dir();
        let path = dir.join(&name);
        if path.as_os_str().len() < 100 {
            path
        } else {
            PathBuf::from("/tmp").join(name)
        }
    }

    /// Serves `path`: every connection is echoed until the client half-closes;
    /// then `trailer` is sent and the connection closed. The trailer is only
    /// ever seen when the client's EOF crossed the bridge.
    fn echo_server(path: &Path, trailer: &'static [u8]) {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path).unwrap();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                thread::spawn(move || {
                    let mut buf = [0u8; 8192];
                    loop {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if stream.write_all(&buf[..n]).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    let _ = stream.write_all(trailer);
                });
            }
        });
    }

    fn bridge_to(socket: &Path) -> SocketAddr {
        bridge_to_limited(socket, MAX_CONNECTIONS).0
    }

    /// A serving bridge with a connection limit, and its counters.
    fn bridge_to_limited(socket: &Path, limit: usize) -> (SocketAddr, Arc<Counters>) {
        let mut bridge = Bridge::default_proxy(0);
        bridge.socket = socket.to_path_buf();
        let bound = bind(bridge).unwrap().with_limit(limit);
        bound.self_connect().unwrap();
        let addr = bound.local_addr();
        let counters = bound.counters();
        thread::spawn(move || bound.serve());
        (addr, counters)
    }

    /// Waits until `probe` holds, or fails after two seconds.
    fn wait_until(what: &str, probe: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !probe() {
            assert!(std::time::Instant::now() < deadline, "{what}");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn pseudo_random(len: usize) -> Vec<u8> {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 56) as u8
            })
            .collect()
    }

    #[test]
    fn bridge_roundtrip() {
        let socket = socket_path("roundtrip");
        echo_server(&socket, b"");
        let addr = bridge_to(&socket);
        let payload = pseudo_random(1024 * 1024);

        let mut client = TcpStream::connect(addr).unwrap();
        let mut reader = client.try_clone().unwrap();
        let expected = payload.clone();
        let writer = thread::spawn(move || {
            client.write_all(&payload).unwrap();
            client.shutdown(Shutdown::Write).unwrap();
        });
        let mut echoed = Vec::with_capacity(expected.len());
        reader.read_to_end(&mut echoed).unwrap();
        writer.join().unwrap();
        assert_eq!(echoed.len(), expected.len());
        assert!(echoed == expected, "the echo is not byte-identical");
        let _ = std::fs::remove_file(&socket);
    }

    #[test]
    fn half_close_crosses_the_bridge_in_both_directions() {
        let socket = socket_path("halfclose");
        echo_server(&socket, b"<eof seen>");
        let addr = bridge_to(&socket);

        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"ping").unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut got = Vec::new();
        client.read_to_end(&mut got).unwrap();
        assert_eq!(got, b"ping<eof seen>");
        let _ = std::fs::remove_file(&socket);
    }

    /// The inode of the bridge's side of the TCP connection from
    /// `client_port` to `bridge_port`, from `/proc/net/tcp`; `Some(0)` once
    /// no descriptor of this process holds it any more (an orphaned socket in
    /// its last TCP states).
    #[cfg(target_os = "linux")]
    fn bridge_side_inode(bridge_port: u16, client_port: u16) -> Option<u64> {
        let table = std::fs::read_to_string("/proc/net/tcp").ok()?;
        let local = format!("0100007F:{bridge_port:04X}");
        let remote = format!("0100007F:{client_port:04X}");
        table.lines().skip(1).find_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            (fields.get(1) == Some(&local.as_str()) && fields.get(2) == Some(&remote.as_str()))
                .then(|| fields.get(9).and_then(|inode| inode.parse().ok()))
                .flatten()
        })
    }

    /// HUM-465 review: the upstream ends while the client keeps its writing
    /// side open and sends nothing. The bridge must still let go of the
    /// connection: both threads end and no descriptor of it stays open.
    /// Before, the thread copying the client's bytes stayed blocked in its
    /// read, held a descriptor of the connection and piled up with every
    /// such connection.
    #[cfg(target_os = "linux")]
    #[test]
    fn upstream_eof_releases_a_client_that_keeps_writing() {
        let socket = socket_path("upstream-eof");
        let _ = std::fs::remove_file(&socket);
        let server = UnixListener::bind(&socket).unwrap();
        thread::spawn(move || {
            for stream in server.incoming() {
                let Ok(mut stream) = stream else { break };
                // Answer at once and close, without reading the request.
                let _ = stream.write_all(b"done");
            }
        });
        let (addr, counters) = bridge_to_limited(&socket, MAX_CONNECTIONS);
        for round in 0..20 {
            // The client sends nothing: a byte to the closed upstream would
            // end the copying thread through EPIPE and hide the leak.
            let mut client = TcpStream::connect(addr).unwrap();
            let mut got = [0u8; 4];
            client.read_exact(&mut got).unwrap();
            assert_eq!(&got, b"done");
            let mut rest = Vec::new();
            client.read_to_end(&mut rest).unwrap();
            // The client never writes again and never closes; the bridge's
            // side must be held by no descriptor within a moment.
            let client_port = client.local_addr().unwrap().port();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let released = loop {
                let inode = bridge_side_inode(addr.port(), client_port);
                if inode.is_none_or(|inode| inode == 0) {
                    break true;
                }
                if std::time::Instant::now() > deadline {
                    break false;
                }
                thread::sleep(Duration::from_millis(10));
            };
            assert!(
                released,
                "round {round}: a descriptor of the bridge still holds the connection"
            );
            drop(client);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while counters.live() > 0 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(counters.live(), 0);
        let _ = std::fs::remove_file(&socket);
    }

    /// The upstream ends its answer and stops reading while the client keeps
    /// streaming a body: the thread that writes the body to the upstream
    /// blocks in a write. Shutting down the upstream's writing side wakes it,
    /// so the connection's slot is free again (HUM-465 review).
    #[cfg(target_os = "linux")]
    #[test]
    fn upstream_eof_releases_a_thread_stuck_writing_to_the_upstream() {
        let socket = socket_path("upstream-stall");
        let _ = std::fs::remove_file(&socket);
        let server = UnixListener::bind(&socket).unwrap();
        let (end_answer, answer_ends) = std::sync::mpsc::channel::<()>();
        thread::spawn(move || {
            let Ok((stream, _)) = server.accept() else {
                return;
            };
            // Never read; end the answer only when told, and keep the
            // connection open afterwards.
            let _ = answer_ends.recv();
            let _ = stream.shutdown(Shutdown::Write);
            thread::sleep(Duration::from_secs(30));
        });
        let (addr, counters) = bridge_to_limited(&socket, MAX_CONNECTIONS);
        let client = TcpStream::connect(addr).unwrap();
        let mut writer = client.try_clone().unwrap();
        let written = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&written);
        thread::spawn(move || {
            let chunk = vec![0u8; 16 * 1024];
            while writer.write_all(&chunk).is_ok() {
                count.fetch_add(chunk.len(), std::sync::atomic::Ordering::Relaxed);
            }
        });
        // The body fills every buffer until the client's writes stall: then a
        // thread of the bridge sits in a write to the upstream.
        let mut last = usize::MAX;
        let filled = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            thread::sleep(Duration::from_millis(300));
            let now = written.load(std::sync::atomic::Ordering::Relaxed);
            if (now == last && now > 0) || std::time::Instant::now() > filled {
                break;
            }
            last = now;
        }
        assert_eq!(
            counters.live(),
            1,
            "the connection is live while the body stalls"
        );
        end_answer.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while counters.live() > 0 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            counters.live(),
            0,
            "the bridge still holds a connection whose upstream stopped reading"
        );
        drop(client);
        let _ = std::fs::remove_file(&socket);
    }

    #[test]
    fn bridge_many_conns() {
        let socket = socket_path("many");
        echo_server(&socket, b"");
        let addr = bridge_to(&socket);

        // macOS keeps at most 128 waiting connections per listener and drops
        // the SYN of the rest, which then wait for a retransmission; under a
        // loaded test run that took minutes (HUM-323).
        let count: u32 = if cfg!(target_os = "linux") { 200 } else { 100 };
        let workers: Vec<_> = (0..count)
            .map(|i| {
                thread::spawn(move || {
                    let mut client = TcpStream::connect(addr).unwrap();
                    let message = format!("connection {i}");
                    client.write_all(message.as_bytes()).unwrap();
                    client.shutdown(Shutdown::Write).unwrap();
                    let mut got = String::new();
                    client.read_to_string(&mut got).unwrap();
                    assert_eq!(got, message);
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let _ = std::fs::remove_file(&socket);
    }

    /// The limit is a bound on threads and stacks in the shim, so it has to
    /// hold from the first connection over it: the one over the limit is
    /// closed unserved and counted, the ones under it keep working, and once
    /// one of them ends the next connection is served again.
    #[test]
    fn bridge_refuses_connections_over_the_limit_and_counts_them() {
        let socket = socket_path("limit");
        echo_server(&socket, b"");
        let (addr, counters) = bridge_to_limited(&socket, 2);

        // Two connections that stay open: the echo answers, so the forwarding
        // threads are running and the counter is at the limit.
        let mut first = TcpStream::connect(addr).unwrap();
        let mut second = TcpStream::connect(addr).unwrap();
        for client in [&mut first, &mut second] {
            client.write_all(b"x").unwrap();
            let mut one = [0u8; 1];
            client.read_exact(&mut one).unwrap();
            assert_eq!(&one, b"x");
        }
        wait_until("both connections are counted", || counters.live() == 2);

        // The third is closed without ever reaching the Unix socket.
        let mut over = TcpStream::connect(addr).unwrap();
        let _ = over.write_all(b"y");
        let mut rest = Vec::new();
        match over.read_to_end(&mut rest) {
            Ok(_) => assert!(
                rest.is_empty(),
                "the bridge answered over the limit: {rest:?}"
            ),
            Err(err) => assert_eq!(err.kind(), ErrorKind::ConnectionReset, "{err}"),
        }
        wait_until("the refusal is counted", || counters.refused() == 1);
        assert_eq!(counters.live(), 2, "a refusal does not raise the count");

        // The connections under the limit are untouched by the refusal.
        first.write_all(b"z").unwrap();
        let mut one = [0u8; 1];
        first.read_exact(&mut one).unwrap();
        assert_eq!(&one, b"z");

        // A place freed by a closed connection is used again.
        drop(second);
        wait_until("the closed connection frees its place", || {
            counters.live() == 1
        });
        let mut again = TcpStream::connect(addr).unwrap();
        again.write_all(b"w").unwrap();
        again.read_exact(&mut one).unwrap();
        assert_eq!(&one, b"w");
        assert_eq!(counters.refused(), 1);

        drop(first);
        drop(again);
        let _ = std::fs::remove_file(&socket);
    }

    #[test]
    fn bridge_proxy_down() {
        let socket = socket_path("down");
        let _ = std::fs::remove_file(&socket);
        let addr = bridge_to(&socket);

        for _ in 0..2 {
            let mut client = TcpStream::connect(addr).unwrap();
            let mut buf = [0u8; 8];
            match client.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => panic!("read {n} bytes from a bridge whose socket is gone"),
                Err(err) => assert_eq!(err.kind(), ErrorKind::ConnectionReset, "{err}"),
            }
        }

        // The accept loop survived: once the socket exists, traffic flows.
        echo_server(&socket, b"");
        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"alive\n").unwrap();
        let mut line = String::new();
        BufReader::new(&client).read_line(&mut line).unwrap();
        assert_eq!(line, "alive\n");
        let _ = std::fs::remove_file(&socket);
    }
}
