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
import '../../../core/ipc/config_providers.dart';
import '../../sandbox/providers/sandbox_status_provider.dart';
import '../../setup/providers/setup_provider.dart';
import '../../setup/setup_screen.dart';
import '../providers/connection.dart';
import '../providers/navigation.dart';
import '../section.dart';
import '../providers/setup_state.dart';

/// The setup screen with everything it needs.
///
/// # The one contact it makes by itself
///
/// When Setup opens, the host asks the daemon once to contact the model
/// server saved in the configuration (HUM-287, owner decision 2026-10-24):
/// the same `ProbeLlm` as "Contact it", with the stored key, which the daemon
/// sends only to the origin of `llm.endpoint` (HUM-288). Nothing else goes
/// out: no search of the network, no other address, and nothing at all while
/// no endpoint is saved. It asks again on "Check again" and when the saved
/// endpoint changes -- after a settings change or a switch of the saved
/// server -- and never on a timer. That is what lets a machine on which
/// everything is set show "Ready" without a click.
class SetupHost extends ConsumerStatefulWidget {
  /// Creates the host.
  const SetupHost({super.key});

  @override
  ConsumerState<SetupHost> createState() => _SetupHostState();
}

class _SetupHostState extends ConsumerState<SetupHost> {
  @override
  void initState() {
    super.initState();
    // Die Auslöser der einen Probe, die das Setup von selbst macht. Was dann
    // geschieht, entscheidet allein [_contactNow].
    ref.listenManual<Section>(navigationProvider, (
      Section? previous,
      Section next,
    ) {
      if (next == Section.setup && previous != Section.setup) {
        _contactSaved();
      }
    }, fireImmediately: true);
    // Der gespeicherte Endpunkt ändert sich mit den Einstellungen und mit dem
    // Wechsel des gespeicherten Servers. Steht im Feld schon genau die neue
    // Adresse, ist sie ab jetzt die gespeicherte.
    ref.listenManual<String>(setupLlmSavedProvider, (
      String? previous,
      String saved,
    ) {
      if (ref.read(setupLlmTypedProvider) == saved) {
        ref.read(setupLlmTypedProvider.notifier).show(null);
      }
      _contactSaved();
    });
    // Ein anderer Server oder ein anderer Schlüssel: Was gemessen war, ist
    // nicht mehr gemessen, und die Regel fragt neu. Ebenso, sobald eine der
    // drei Antworten ankommt, auf die sie wartet.
    ref.listenManual<SetupLlmContext>(
      setupLlmContextProvider,
      (SetupLlmContext? previous, SetupLlmContext next) => _contactSaved(),
    );
    ref.listenManual<AsyncValue<LlmKeyStatus>>(
      llmKeyProvider,
      (AsyncValue<LlmKeyStatus>? previous, AsyncValue<LlmKeyStatus> next) =>
          _contactSaved(),
    );
    ref.listenManual<AsyncValue<SavedServers>>(
      llmServersProvider,
      (AsyncValue<SavedServers>? previous, AsyncValue<SavedServers> next) =>
          _contactSaved(),
    );
    ref.listenManual<AsyncValue<ConfigValues>>(
      configProvider,
      (AsyncValue<ConfigValues>? previous, AsyncValue<ConfigValues> next) =>
          _contactSaved(),
    );
    // Kehrt das Feld zur gespeicherten Adresse zurück, gilt die Regel wieder.
    ref.listenManual<String?>(setupLlmTypedProvider, (
      String? previous,
      String? next,
    ) {
      if (previous != null && next == null) {
        _contactSaved();
      }
    });
    ref.listenManual<ConnectionStatus>(connectionStateProvider, (
      ConnectionStatus? previous,
      ConnectionStatus next,
    ) {
      if (previous is! ConnectionConnected && next is ConnectionConnected) {
        _contactSaved();
      }
    });
  }

  /// Records which address the field shows, exactly as it stands there:
  /// null while it is the saved one.
  void _show(String text) {
    ref
        .read(setupLlmTypedProvider.notifier)
        .show(text == ref.read(setupLlmSavedProvider) ? null : text);
  }

