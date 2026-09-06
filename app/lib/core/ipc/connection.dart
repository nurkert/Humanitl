/// Was `GetInfo` gesagt hat, ob die Shell zeigen darf und ob die Verbindung
/// gerade lebt (`daemonInfoProvider`, `connectionStateProvider`,
/// `linkLiveProvider`; HUM-019 Spezifikation, HUM-044).
///
/// Die Datei steht in `core/ipc` und nicht mehr in der Shell, seit die
/// Warteschlange dieselbe Frage stellen muss wie der Rahmen um sie herum:
/// „Kommt das, was auf dem Schirm steht, noch vom Daemon?“ Die Antwort ist
/// [linkLiveProvider], und sie wird abgeleitet und nicht weitergereicht. Ein
/// Feature darf kein anderes importieren (ARCHITECTURE 5), also gäbe es
/// sonst zwei Wände: eine in der Shell und eine gespiegelte in der
/// Warteschlange, und die beiden könnten auseinanderlaufen. Der alte Pfad
/// `features/shell/providers/connection.dart` exportiert diese Datei weiter,
/// damit kein Aufrufer umziehen muss.
///
/// The version check lives here, not in the client: the fake reports whatever
/// it is told, and the app decides what it accepts.
library;

import 'dart:async';

import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../domain/domain.dart';
import 'client_diagnostics.dart';
import 'client_providers.dart';
import 'daemon_client.dart';
import 'proto_version.dart';

part 'connection.freezed.dart';
part 'connection.g.dart';

/// How often the connection is confirmed with a `GetInfo` while connected,
/// or null for never. Two seconds keeps "daemon stopped" under the five
/// seconds HUM-019 asks for; tests override it.
@Riverpod(keepAlive: true)
Duration? connectionHeartbeat(Ref ref) => const Duration(seconds: 2);

/// How often a failed connection is tried again, or null for never.
///
/// **This reverses, for the red case only, the decision documented right
/// below.** Reconnecting stays explicit everywhere a person is working; but
/// the first screen of this product is a checklist whose first line is "does
/// the background service answer?", and the fix for that line is a command
/// somebody runs in a terminal. A line that stays red until the person comes
/// back and presses a button would teach them that the command did not work
/// (HUM-044). Two seconds is what keeps that line green within four.
///
/// It costs one `GetInfo` attempt against a socket nobody is listening on --
/// a `connect` that fails immediately, no traffic, nothing on the network. It
/// runs only while the connection is down, never while it stands; tests pass
/// null and drive the clock themselves.
@Riverpod(keepAlive: true)
Duration? connectionReconnect(Ref ref) => const Duration(seconds: 2);

/// Riverpod 3 retries a failed provider on its own (backoff from 200 ms, ten
/// attempts) and reports `AsyncLoading` with the error tucked inside while it
/// does, so the gate would never see the failure. Reconnecting is explicit
/// here: the button and the palette command call [DaemonConnection.retry];
/// the heartbeat only notices a daemon that went away.
///
/// The one exception is the red case, and it is deliberate: while there is no
/// usable daemon, [connectionReconnectProvider] tries again on its own, so the
/// first line of the setup screen turns green on its own once somebody starts
/// the service. See its comment for why that is not the same decision.
Duration? noConnectionRetry(int retryCount, Object error) => null;

/// `GetInfo` plus the version check.
///
/// Retry with `ref.invalidate(daemonInfoProvider)` -- what
/// [DaemonConnection.retry] does. Riverpod never retries this provider on its
/// own ([noConnectionRetry]); while there is no usable daemon, the timer of
/// [connectionReconnectProvider] invalidates it the same way every two
/// seconds, so the invalidation is the only way back either way. The one
/// difference is that the timer does not announce itself on the daemon row,
/// because nobody asked for it. See the comment on that provider for why the
/// red case is the one exception.
@Riverpod(retry: noConnectionRetry)
Future<DaemonInfo> daemonInfo(Ref ref) async {
  final DaemonInfo info = await ref.watch(daemonClientProvider).getInfo();
  if (!ProtoVersion.isCompatible(info.protoMajor)) {
    throw DaemonException(ClientDiagnostics.protoIncompatible(info));
  }
  return info;
}

