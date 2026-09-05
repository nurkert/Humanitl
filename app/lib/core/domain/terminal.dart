/// The terminal of the sandbox: what a client sends up, what comes back down
/// (HUM-042).
///
/// The pseudo terminal lives in the daemon, because that is where bubblewrap
/// runs and because this app will be sandboxed itself (ADR-003). What travels
/// here is therefore not a terminal but a stream of bytes with three
/// out-of-band messages beside it: the geometry of the writer, a finding, and
/// the exit code of the agent.
///
/// The bytes are **filtered in the daemon**, once, for every client
/// (`docs/SECURITY.md` 3.3). Nothing in this app filters them a second time:
/// a second filter would be a second promise, and the one that counts is the
/// one every client gets.
///
/// These are the two types in `core/domain` that are not freezed. They carry
/// bytes and are never compared, sorted or rebuilt from a copy; an
/// `operator ==` over a screenful of output would be a cost without a reader.
library;

import 'dart:typed_data';

import 'diagnostic.dart';

/// What a client sends to the terminal of the sandbox.
sealed class TerminalCommand {
  /// Nothing to build; the variants carry the data.
  const TerminalCommand();
}

/// Attach to the terminal of a running session.
///
/// The first message of every stream, by construction: a resize before it is
/// not the protocol. The geometry is the one of the writer; a reader is
/// always accepted and renders letterboxed (CONVENTIONS 4.10).
final class TerminalOpen extends TerminalCommand {
  /// Attaches to [sandboxId], or to the running session when it is empty.
  const TerminalOpen({
    required this.sandboxId,
    required this.cols,
    required this.rows,
    this.readOnly = false,
  });

  /// The sandbox to attach to; empty means the session that runs.
  final String sandboxId;

  /// Columns of this client.
  final int cols;

  /// Rows of this client.
  final int rows;

  /// Watch only. A second writer is refused with `TERM_001`.
  final bool readOnly;
}

/// Keys of the human, on their way to the agent.
///
/// `Ctrl+C` is byte `0x03` here and not a signal: the sandbox runs with
/// `--new-session` and has no controlling terminal.
final class TerminalKeys extends TerminalCommand {
  /// The bytes as the terminal produced them.
  const TerminalKeys(this.bytes);

  /// What was typed.
  final Uint8List bytes;
}

/// The window of the writer changed.
final class TerminalResize extends TerminalCommand {
  /// The new geometry.
  const TerminalResize({required this.cols, required this.rows});

  /// Columns.
  final int cols;

  /// Rows.
  final int rows;
}

/// Detach. Ends this stream, never the session.
final class TerminalDetach extends TerminalCommand {
  /// Nothing to carry.
  const TerminalDetach();
}

/// What the daemon sends to a terminal client.
sealed class TerminalFrame {
  /// Nothing to build; the variants carry the data.
  const TerminalFrame();
}

/// Filtered bytes of the agent, or a line the daemon wrote itself.
final class TerminalOutput extends TerminalFrame {
  /// The bytes, exactly as they left the daemon.
  const TerminalOutput(this.bytes);

  /// What to write into the emulator.
  final Uint8List bytes;
}

/// The geometry of the writer; a reader renders letterboxed.
final class TerminalGeometry extends TerminalFrame {
  /// The geometry that now holds.
  const TerminalGeometry({required this.cols, required this.rows});

  /// Columns.
  final int cols;

  /// Rows.
  final int rows;
}

/// A finding about this terminal, `TERM_001` above all.
final class TerminalFinding extends TerminalFrame {
  /// The finding, with its reason and, where there is one, its fix.
  const TerminalFinding(this.diagnostic);

  /// What went wrong.
  final Diagnostic diagnostic;
}

/// The agent ended; the stream ends behind this.
final class TerminalExit extends TerminalFrame {
  /// The exit code, or 128 plus the number of the signal.
  const TerminalExit(this.code);

  /// What the agent returned.
  final int code;
}
