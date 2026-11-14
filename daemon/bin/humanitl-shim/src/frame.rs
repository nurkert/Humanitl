//! The frames of the control stream (HUM-443): one codec for both ends.
//!
//! In a micro-VM no descriptor crosses the boundary. The pseudo-terminals of
//! the agent and of every further shell live in the guest, and the daemon
//! talks to them over one stream socket: `AF_VSOCK` in the guest, a Unix
//! socket pair under bwrap when the stream mode is forced. This file is the
//! whole wire format. The shim compiles it as `frame`, and the daemon
//! includes the very same file (`humanitl_sandbox::control_stream`), so the
//! two ends cannot drift apart. It is plain Rust without `unsafe` and without
//! a dependency, which both crates allow.
//!
//! # Layout
//!
//! | bytes | meaning |
//! |---|---|
//! | 1 | kind ([`Kind`]) |
//! | 4, little endian | channel: 0 is the agent, a further shell has the id the daemon chose |
//! | 4, little endian | length of the payload, at most [`MAX_PAYLOAD`] |
//! | length | payload, its shape fixed by the kind |
//!
//! Daemon to shim: `D` terminal bytes, `W` size (cols, rows: two `u16`),
//! `O` open a shell (cols, rows, then the command with a NUL after each
//! argument), `C` close a shell, `I` interrupt the foreground of a channel,
//! `H` heartbeat. Shim to daemon: `D` terminal bytes, `R` started (pid),
//! `F` refused (a code as in `exec.rs`), `E` exit (code or 128 + signal),
//! `H` heartbeat. Every value is an `i32` or `u16`, little endian.
//!
//! # What a reader does with what it does not expect
//!
//! A frame whose declared length exceeds [`MAX_PAYLOAD`] is skipped: the
//! reader counts its bytes away without keeping them and then reads the next
//! header ([`Dropped::Oversized`]). A frame of an unknown kind is skipped the
//! same way ([`Dropped::UnknownKind`]). A frame of a known kind whose
//! payload has the wrong shape, or a kind that travels the other way, is
//! dropped by [`Message::parse`]; an unknown channel is dropped by the side
//! that serves it. None of it ends the stream, and none of it makes a reader
//! keep more than [`HEADER`] + [`MAX_PAYLOAD`] bytes ([`Decoder::buffered`]).
//! Only the end of the stream ends it, and that end is the lifeline.

/// Bytes of a header: kind, channel, length.
pub const HEADER: usize = 9;

/// The most bytes a payload may have. A read of a terminal fills one frame
/// at most; an `O` carries a command of at most this length.
pub const MAX_PAYLOAD: usize = 16 * 1024;

/// The most arguments the command of an `O` may have, as for the door
/// (`exec.rs`).
pub const MAX_ARGS: usize = 256;

/// The channel of the agent's terminal.
pub const AGENT: u32 = 0;

/// The kinds of frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Terminal bytes, both ways.
    Data,
    /// The size of a terminal, daemon to shim.
    Size,
    /// Open a further shell, daemon to shim.
    Open,
    /// Close a further shell, daemon to shim.
    Close,
    /// Interrupt the foreground of a terminal, daemon to shim.
    Interrupt,
    /// A sign of life, both ways.
    Heartbeat,
    /// A further shell started, shim to daemon.
    Started,
    /// A further shell was refused, shim to daemon.
    Refused,
    /// The process of a channel ended, shim to daemon.
    Exit,
}

impl Kind {
    /// The byte on the wire.
    #[must_use]
    pub const fn byte(self) -> u8 {
        match self {
            Self::Data => b'D',
            Self::Size => b'W',
            Self::Open => b'O',
            Self::Close => b'C',
            Self::Interrupt => b'I',
            Self::Heartbeat => b'H',
            Self::Started => b'R',
            Self::Refused => b'F',
            Self::Exit => b'E',
        }
    }

    /// The kind of a byte, `None` for an unknown one.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            b'D' => Self::Data,
            b'W' => Self::Size,
            b'O' => Self::Open,
            b'C' => Self::Close,
            b'I' => Self::Interrupt,
            b'H' => Self::Heartbeat,
            b'R' => Self::Started,
            b'F' => Self::Refused,
            b'E' => Self::Exit,
            _ => return None,
        })
    }
}