/// The four states of the connection gate.
///
/// Two of them are failures, and the difference between them is the whole
/// point: a cold start without a daemon has no shell to keep, while a
/// connection that stood and broke has twelve requests on screen that nobody
/// may take away (`docs/UX.md` 4.2, cases 2 and 4).
@freezed
sealed class ConnectionStatus with _$ConnectionStatus {
  /// `GetInfo` is in flight, and no connection has stood yet.
  const factory ConnectionStatus.connecting() = ConnectionConnecting;

  /// The daemon answered and is compatible.
  const factory ConnectionStatus.connected({required DaemonInfo info}) =
      ConnectionConnected;

  /// No daemon has answered yet in this run; the setup screen shows
  /// [diagnostic] instead of the shell.
  ///
  /// [retrying] is true while an attempt somebody asked for is in flight. It
  /// is a field of the failure and not a state of its own, and the reason is
  /// the whole of `_breakOrElse`: the screen keeps standing with its finding
  /// on it while the attempt runs.
  const factory ConnectionStatus.failed({
    required Diagnostic diagnostic,
    @Default(false) bool retrying,
  }) = ConnectionFailed;

  /// A connection that stood and broke: the shell stays, and the queue in it
  /// is a snapshot.
  ///
  /// [info] is the last thing `GetInfo` said, so the shell still has a status
  /// bar; [diagnostic] is why the connection went away, and the banner above
  /// the sections says it.
  ///
  /// [retrying] means the same as it does on [ConnectionFailed]: an attempt
  /// somebody asked for is in flight, and the banner keeps standing while it
  /// runs.
  const factory ConnectionStatus.frozen({
    required Diagnostic diagnostic,
    required DaemonInfo info,
    @Default(false) bool retrying,
  }) = ConnectionFrozen;

  const ConnectionStatus._();

  /// The daemon description while it is known, otherwise null.
  ///
  /// A frozen connection keeps it: it is the last measurement, not a current
  /// one, and everything drawn from it stands still while the banner says so.
  DaemonInfo? get info => switch (this) {
    ConnectionConnected(:final info) || ConnectionFrozen(:final info) => info,
    _ => null,
  };
}

/// `connectionStateProvider`: derives [ConnectionStatus] from
/// [daemonInfoProvider] and, while connected, keeps confirming the daemon
/// with a heartbeat so that a stopped daemon shows up as a failure.
///
/// It is also what decides which of the two failures a failure is, and that
/// decision needs one thing the providers do not carry: whether a connection
/// has ever stood in this run. [_known] is that memory. Once `GetInfo` has
/// answered, every later failure is a break in the middle of the work, the
/// shell stays up, and the last [DaemonInfo] travels through the failure so
/// there is something to render (`docs/UX.md` 4.2, case 4).
@Riverpod(keepAlive: true, name: 'connectionStateProvider')
class DaemonConnection extends _$DaemonConnection {
  Timer? _heartbeat;
  Timer? _reconnect;

  /// The last thing `GetInfo` answered, or null while nothing has.
  DaemonInfo? _known;

  /// Why the connection that stood went away, or null while it stands.
  Diagnostic? _broke;

  /// True while an attempt somebody asked for has not answered yet.
  ///
  /// Set by [retry] -- the button on the daemon row, the action in the banner,
  /// the palette command -- and cleared as soon as the attempt ends, in
  /// [_connected] or [_failed]. The two-second timer of
  /// [connectionReconnectProvider] never sets it: nobody asked for it, and a
  /// screen that redrew its first row every two seconds would flicker for a
  /// probe that is meant to be invisible (see the comment on that provider).
  bool _requested = false;

  @override
  ConnectionStatus build() {
    ref.onDispose(_stopTimers);
    final AsyncValue<DaemonInfo> info = ref.watch(daemonInfoProvider);
    // `isLoading` first: after `retry` Riverpod reports the new attempt with
    // the previous result still attached, and the gate shows the splash,
    // not the stale outcome.
    return switch (info) {
      AsyncValue(isLoading: true) => _connecting(),
      AsyncData(:final value) => _connected(value),
      AsyncError(:final error) => _failed(diagnosticOf(error)),
      // `AsyncLoading` is caught above; the analyzer cannot see that.
      _ => _connecting(),
    };
  }

  /// Tries again, because somebody asked: the attempt is announced on the
  /// failure it is trying to end, and whatever `GetInfo` says replaces it.
  void retry() {
    _requested = true;
    _attempt();
  }

