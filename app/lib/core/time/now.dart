/// Die eine UI-Uhr des Programms: ein Ticker für jeden Countdown, jede
/// Laufzeit und jedes Label, statt eines Timers je Zeile (HUM-020
/// Fallstricke, `docs/UX.md` 7, „Zwei Uhren, nicht eine").
///
/// Sie steht in `core` und nicht mehr in `features/intercept`, seit ein
/// zweites Feature dieselbe Sekunde braucht: Die Laufzeit der Sandbox steht
/// in der Statuszeile des Sandbox-Bildschirms und muss mit der Warteschlange
/// zugleich stehenbleiben, wenn die Verbindung bricht. Ein Feature darf kein
/// anderes importieren (ARCHITECTURE 5), und zwei Uhren nebeneinander wären
/// genau das, was `docs/UX.md` 7 verbietet. Der alte Pfad
/// `features/intercept/providers/now.dart` exportiert diese Datei weiter,
/// damit kein Aufrufer umziehen muss -- derselbe Weg, den
/// `core/ipc/connection.dart` schon gegangen ist.
///
/// Wer die Uhr für einen Teilbaum anhalten will, überschreibt [nowProvider] in
/// einem [ProviderScope]; `features/shell/widgets/frozen_sections.dart` tut
/// genau das. Alles, was seine Sekunde von hier liest, steht damit zugleich
/// still, und nichts, was sie sich selbst nimmt, tut es.
library;

import 'dart:async';

import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'now.g.dart';

/// How often [Now] publishes a new time.
///
/// 250 ms is fast enough for a countdown in `mm:ss` and slow enough that a
/// queue of two hundred rows does not repaint itself to death. Tests override
/// it, golden tests replace [nowProvider] outright so that no timer runs.
@Riverpod(keepAlive: true)
Duration nowInterval(Ref ref) => const Duration(milliseconds: 250);

/// The current time, republished every [nowIntervalProvider].
///
/// The timer starts with the first watcher and stops with the last one,
/// because the provider is disposed with the scope that holds it.
///
/// A widget that only needs whole seconds -- an uptime, a countdown label --
/// watches it through a `.select` that projects the value it draws, so a
/// screen is rebuilt once a second and not four times (`docs/UX.md` 7).
@Riverpod(keepAlive: true)
class Now extends _$Now {
  @override
  DateTime build() {
    final Timer timer = Timer.periodic(
      ref.watch(nowIntervalProvider),
      (Timer _) => state = DateTime.now(),
    );
    ref.onDispose(timer.cancel);
    return DateTime.now();
  }
}