/// One frame as it came off the stream, payload unchecked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Its kind.
    pub kind: Kind,
    /// Its channel.
    pub channel: u32,
    /// Its payload, at most [`MAX_PAYLOAD`] bytes.
    pub payload: Vec<u8>,
}

/// A frame the reader skipped without keeping its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// The declared length exceeds [`MAX_PAYLOAD`].
    Oversized {
        /// The kind byte, known or not.
        kind: u8,
        /// The channel it named.
        channel: u32,
        /// The length it declared.
        len: u32,
    },
    /// The kind byte names no [`Kind`].
    UnknownKind {
        /// The byte.
        kind: u8,
        /// The channel it named.
        channel: u32,
    },
}

/// What one step of the reader produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A whole frame of a known kind.
    Frame(Frame),
    /// A frame skipped, once its last byte has passed.
    Dropped(Dropped),
}

/// Where the reader stands.
#[derive(Debug)]
enum State {
    /// Collecting the nine bytes of a header.
    Header { have: usize },
    /// Collecting the payload of a frame of a known kind.
    Payload {
        kind: Kind,
        channel: u32,
        len: usize,
    },
    /// Counting away the payload of a skipped frame.
    Skip { left: u64, dropped: Dropped },
}

/// The incremental reader of a control stream.
///
/// Fed with whatever a `read(2)` returned, in any cut; hands out every frame
/// once its last byte arrived. It never keeps more than [`HEADER`] +
/// [`MAX_PAYLOAD`] bytes, whatever it is fed.
#[derive(Debug)]
pub struct Decoder {
    header: [u8; HEADER],
    payload: Vec<u8>,
    state: State,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    /// A reader before the first header.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            header: [0; HEADER],
            payload: Vec::new(),
            state: State::Header { have: 0 },
        }
    }

    /// How many bytes the reader holds right now; never more than
    /// [`HEADER`] + [`MAX_PAYLOAD`].
    #[must_use]
    pub fn buffered(&self) -> usize {
        let header = match self.state {
            State::Header { have } => have,
            State::Payload { .. } | State::Skip { .. } => HEADER,
        };
        header + self.payload.len()
    }

    /// Feeds `input` and calls `out` for every frame it completes, in order.
    pub fn push(&mut self, mut input: &[u8], mut out: impl FnMut(Event)) {
        while !input.is_empty() {
            match self.state {
                State::Header { have } => {
                    let take = (HEADER - have).min(input.len());
                    self.header[have..have + take].copy_from_slice(&input[..take]);
                    input = &input[take..];
                    if have + take < HEADER {
                        self.state = State::Header { have: have + take };
                    } else {
                        self.state = self.after_header();
                        self.finish_empty(&mut out);
                    }
                }
                State::Payload { kind, channel, len } => {
                    let take = (len - self.payload.len()).min(input.len());
                    self.payload.extend_from_slice(&input[..take]);
                    input = &input[take..];
                    if self.payload.len() == len {
                        out(Event::Frame(Frame {
                            kind,
                            channel,
                            payload: std::mem::take(&mut self.payload),
                        }));
                        self.state = State::Header { have: 0 };
                    }
                }
                State::Skip { left, dropped } => {
                    let available = u64::try_from(input.len()).unwrap_or(u64::MAX);
                    let take = left.min(available);
                    input = &input[usize::try_from(take).unwrap_or(input.len())..];
                    if take == left {
                        out(Event::Dropped(dropped));
                        self.state = State::Header { have: 0 };
                    } else {
                        self.state = State::Skip {
                            left: left - take,
                            dropped,
                        };
                    }
                }
            }
        }
    }

    /// The state that follows a complete header. The length check lives
    /// here and nowhere else: a frame that declares more than
    /// [`MAX_PAYLOAD`] is never collected, only counted away.
    fn after_header(&mut self) -> State {
        let [kind, c0, c1, c2, c3, l0, l1, l2, l3] = self.header;
        let channel = u32::from_le_bytes([c0, c1, c2, c3]);
        let len = u32::from_le_bytes([l0, l1, l2, l3]);
        let declared = usize::try_from(len).unwrap_or(usize::MAX);
        if declared > MAX_PAYLOAD {
            return State::Skip {
                left: u64::from(len),
                dropped: Dropped::Oversized { kind, channel, len },
            };
        }
        match Kind::from_byte(kind) {
            Some(kind) => {
                self.payload = Vec::with_capacity(declared);
                State::Payload {
                    kind,
                    channel,
                    len: declared,
                }
            }
            None => State::Skip {
                left: u64::from(len),
                dropped: Dropped::UnknownKind { kind, channel },
            },
        }
    }

    /// A frame without payload is complete with its header.
    fn finish_empty(&mut self, out: &mut impl FnMut(Event)) {
        match self.state {
            State::Payload {
                kind,
                channel,
                len: 0,
            } => {
                out(Event::Frame(Frame {
                    kind,
                    channel,
                    payload: Vec::new(),
                }));
                self.state = State::Header { have: 0 };
            }
            State::Skip { left: 0, dropped } => {
                out(Event::Dropped(dropped));
                self.state = State::Header { have: 0 };
            }
            _ => {}
        }
    }
}

