/// The backend at the top of the isolation tab (HUM-440).
///
/// While a session runs: its backend and isolation level, as the snapshot of
/// the daemon names them. While none runs: the backend of the next start,
/// with `sandbox.backend` as the default, the levels and reasons of every
/// backend the daemon admits, and the lower bound `sandbox.min_isolation`.
/// The choice goes along with the start as `Start.cli_overrides`
/// ([sandboxBackendOverride]); `config.toml` stays as it is.
///
/// Whether a backend can start, its level and the lower bound come from
/// `SandboxBackends`, asked with the plan of this app's snapshots: the lower
/// bound a project profile raises for its folder is in the list already
/// (HUM-440). The section checks nothing of its own (ADR-018).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/sandbox_backends_feed.dart';
import '../../../core/ui/sandbox_backend_choice.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/backend_choice.dart';

/// The section; nothing at all while nothing runs and the daemon lists no
/// backend (an older daemon, no answer yet). With one backend only (macOS)
/// it shows the line and the lower bound without the segments.
///
/// A new choice asks for the list of a start on that backend, and that list
/// has no answer yet the first time. Until it arrives the section keeps the
/// list it showed last, with the new choice already selected, so it neither
/// collapses nor shifts what lies under it (HUM-504). That list belongs to
/// the previous plan: its lines are muted and the segments disabled until
/// the answer for the new choice is there. The same holds when the same list
/// is asked again for another project or configuration (HUM-510).
class SandboxBackendSection extends ConsumerStatefulWidget {
  /// Creates the section for [status].
  const SandboxBackendSection({required this.status, super.key});

  /// What the daemon last said.
  final SandboxStatus status;

  @override
  ConsumerState<SandboxBackendSection> createState() =>
      _SandboxBackendSectionState();
}

class _SandboxBackendSectionState extends ConsumerState<SandboxBackendSection> {
  /// The list the section showed last; it stands in while the list for a
  /// newly chosen backend is still loading.
  SandboxBackendList? _lastShown;

  @override
  Widget build(BuildContext context) {
    final SandboxStatus status = widget.status;
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    if (status.isUp || status.isBusy) {
      // Eine Liste von vor der Sitzung steht danach für keine Wahl mehr ein.
      _lastShown = null;
      if (status.backend.isEmpty) {
        return const SizedBox.shrink();
      }
      return _Block(
        heading: l10n.sandboxBackendCurrent,
        children: <Widget>[
          Text(
            l10n.sandboxBackendLine(
              status.backend,
              isolationLevelText(l10n, status.isolationLevel),
            ),
            key: const Key('sandbox-backend-current'),
            style: tokens.typography.ui13.tinted(tokens.colors.fg0),
          ),
        ],
      );
    }
    // Dieselbe Liste wie für den Plan des Schnappschusses, mit dem Backend
    // der Sitzung (HUM-440).
    final String? session = ref.watch(sessionSandboxBackendProvider);
    final AsyncValue<SandboxBackendList> answer = ref.watch(
      sandboxBackendsProvider(session),
    );
    // Nur solange die Antwort unterwegs ist, steht die alte Liste ein. Ein
    // Fehler (ein Daemon ohne den Aufruf) zeigt nichts, auch wenn Riverpod
    // nach einer gescheiterten Auffrischung den alten Wert noch hält.
    final SandboxBackendList? list = answer.hasError
        ? null
        : answer.value ?? _lastShown;
    _lastShown = list;
    // Die alte Liste gehört zum vorigen Plan, sei es eines anderen Backends
    // (neues Mitglied ohne Wert, HUM-504) oder eines anderen Projekts oder
    // einer anderen Konfiguration (dasselbe Mitglied, neu geladen, HUM-510):
    // Sie hält den Platz, sieht aber nicht aktuell aus und lässt nichts
    // wählen, bis die Antwort für den neuen Plan da ist. Das erneute Fragen
    // nach einem Backend in Prüfung (HUM-502) ändert den Plan nicht und
    // dämpft nichts.
    final bool pending =
        list != null && (!answer.hasValue || sandboxBackendsOutdated(answer));
    if (list == null || list.offered.isEmpty) {
      return const SizedBox.shrink();
    }
    // Ohne Konfiguration keine Auswahl: Das Segment zeigte sonst `auto`
    // neben einer Einstellung, die etwas anderes sagt.
    final String? shown = shownSandboxBackend(
      ref.watch(sandboxBackendChoiceProvider),
      ref.watch(configProvider).value,
      list,
    );
    final bool choosing = shown != null && offersBackendChoice(list);
    return _Block(
      heading: l10n.sandboxBackendSessionLabel,
      children: <Widget>[
        if (shown != null && choosing) ...<Widget>[
          HSegmented<String>(
            key: const Key('sandbox-backend-choice'),
            options: sandboxBackendOptions(l10n, list),
            selected: shown,
            onSelect: ref.read(sandboxBackendChoiceProvider.notifier).choose,
            enabled: !pending,
          ),
          SizedBox(height: tokens.spacing.x2),
        ],
        SandboxBackendLines(
          list: list,
          keyPrefix: 'sandbox-backend',
          showMinimum: true,
          pending: pending,
        ),
        if (choosing) ...<Widget>[
          SizedBox(height: tokens.spacing.x1),
          Text(
            l10n.sandboxBackendSessionHint,
            style: tokens.typography.ui12.tinted(tokens.colors.fg2),
          ),
        ],
      ],
    );
  }
}

/// Heading, content, and the hairline that separates the section from the
/// guarantees under it.
class _Block extends StatelessWidget {
  const _Block({required this.heading, required this.children});

  final String heading;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x3),
      child: Column(
        key: const Key('sandbox-backend'),
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            heading,
            style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x2),
          // Das Maß eines Satzes, nicht die Breite der Fläche, wie beim
          // Einleitungssatz darunter.
          ConstrainedBox(
            constraints: BoxConstraints(
              maxWidth: HSize.measureWidth(tokens.typography.ui13.fontSize!),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: children,
            ),
          ),
          SizedBox(height: tokens.spacing.x3),
          const HHairline(),
        ],
      ),
    );
  }
}