  /// One `GetInfo`, without saying that anybody asked for it. The two-second
  /// timer takes this door.
  void _attempt() => ref.invalidate(daemonInfoProvider);

  ConnectionStatus _connecting() {
    // Während ein Versuch läuft, läuft kein zweiter: Ein Timer, der in eine
    // offene Anfrage hineinruft, häuft Versuche an, statt zu wiederholen.
    _stopTimers();
    // Ein Versuch, wieder hereinzukommen, ist kein Kaltstart. Der Splash
    // gehört dem ersten Versuch dieses Laufs; wer schon einen Grund vor sich
    // hat, bekommt ihn nicht alle zwei Sekunden neu, während der
    // Zwei-Sekunden-Takt es wieder versucht (`docs/UX.md` 4.2, Fälle 2 und 4).
    return _breakOrElse(const ConnectionStatus.connecting());
  }

  ConnectionStatus _connected(DaemonInfo info) {
    _stopReconnect();
    _startHeartbeat();
    _known = info;
    _broke = null;
    _requested = false;
    return ConnectionStatus.connected(info: info);
  }

  ConnectionStatus _failed(Diagnostic diagnostic) {
    _stopHeartbeat();
    _startReconnect();
    _broke = diagnostic;
    _requested = false;
    final DaemonInfo? known = _known;
    if (known == null) {
      // Kaltstart: Es gibt keine Shell, die stehen bleiben könnte, und der
      // Setup-Bildschirm ist der ganze Bildschirm (`docs/UX.md` 4.2, Fall 2).
      return ConnectionStatus.failed(diagnostic: diagnostic);
    }
    return ConnectionStatus.frozen(diagnostic: diagnostic, info: known);
  }

  /// Der Zustand, den ein bereits gemeldeter Fehlschlag festhält, sonst
  /// [otherwise].
  ///
  /// Er gilt für beide Fehlschläge, und das ist der Unterschied zwischen einem
  /// Splash beim ersten Start und einem Splash alle zwei Sekunden. Solange
  /// noch nie etwas gescheitert ist, gibt es nichts festzuhalten, und der
  /// nächste Versuch ist der erste: der Splash gehört ihm. Sobald ein Grund
  /// auf dem Schirm steht -- der Setup-Bildschirm beim Kaltstart, das Banner
  /// mitten in der Arbeit --, bleibt dieser Bildschirm über jedem weiteren
  /// Versuch stehen. Sonst nähme der Zwei-Sekunden-Takt genau den Bildschirm
  /// weg, auf dem der Befund und seine Abhilfe stehen, samt dem Knopf, der
  /// `humanitl daemon install` gerade fährt (`docs/UX.md` 4.2, Fälle 2 und 4,
  /// HUM-044 Akzeptanzkriterium 1).
  ///
  /// # Welcher Bildschirm, und was darauf steht, sind zwei Fragen
  ///
  /// Das Festhalten beantwortet die erste und darf die zweite nicht
  /// mitbeantworten. **Welcher Bildschirm** gezeigt wird, entscheidet die Art
  /// des Zustands, und die bleibt über jeden weiteren Versuch dieselbe: der
  /// Setup-Bildschirm beim Kaltstart, die Shell mit ihrem Banner mitten in der
  /// Arbeit. **Was auf dem Bildschirm steht**, ist davon unabhängig: Solange
  /// ein Versuch läuft, den jemand ausgelöst hat, sagt die Zeile des Dienstes
  /// „asking" und ihr Knopf ruht, damit niemand einen zweiten Versuch auf
  /// einen laufenden setzt ([_requested], `daemonLinkOf`). Der Befund und
  /// seine Abhilfe bleiben dabei stehen, denn genau das ist der Bildschirm,
  /// von dem aus jemand den Dienst repariert.
  ///
  /// Beides zusammen geht nur, weil der laufende Versuch ein Feld des
  /// Fehlschlags ist und kein eigener Zustand. Ein zurückgegebenes
  /// [ConnectionConnecting] beantwortete beide Fragen auf einmal falsch: das
  /// Gate zeigte den Splash, und das Banner der eingefrorenen Shell wäre für
  /// die Dauer des Versuchs verschwunden.
  ConnectionStatus _breakOrElse(ConnectionStatus otherwise) {
    final Diagnostic? broke = _broke;
    if (broke == null) {
      return otherwise;
    }
    final DaemonInfo? known = _known;
    return known == null
        ? ConnectionStatus.failed(diagnostic: broke, retrying: _requested)
        : ConnectionStatus.frozen(
            diagnostic: broke,
            info: known,
            retrying: _requested,
          );
  }