/// A frame whose payload has the shape its kind asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Terminal bytes of a channel, never empty.
    Data {
        /// The channel.
        channel: u32,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// The size of a channel's terminal.
    Size {
        /// The channel.
        channel: u32,
        /// Columns.
        cols: u16,
        /// Rows.
        rows: u16,
    },
    /// Open a further shell on a new channel.
    Open {
        /// The new channel, never [`AGENT`].
        channel: u32,
        /// Columns.
        cols: u16,
        /// Rows.
        rows: u16,
        /// The command, never empty, the first argument never empty, no
        /// argument with a NUL.
        argv: Vec<Vec<u8>>,
    },
    /// Close a further shell.
    Close {
        /// The channel, never [`AGENT`].
        channel: u32,
    },
    /// Interrupt the foreground of a channel.
    Interrupt {
        /// The channel.
        channel: u32,
    },
    /// A sign of life.
    Heartbeat,
    /// A further shell started.
    Started {
        /// Its channel.
        channel: u32,
        /// Its pid, as the sandbox counts.
        pid: i32,
    },
    /// A further shell was refused.
    Refused {
        /// Its channel.
        channel: u32,
        /// The code.
        code: i32,
    },
    /// The process of a channel ended.
    Exit {
        /// The channel.
        channel: u32,
        /// Exit code, or 128 + signal.
        code: i32,
    },
}

/// Why a frame of a known kind is not a [`Message`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed(pub &'static str);

fn int(payload: &[u8]) -> Result<i32, Malformed> {
    match payload {
        [a, b, c, d] => Ok(i32::from_le_bytes([*a, *b, *c, *d])),
        _ => Err(Malformed("a value is four bytes")),
    }
}

fn size(payload: &[u8]) -> Option<(u16, u16)> {
    match payload {
        [c0, c1, r0, r1, ..] => Some((
            u16::from_le_bytes([*c0, *c1]),
            u16::from_le_bytes([*r0, *r1]),
        )),
        _ => None,
    }
}

fn command(body: &[u8]) -> Result<Vec<Vec<u8>>, Malformed> {
    let Some((&0, args)) = body.split_last() else {
        return Err(Malformed("the last argument is not terminated"));
    };
    let parts: Vec<&[u8]> = args.split(|byte| *byte == 0).collect();
    if parts.len() > MAX_ARGS {
        return Err(Malformed("too many arguments"));
    }
    if parts.first().is_none_or(|first| first.is_empty()) {
        return Err(Malformed("empty program name"));
    }
    Ok(parts.into_iter().map(<[u8]>::to_vec).collect())
}

