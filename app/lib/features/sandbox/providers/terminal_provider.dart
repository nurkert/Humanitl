/// The terminal of the running session, and the strip above it (HUM-042).
///
/// Two providers, because they answer two questions from two sources:
///
/// * [TerminalSession] holds the emulator and the one bidirectional stream to
///   the daemon. What arrives has been filtered there, once, for every client;
///   this app adds no second filter and registers no OSC handler
///   (`docs/SECURITY.md` 3.3).
/// * [HeldNotice] watches the flow events every screen already watches and
///   says what the agent is waiting for. It is deliberately **not** read out
///   of the terminal stream: a full-screen agent redraws over that line with
///   its next frame, and the sentence a human needs must not depend on what
///   the agent does next.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:xterm2/core.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/flow_events.dart';

part 'terminal_provider.g.dart';

/// How many lines the emulator keeps.
///
/// Ten thousand is what the contract of the pane promises and what the daemon
/// cannot: its ring buffer holds the last 64 KiB and is there for a client
/// that attaches late, not for a human scrolling back through an afternoon.
const int terminalScrollback = 10000;

/// How many flows the strip remembers by their request line.
///
/// A held flow is announced twice -- `received` carries the request, `held`
/// carries only the id -- so the strip has to keep the first to say anything
/// about the second. Sixty-four is far more than can be held at once
/// (`limits.hold_max_flows`) and small enough that a long session does not
/// turn this into a second history.
const int heldNoticeMemory = 64;

/// Where a terminal session stands.
enum TerminalPhase {
  /// Nothing attached yet.
  idle,

  /// The stream is open; the agent writes.
  attached,

  /// The daemon refused this client, and [TerminalSessionState.diagnostic] says why.
  refused,

  /// The agent ended, and [TerminalSessionState.exitCode] carries its code.
  ended,
}

/// What the pane needs to know about its session.
class TerminalSessionState {
  /// A state at [phase].
  const TerminalSessionState({
    required this.terminal,
    this.phase = TerminalPhase.idle,
    this.diagnostic,
    this.exitCode,
    this.cols = 0,
    this.rows = 0,
    this.readOnly = false,
  });

  /// The emulator. It survives a rebuild, so the screen keeps its scrollback.
  final Terminal terminal;

  /// Where the session stands.
  final TerminalPhase phase;

  /// Why the daemon refused, when it did.
  final Diagnostic? diagnostic;

  /// What the agent returned, when it ended.
  final int? exitCode;

  /// The geometry of the writer, as the daemon last confirmed it.
  final int cols;

  /// Rows of the writer.
  final int rows;

  /// Whether this client gave up the keyboard when it attached.
  ///
  /// The daemon drops the keys of a reader, and it is right to do so -- but a
  /// pane that keeps taking them shows a cursor waiting for input that goes
  /// nowhere. The emulator is told, so that it stops asking.
  final bool readOnly;

  /// The same state at another phase.
  TerminalSessionState copyWith({
    TerminalPhase? phase,
    Diagnostic? diagnostic,
    int? exitCode,
    int? cols,
    int? rows,
    bool? readOnly,
  }) => TerminalSessionState(
    terminal: terminal,
    phase: phase ?? this.phase,
    diagnostic: diagnostic ?? this.diagnostic,
    exitCode: exitCode ?? this.exitCode,
    cols: cols ?? this.cols,
    rows: rows ?? this.rows,
    readOnly: readOnly ?? this.readOnly,
  );
}

/// The terminal of one sandbox: the emulator, and the stream that feeds it.
@Riverpod(keepAlive: true)
class TerminalSession extends _$TerminalSession {
  StreamController<TerminalCommand>? _input;
  StreamSubscription<TerminalFrame>? _frames;
  ByteConversionSink? _decoder;

  @override
  TerminalSessionState build(String sandboxId) {
    final Terminal terminal = Terminal(maxLines: terminalScrollback);
    // What the human types goes up as bytes; what the daemon sends comes back
    // through the same pipe. `onOutput` is the keyboard, not the screen.
    terminal.onOutput = (String data) =>
        _send(TerminalKeys(Uint8List.fromList(utf8.encode(data))));
    terminal.onResize = (
      int width,
      int height,
      int pixelWidth,
      int pixelHeight,
    ) => _send(TerminalResize(cols: width, rows: height));
    ref.onDispose(_detach);
    return TerminalSessionState(terminal: terminal);
  }

  /// Opens the stream. Idempotent: a second call while attached does nothing.
  ///
  /// [readOnly] gives up the keyboard before the daemon has to refuse it. A
  /// second writing client would get `TERM_001` and its stream would end;
  /// asking for a reader instead is how a second window watches a session.
  Future<void> attach({bool readOnly = false}) async {
    if (_input != null) {
      return;
    }
    state = state.copyWith(readOnly: readOnly);
    final Terminal terminal = state.terminal;
    final StreamController<TerminalCommand> input =
        StreamController<TerminalCommand>();
    _input = input;
    _decoder = const Utf8Decoder(allowMalformed: true).startChunkedConversion(
      // The decoder keeps the tail of a character that a chunk cut in half.
      // Without it every multi-byte character on a chunk boundary would show
      // up as two replacement characters.
      _TerminalSink(terminal.write),
    );
    input.add(
      TerminalOpen(
        sandboxId: sandboxId,
        cols: terminal.viewWidth,
        rows: terminal.viewHeight,
        readOnly: readOnly,
      ),
    );
    final DaemonClient client = ref.read(daemonClientProvider);
    _frames = client
        .terminal(input.stream)
        .listen(_onFrame, onError: _onError, onDone: _onDone);
  }

