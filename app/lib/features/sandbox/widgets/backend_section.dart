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
class SandboxBackendSection extends ConsumerWidget {
  /// Creates the section for [status].
  const SandboxBackendSection({required this.status, super.key});

  /// What the daemon last said.
  final SandboxStatus status;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    if (status.isUp || status.isBusy) {
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
    final SandboxBackendList? list = ref
        .watch(sandboxBackendsProvider(session))
        .value;
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
          ),
          SizedBox(height: tokens.spacing.x2),
        ],
        SandboxBackendLines(
          list: list,
          keyPrefix: 'sandbox-backend',
          showMinimum: true,
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