impl Message {
    /// Checks the payload of `frame` against its kind.
    ///
    /// # Errors
    ///
    /// [`Malformed`] for a payload of the wrong shape: empty data, a size or
    /// value of the wrong length, an `O` without a command, an `O` or `C` on
    /// the agent's channel, a payload on a kind that carries none.
    pub fn parse(frame: &Frame) -> Result<Self, Malformed> {
        let channel = frame.channel;
        let payload = frame.payload.as_slice();
        let empty = || {
            if payload.is_empty() {
                Ok(())
            } else {
                Err(Malformed("this kind carries no payload"))
            }
        };
        Ok(match frame.kind {
            Kind::Data if payload.is_empty() => return Err(Malformed("empty data")),
            Kind::Data => Self::Data {
                channel,
                bytes: payload.to_vec(),
            },
            Kind::Size => match (size(payload), payload.len()) {
                (Some((cols, rows)), 4) => Self::Size {
                    channel,
                    cols,
                    rows,
                },
                _ => return Err(Malformed("a size is four bytes")),
            },
            Kind::Open => {
                if channel == AGENT {
                    return Err(Malformed("channel 0 is the agent's"));
                }
                let (cols, rows) = size(payload).ok_or(Malformed("an open starts with a size"))?;
                Self::Open {
                    channel,
                    cols,
                    rows,
                    argv: command(&payload[4..])?,
                }
            }
            Kind::Close => {
                if channel == AGENT {
                    return Err(Malformed("channel 0 is the agent's"));
                }
                empty()?;
                Self::Close { channel }
            }
            Kind::Interrupt => {
                empty()?;
                Self::Interrupt { channel }
            }
            Kind::Heartbeat => {
                empty()?;
                Self::Heartbeat
            }
            Kind::Started => Self::Started {
                channel,
                pid: int(payload)?,
            },
            Kind::Refused => Self::Refused {
                channel,
                code: int(payload)?,
            },
            Kind::Exit => Self::Exit {
                channel,
                code: int(payload)?,
            },
        })
    }
}