  /// Applies the rule after this frame; a provider must not change while the
  /// tree is being built. [forced] is "Check again".
  void _contactSaved({bool forced = false}) {
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted) {
        _contactNow(forced: forced);
      }
    });
  }

  /// The rule of the one contact Setup makes by itself (HUM-287).
  ///
  /// Setup is open and the daemon answers, an endpoint is saved, the field
  /// shows it and not an address somebody typed, and nothing is measured or
  /// being measured for exactly what the step shows. Then the saved endpoint
  /// is contacted, once; "Check again" ([forced]) contacts it even over a
  /// result. A typed address is never contacted by itself. A probe still
  /// running for something else is overtaken: only the latest request sets
  /// the result (`SetupLlmProbe`).
  void _contactNow({required bool forced}) {
    if (ref.read(navigationProvider) != Section.setup ||
        ref.read(connectionStateProvider) is! ConnectionConnected) {
      return;
    }
    // Erst wenn Konfiguration, Server und Schlüssel da sind: Eine Probe vor
    // ihnen trüge einen Fingerabdruck, den ihre Ankunft gleich wieder
    // ungültig machte. Ihre Ankunft löst die Regel selbst aus.
    if (ref.read(configProvider).isLoading ||
        ref.read(llmServersProvider).isLoading ||
        ref.read(llmKeyProvider).isLoading) {
      return;
    }
    final String saved = ref.read(setupLlmSavedProvider);
    if (saved.isEmpty || ref.read(setupLlmTypedProvider) != null) {
      return;
    }
    // Gemessen oder in Messung ist genau das, was der Schritt zeigt: nichts
    // zu tun. Eine Probe für etwas anderes wird überholt.
    if (!forced &&
        ref.read(setupLlmProbeProvider).fingerprint ==
            ref.read(setupLlmShownProvider)) {
      return;
    }
    unawaited(ref.read(setupLlmProbeProvider.notifier).run(saved));
  }

  @override
  Widget build(BuildContext context) {
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
    final int attempts = switch (ref.watch(connectionStateProvider)) {
      ConnectionFailed(:final int attempts) ||
      ConnectionFrozen(:final int attempts) => attempts,
      _ => 0,
    };
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
      daemonAttempts: attempts,
      report: ref.watch(setupDoctorProvider).value ?? DoctorReport.empty,
      // Das Feld zeigt eine Messung nur für die Adresse, die in ihm steht.
      probe: ref
          .watch(setupLlmProbeProvider)
          .about(ref.watch(setupLlmShownProvider)),
      // Das Feld zeigt den Endpunkt, den die Einstellungen schreiben, nicht
      // den des letzten Sandbox-Schnappschusses (HUM-287).
      llmEndpoint: ref.watch(setupLlmSavedProvider),
      activeServer: _activeServer(
        ref.watch(llmServersProvider).value ?? SavedServers.none,
      ),
      workMode: status.workMode,
      sandboxLocked: sandboxBusy,
      // Läuft schon eine Session, ist der Weg zur Warteschlange die eine
      // Aktion des fertigen Setups (HUM-287).
      onOpenIntercept: () =>
          ref.read(navigationProvider.notifier).go(Section.intercept),
      onRetry: () {
        // Alles zusammen: Die Verbindung ist die eine Zeile, die nur ein
        // Client beantworten kann, und der Bericht über die Maschine wie der
        // Schnappschuss der Sandbox kommen aus dem Daemon, den es gerade
        // wieder gibt.
        ref.read(connectionStateProvider.notifier).retry();
        _remeasure(ref);
      },
      onRecheck: () {
        unawaited(ref.read(setupDoctorProvider.notifier).refresh());
        _contactSaved(forced: true);
      },
      onProbeLlm: (String endpoint) {
        _show(endpoint);
        unawaited(ref.read(setupLlmProbeProvider.notifier).run(endpoint));
      },
      onAddress: _show,
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

/// The name of the active saved server the model row names, or empty: also
/// empty while the list only derives `default` from the flat keys, because
/// then nobody has saved a server yet (HUM-289).
String _activeServer(SavedServers list) =>
    list.implicit || list.servers.isEmpty ? '' : list.active;
