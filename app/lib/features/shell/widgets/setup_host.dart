/// Hangs the setup screen into the application (HUM-044).
///
/// The screen itself holds no provider of another feature; this widget is
/// where its inputs and its two gestures are wired, because composing the
/// features is what the shell does (ARCHITECTURE 5).
///
/// It is used twice, and that is deliberate:
///
/// - As the sixth section of the shell, so the header keeps showing held
///   requests and `Ctrl+1` still switches to them while somebody sets up.
/// - Alone, when no daemon answers. There is no shell to put it in then --
///   every section would be an error card -- and the first row carries the
///   finding with its fix (`docs/UX.md` 4.2, case 2).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../sandbox/providers/sandbox_status_provider.dart';
import '../../setup/providers/setup_provider.dart';
import '../../setup/setup_screen.dart';
import '../providers/connection.dart';
import '../providers/navigation.dart';
import '../section.dart';
import '../providers/setup_state.dart';

/// The setup screen with everything it needs.
class SetupHost extends ConsumerWidget {
  /// Creates the host.
  const SetupHost({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Kommt der Daemon von selbst zurueck -- der Zwei-Sekunden-Takt der
    // Verbindung tut das, waehrend jemand im Terminal `humanitl daemon install`
    // faehrt --, dann hat niemand nach der Maschine gefragt, und die Antworten
    // stuenden weiter auf dem Transportfehler des Kaltstarts, bis jemand einen
    // Knopf drueckt. Genau einmal je Rueckkehr, nicht in jedem Frame:
    // `ref.listen` sieht den Uebergang, nicht den Zustand.
    ref.listen<ConnectionStatus>(connectionStateProvider, (
      ConnectionStatus? previous,
      ConnectionStatus next,
    ) {
      if (previous is! ConnectionConnected && next is ConnectionConnected) {
        _remeasure(ref);
      }
    });
    final SetupState state = ref.watch(setupStateProvider);
    final AsyncValue<SandboxStatus> sandbox = ref.watch(sandboxStatusProvider);
    final SandboxStatus status = sandbox.value ?? const SandboxStatus();
    // Ein Start laeuft schon, oder eine Sandbox steht. Beides sperrt den
    // Knopf, und zwar genau so, wie der Kopf des Sandbox-Bildschirms es tut:
    // Zwischen dem Klick und dem ersten `Sandbox(Status)` baut sich nichts
    // neu auf, und ohne diese Sperre schickt ein zweiter Druck ein zweites
    // `Sandbox(Start)`, dessen Ereignisse sich mit denen des ersten in
    // denselben Schnappschuss falten.
    final bool sandboxBusy = status.isUp || status.isBusy;
    return SetupScreen(
      state: state,
      report: ref.watch(setupDoctorProvider).value ?? DoctorReport.empty,
      probe: ref.watch(setupLlmProbeProvider),
      llmEndpoint: status.llmEndpoint,
      workMode: status.workMode,
      sandboxLocked: sandboxBusy,
      onRetry: () {
        // Alles zusammen: Die Verbindung ist die eine Zeile, die nur ein
        // Client beantworten kann, und der Bericht über die Maschine wie der
        // Schnappschuss der Sandbox kommen aus dem Daemon, den es gerade
        // wieder gibt.
        ref.read(connectionStateProvider.notifier).retry();
        _remeasure(ref);
      },
      onRecheck: () =>
          unawaited(ref.read(setupDoctorProvider.notifier).refresh()),
      onProbeLlm: (String endpoint) =>
          unawaited(ref.read(setupLlmProbeProvider.notifier).run(endpoint)),
      onForgetProbe: ref.read(setupLlmProbeProvider.notifier).forget,
      onPickWorkDir: (String dir) => unawaited(
        ref.read(sandboxStatusProvider.notifier).plan(workDir: dir),
      ),
      onWorkMode: (WorkMode mode) => unawaited(
        ref.read(sandboxStatusProvider.notifier).plan(workMode: mode),
      ),
      // Der Start geht denselben Weg wie der Knopf des Sandbox-Bildschirms:
      // `Sandbox(Start)`. Der Wechsel in die Warteschlange kommt aber erst,
      // wenn der Start wirklich gelungen ist.
      //
      // Die Spezifikation nennt die Reihenfolge (`backlog/sprint-3.md`,
      // HUM-044: „Start agent" enabled, `Sandbox(Start)`, Isolation checks,
      // Intercept screen), und der Grund steht in `docs/UX.md` 4.2: Wer in
      // eine leere Warteschlange geworfen wird, weil der Start scheiterte,
      // sieht den Befund nicht, der auf dem Bildschirm hinter ihm stehen
      // bleibt. Scheitert der Start, bleibt die Einrichtung stehen, und die
      // Zeile trägt die Karte des Daemons.
      //
      // `start()` läuft, bis der Strom der Sitzung endet; danach sagt
      // `isUp`, ob eine Sandbox läuft. Ein Fehlschlag hinterlässt entweder
      // einen Befund im Schnappschuss oder einen `AsyncError`, und beide
      // lassen `isUp` falsch.
      onStart: state.canStart && !sandboxBusy
          ? () async {
              await ref.read(sandboxStatusProvider.notifier).start();
              if (!context.mounted) {
                return;
              }
              if (ref.read(sandboxStatusProvider).value?.isUp ?? false) {
                ref.read(navigationProvider.notifier).go(Section.intercept);
              }
            }
          : null,
    );
  }
}

/// Fragt den Daemon noch einmal nach allem, was aus ihm kommt und nicht von
/// selbst nachkommt.
///
/// Beide Antworten scheitern beim Kaltstart am selben Transportfehler, und
/// beide bleiben darauf stehen: `noSetupRetry` und `noSandboxRetry` verbieten
/// den stillen Wiederholungsversuch von Riverpod, weil ein Bildschirm, dessen
/// Aufgabe das Benennen des Fehlers ist, nicht für immer „frage gerade" sagen
/// darf. Der Preis dafür ist, dass jemand fragen muss, sobald der Daemon
/// wieder da ist — das ist diese Funktion.
///
/// Der Schnappschuss der Sandbox gehört dazu und nicht nur der Bericht über
/// die Maschine: Der Projektordner steht in `Sandbox(Status)`, und ohne ihn
/// bliebe die dritte Zeile nach der Rückkehr des Daemons auf dem alten
/// Transportfehler stehen, der Ordner-Knopf sagte „kein Ordner", und der
/// Start bliebe unerreichbar (HUM-044, Akzeptanzkriterium 1).
void _remeasure(WidgetRef ref) {
  unawaited(ref.read(setupDoctorProvider.notifier).refresh());
  unawaited(ref.read(sandboxStatusProvider.notifier).refresh());
}