/// Appends one frame to `out`.
///
/// # Errors
///
/// [`Malformed`] when `payload` is longer than [`MAX_PAYLOAD`]; nothing is
/// appended then.
pub fn encode(
    kind: Kind,
    channel: u32,
    payload: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), Malformed> {
    let len = u32::try_from(payload.len())
        .ok()
        .filter(|_| payload.len() <= MAX_PAYLOAD)
        .ok_or(Malformed("the payload is too long"))?;
    out.push(kind.byte());
    out.extend_from_slice(&channel.to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

/// Appends terminal bytes of any length, cut into frames of at most
/// [`MAX_PAYLOAD`] bytes.
pub fn encode_data(channel: u32, bytes: &[u8], out: &mut Vec<u8>) {
    for chunk in bytes.chunks(MAX_PAYLOAD) {
        // A chunk is never longer than the limit, so this cannot fail.
        let _ = encode(Kind::Data, channel, chunk, out);
    }
}

/// Appends a frame that carries one `i32`.
pub fn encode_value(kind: Kind, channel: u32, value: i32, out: &mut Vec<u8>) {
    let _ = encode(kind, channel, &value.to_le_bytes(), out);
}

/// Appends a `W` frame.
pub fn encode_size(channel: u32, cols: u16, rows: u16, out: &mut Vec<u8>) {
    let mut payload = [0u8; 4];
    payload[..2].copy_from_slice(&cols.to_le_bytes());
    payload[2..].copy_from_slice(&rows.to_le_bytes());
    let _ = encode(Kind::Size, channel, &payload, out);
}

/// Appends an `O` frame.
///
/// # Errors
///
/// [`Malformed`] for the agent's channel, an empty command or program name,
/// an argument with a NUL, more than [`MAX_ARGS`] arguments, or a command
/// that does not fit one frame; nothing is appended then.
pub fn encode_open(
    channel: u32,
    cols: u16,
    rows: u16,
    argv: &[&[u8]],
    out: &mut Vec<u8>,
) -> Result<(), Malformed> {
    if channel == AGENT {
        return Err(Malformed("channel 0 is the agent's"));
    }
    if argv.first().is_none_or(|first| first.is_empty()) {
        return Err(Malformed("empty program name"));
    }
    if argv.len() > MAX_ARGS {
        return Err(Malformed("too many arguments"));
    }
    let mut payload = Vec::with_capacity(4 + argv.iter().map(|arg| arg.len() + 1).sum::<usize>());
    payload.extend_from_slice(&cols.to_le_bytes());
    payload.extend_from_slice(&rows.to_le_bytes());
    for arg in argv {
        if arg.contains(&0) {
            return Err(Malformed("an argument contains a NUL byte"));
        }
        payload.extend_from_slice(arg);
        payload.push(0);
    }
    encode(Kind::Open, channel, &payload, out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn decode(bytes: &[u8], cut: usize) -> (Vec<Event>, usize) {
        let mut decoder = Decoder::new();
        let mut events = Vec::new();
        let mut most = 0;
        for chunk in bytes.chunks(cut.max(1)) {
            decoder.push(chunk, |event| events.push(event));
            most = most.max(decoder.buffered());
        }
        (events, most)
    }

    fn header(kind: u8, channel: u32, len: u32) -> Vec<u8> {
        let mut out = vec![kind];
        out.extend_from_slice(&channel.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out
    }

    #[test]
    fn every_kind_survives_the_round_trip_in_any_cut() {
        let mut wire = Vec::new();
        encode_data(AGENT, b"hello", &mut wire);
        encode_size(3, 120, 40, &mut wire);
        encode_open(3, 80, 24, &[b"bash", b"-i"], &mut wire).unwrap();
        encode(Kind::Close, 3, &[], &mut wire).unwrap();
        encode(Kind::Interrupt, AGENT, &[], &mut wire).unwrap();
        encode(Kind::Heartbeat, AGENT, &[], &mut wire).unwrap();
        encode_value(Kind::Started, 3, 42, &mut wire);
        encode_value(Kind::Refused, 4, -1, &mut wire);
        encode_value(Kind::Exit, AGENT, 130, &mut wire);
        let expected = vec![
            Message::Data {
                channel: 0,
                bytes: b"hello".to_vec(),
            },
            Message::Size {
                channel: 3,
                cols: 120,
                rows: 40,
            },
            Message::Open {
                channel: 3,
                cols: 80,
                rows: 24,
                argv: vec![b"bash".to_vec(), b"-i".to_vec()],
            },
            Message::Close { channel: 3 },
            Message::Interrupt { channel: 0 },
            Message::Heartbeat,
            Message::Started {
                channel: 3,
                pid: 42,
            },
            Message::Refused {
                channel: 4,
                code: -1,
            },
            Message::Exit {
                channel: 0,
                code: 130,
            },
        ];
        for cut in [1, 2, 7, 9, 10, 64, wire.len()] {
            let (events, _) = decode(&wire, cut);
            let messages: Vec<Message> = events
                .iter()
                .map(|event| match event {
                    Event::Frame(frame) => Message::parse(frame).unwrap(),
                    Event::Dropped(dropped) => panic!("dropped {dropped:?}"),
                })
                .collect();
            assert_eq!(messages, expected, "cut {cut}");
        }
    }

    /// A hostile peer: an oversized frame, an unknown kind, a malformed
    /// payload. Each is dropped on its own, the frame after it arrives, and
    /// the reader never holds more than one frame.
    #[test]
    fn oversized_and_unknown_frames_are_skipped_and_the_stream_stays_in_step() {
        let skipped = 3 * MAX_PAYLOAD;
        // An oversized frame whose declared bytes do follow, in full.
        let mut wire = header(b'D', 0, u32::try_from(skipped).unwrap());
        wire.extend(std::iter::repeat_n(b'x', skipped));
        wire.extend(header(b'?', 9, 5));
        wire.extend(b"junk!");
        wire.extend(header(b'W', 0, 3));
        wire.extend(b"abc");
        encode_data(AGENT, b"after", &mut wire);
        for cut in [1, 5, 4096, wire.len()] {
            let (events, most) = decode(&wire, cut);
            assert!(most <= HEADER + MAX_PAYLOAD, "held {most} bytes");
            assert_eq!(events.len(), 4, "cut {cut}: {events:?}");
            assert!(matches!(
                events[0],
                Event::Dropped(Dropped::Oversized { kind: b'D', len, .. }) if len as usize == skipped
            ));
            assert_eq!(
                events[1],
                Event::Dropped(Dropped::UnknownKind {
                    kind: b'?',
                    channel: 9
                })
            );
            let Event::Frame(bad) = &events[2] else {
                panic!("{:?}", events[2]);
            };
            assert!(Message::parse(bad).is_err());
            let Event::Frame(good) = &events[3] else {
                panic!("{:?}", events[3]);
            };
            assert_eq!(
                Message::parse(good),
                Ok(Message::Data {
                    channel: 0,
                    bytes: b"after".to_vec()
                })
            );
        }
        // A header that promises four gigabytes keeps the reader at nine
        // bytes, however much follows.
        let mut endless = Decoder::new();
        endless.push(&header(b'D', 0, u32::MAX), |_| {});
        for _ in 0..64 {
            endless.push(&[0u8; 4096], |_| panic!("nothing completes"));
            assert_eq!(endless.buffered(), HEADER);
        }
    }

    #[test]
    fn a_payload_of_the_wrong_shape_is_malformed() {
        let frame = |kind, channel, payload: &[u8]| Frame {
            kind,
            channel,
            payload: payload.to_vec(),
        };
        let bad = [
            frame(Kind::Data, 0, b""),
            frame(Kind::Size, 0, b"abc"),
            frame(Kind::Size, 0, b"abcde"),
            frame(Kind::Open, 0, b"\x50\0\x18\0sh\0"),
            frame(Kind::Open, 1, b"\x50\0"),
            frame(Kind::Open, 1, b"\x50\0\x18\0sh"),
            frame(Kind::Open, 1, b"\x50\0\x18\0\0x\0"),
            frame(Kind::Close, 0, b""),
            frame(Kind::Close, 1, b"x"),
            frame(Kind::Interrupt, 0, b"x"),
            frame(Kind::Heartbeat, 0, b"x"),
            frame(Kind::Started, 1, b"abc"),
            frame(Kind::Exit, 1, b"abcde"),
        ];
        for frame in &bad {
            assert!(Message::parse(frame).is_err(), "{frame:?}");
        }
        let many: Vec<Vec<u8>> = (0..=MAX_ARGS).map(|n| n.to_string().into_bytes()).collect();
        let many: Vec<&[u8]> = many.iter().map(Vec::as_slice).collect();
        assert!(encode_open(1, 80, 24, &many, &mut Vec::new()).is_err());
        assert!(encode_open(0, 80, 24, &[b"sh"], &mut Vec::new()).is_err());
        assert!(encode_open(1, 80, 24, &[b"a\0b"], &mut Vec::new()).is_err());
        let long = vec![b'x'; MAX_PAYLOAD];
        let mut out = Vec::new();
        assert!(encode_open(1, 80, 24, &[&long], &mut out).is_err());
        assert!(out.is_empty(), "nothing appended on a refusal");
    }

    /// An open with exactly [`MAX_ARGS`] arguments reads, one more does
    /// not; the size comes first.
    #[test]
    fn an_open_carries_at_most_max_args_arguments() {
        let open = |count: usize| {
            let mut payload = vec![0x50, 0, 0x18, 0];
            for n in 0..count {
                payload.extend_from_slice(n.to_string().as_bytes());
                payload.push(0);
            }
            Frame {
                kind: Kind::Open,
                channel: 4,
                payload,
            }
        };
        let Ok(Message::Open {
            channel,
            cols,
            rows,
            argv,
        }) = Message::parse(&open(MAX_ARGS))
        else {
            panic!("an open of {MAX_ARGS} arguments did not read");
        };
        assert_eq!((channel, cols, rows, argv.len()), (4, 80, 24, MAX_ARGS));
        assert_eq!(argv[MAX_ARGS - 1], (MAX_ARGS - 1).to_string().into_bytes());
        assert_eq!(
            Message::parse(&open(MAX_ARGS + 1)),
            Err(Malformed("too many arguments"))
        );
    }

    #[test]
    fn long_data_is_cut_into_frames_that_fit() {
        let bytes = vec![7u8; 2 * MAX_PAYLOAD + 5];
        let mut wire = Vec::new();
        encode_data(5, &bytes, &mut wire);
        let (events, _) = decode(&wire, 1000);
        let mut joined = Vec::new();
        for event in events {
            let Event::Frame(frame) = event else {
                panic!("{event:?}");
            };
            assert!(frame.payload.len() <= MAX_PAYLOAD);
            joined.extend(frame.payload);
        }
        assert_eq!(joined, bytes);
    }
}