  /// Detaches this client. The session keeps running.
  Future<void> detach() async {
    _input?.add(const TerminalDetach());
    await _detach();
  }

  void _send(TerminalCommand command) {
    final StreamController<TerminalCommand>? input = _input;
    if (input != null && !input.isClosed) {
      input.add(command);
    }
  }

  void _onFrame(TerminalFrame frame) {
    switch (frame) {
      case TerminalOutput(bytes: final Uint8List bytes):
        _decoder?.add(bytes);
      case TerminalGeometry(cols: final int cols, rows: final int rows):
        state = state.copyWith(
          phase: TerminalPhase.attached,
          cols: cols,
          rows: rows,
        );
      case TerminalFinding(diagnostic: final Diagnostic diagnostic):
        state = state.copyWith(
          phase: TerminalPhase.refused,
          diagnostic: diagnostic,
        );
      case TerminalExit(code: final int code):
        state = state.copyWith(phase: TerminalPhase.ended, exitCode: code);
    }
  }

  void _onError(Object error) {
    if (error is DaemonException) {
      state = state.copyWith(
        phase: TerminalPhase.refused,
        diagnostic: error.diagnostic,
      );
    }
  }

  void _onDone() {
    if (state.phase == TerminalPhase.attached) {
      state = state.copyWith(phase: TerminalPhase.idle);
    }
    unawaited(_detach());
  }

  /// Gives up the stream. Never waited on.
  ///
  /// Cancelling a subscription waits for the source to acknowledge it, and
  /// the source here is a network stream: a daemon that says nothing would
  /// hold the provider -- and with it the screen -- in its disposal. What is
  /// gone is gone, and the transport closes on its own.
  Future<void> _detach() async {
    final StreamSubscription<TerminalFrame>? frames = _frames;
    final StreamController<TerminalCommand>? input = _input;
    _frames = null;
    _input = null;
    _decoder?.close();
    _decoder = null;
    unawaited(frames?.cancel());
    unawaited(input?.close());
  }
}

/// Writes decoded text into the emulator.
class _TerminalSink implements Sink<String> {
  _TerminalSink(this._write);

  final void Function(String) _write;

  @override
  void add(String data) => _write(data);

  @override
  void close() {}
}

/// What the agent is waiting for, for the strip above the terminal.
class TerminalNotice {
  /// A notice about one held flow.
  const TerminalNotice({
    required this.flowId,
    required this.method,
    required this.host,
    required this.path,
  });

  /// The flow that waits.
  final FlowId flowId;

  /// Its method.
  final String method;

  /// Its host.
  final String host;

  /// Its path, as it came from the wire.
  final String path;
}

/// The newest held flow, or null when nothing waits.
///
/// The strip is the acceptance criterion of HUM-042 and the line in the byte
/// stream is not: a full-screen TUI redraws with absolute addressing, so a
/// `\r\n[humanitl] …\r\n` in the same stream lands wherever the cursor
/// happens to be and is gone with the next frame.
@Riverpod(keepAlive: true)
class HeldNotice extends _$HeldNotice {
  final Map<FlowId, TerminalNotice> _seen = <FlowId, TerminalNotice>{};

  @override
  TerminalNotice? build() {
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      final FlowEvent? event = next.value;
      if (event != null) {
        _apply(event);
      }
    });
    return null;
  }

  void _apply(FlowEvent event) {
    switch (event) {
      case FlowEventReceived(flow: final Flow flow):
        if (_seen.length >= heldNoticeMemory) {
          _seen.remove(_seen.keys.first);
        }
        _seen[flow.id] = TerminalNotice(
          flowId: flow.id,
          method: flow.methodRaw.isEmpty
              ? flow.method.name.toUpperCase()
              : flow.methodRaw,
          host: flow.authority.displayHost.isEmpty
              ? flow.authority.host
              : flow.authority.displayHost,
          path: flow.path,
        );
      case FlowEventHeld(flowId: final FlowId id):
        final TerminalNotice? notice = _seen[id];
        if (notice != null) {
          state = notice;
        }
      // A decision ends the wait, whoever made it. The strip goes away rather
      // than turning into a log: what was decided stands in the history.
      case FlowEventDecided(flowId: final FlowId id):
        if (state?.flowId == id) {
          state = null;
        }
        _seen.remove(id);
      case FlowEventTimedOut(flowId: final FlowId id):
        if (state?.flowId == id) {
          state = null;
        }
        _seen.remove(id);
      default:
        break;
    }
  }
}