  void _startReconnect() {
    _stopReconnect();
    final Duration? interval = ref.read(connectionReconnectProvider);
    if (interval != null) {
      // `_attempt` und nicht `retry`: Der Takt läuft von selbst, also kündigt
      // er sich auf der Zeile des Dienstes auch nicht an.
      _reconnect = Timer.periodic(interval, (_) => _attempt());
    }
  }

  void _stopReconnect() {
    _reconnect?.cancel();
    _reconnect = null;
  }

  void _stopTimers() {
    _stopHeartbeat();
    _stopReconnect();
  }

  void _startHeartbeat() {
    _stopHeartbeat();
    final Duration? interval = ref.read(connectionHeartbeatProvider);
    if (interval != null) {
      _heartbeat = Timer.periodic(interval, (_) => _beat());
    }
  }

  void _stopHeartbeat() {
    _heartbeat?.cancel();
    _heartbeat = null;
  }

  Future<void> _beat() async {
    try {
      final DaemonInfo info = await ref.read(daemonClientProvider).getInfo();
      if (!ref.mounted) {
        return;
      }
      if (!ProtoVersion.isCompatible(info.protoMajor)) {
        _fail(ClientDiagnostics.protoIncompatible(info));
      } else if (state case ConnectionConnected(info: final known)
          when known != info) {
        state = ConnectionStatus.connected(info: info);
      }
    } on Object catch (error) {
      if (ref.mounted) {
        _fail(diagnosticOf(error));
      }
    }
  }

  void _fail(Diagnostic diagnostic) {
    // Derselbe Weg wie im Aufbau: Der Herzschlag hört auf, und der Versuch,
    // wieder hereinzukommen, fängt an.
    state = _failed(diagnostic);
  }

  /// The diagnostic behind [error]: the one a [DaemonException] carries, or
  /// a `DAEMON_001` describing anything else that reached the gate.
  static Diagnostic diagnosticOf(Object error) => switch (error) {
    DaemonException(:final diagnostic) => diagnostic,
    _ => ClientDiagnostics.daemonUnreachable(
      socketPath: '?',
      detail: error.toString(),
    ),
  };
}

/// Die eine Frage, die das ganze Programm über die Verbindung stellt: lebt
/// sie?
///
/// Genau ein Zustand heißt ja, [ConnectionConnected]. Der erste Versuch, der
/// Kaltstart ohne Daemon und der Bruch mitten in der Arbeit heißen alle drei
/// nein, und sie heissen dasselbe: Was auf dem Schirm steht, kommt nicht mehr
/// vom Daemon, und nichts, was jemand jetzt tut, käme dort an.
///
/// Es gibt diesen Provider, damit die Frage einmal gestellt wird und nicht
/// fünfmal verschieden. Vor HUM-044 fragte die Statuszeile gar nicht, der
/// Kopf fragte den Sandbox-Status, die Tray-Brücke schrieb
/// `is ConnectionConnected` selbst hin und die Shell fragte nach
/// `is ConnectionFrozen`; jede Stelle hatte ihre eigene Antwort, und
/// drei davon waren falsch.
///
/// **Er antwortet für `GetInfo`, und das ist Absicht.** Über denselben Socket
/// läuft ein zweiter Aufruf, `Subscribe`, mit einer eigenen Rückfallzeit. Ihn
/// in diese Frage hineinzunehmen hieße, die Shell wegen eines Stolperers im
/// Ereignisstrom einzufrieren, obwohl der Daemon antwortet. Statt dessen folgt
/// der Strom dieser Antwort: `flowEventsProvider` horcht auf diesen Provider
/// und verbindet sich sofort neu, sobald er ja sagt, damit „lebendig" nicht
/// eine halbe Minute lang „lebendig und taub" bedeutet.
@Riverpod(keepAlive: true)
bool linkLive(Ref ref) =>
    ref.watch(connectionStateProvider) is ConnectionConnected;
