/// The row of `sandbox.backend` in the category Sandbox (HUM-440): automatic
/// or one of the backends the daemon admits, each with its isolation level,
/// and for one that cannot start here the daemon's reason and fix.
///
/// The row only writes the word through `SetConfig`. Which backends exist,
/// which of them could start, their levels and the reasons all come from
/// `SandboxBackends` (`sandboxBackendsProvider`); the row checks nothing of
/// its own (ADR-018).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/sandbox_backends_feed.dart';
import '../../../core/ui/sandbox_backend_choice.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../settings_scope.dart';
import 'setting_controls.dart';
import 'setting_row.dart';

/// The row. While the daemon has not answered it shows name and help only;
/// a daemon that cannot list its backends gets its reason under the help
/// and no control.
class SandboxBackendRow extends ConsumerStatefulWidget {
  /// Creates the row for [config].
  const SandboxBackendRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  /// The key of the whole row, in every state: a `ChangeSetting` on
  /// `sandbox.backend` lands here even while the daemon has not listed its
  /// backends yet and the row has no control.
  static const Key rowKey = Key('settings-sandbox-backend-row');

  @override
  ConsumerState<SandboxBackendRow> createState() => _SandboxBackendRowState();
}

class _SandboxBackendRowState extends ConsumerState<SandboxBackendRow> {
  /// Umschließt die Segmente: Hat einer von ihnen den Fokus, hat ihn auch
  /// dieser Knoten.
  final FocusNode _choice = FocusNode(
    debugLabel: 'sandbox-backend-choice',
    canRequestFocus: false,
    skipTraversal: true,
  );

  /// Ob die Segmente den Fokus hatten, als das Neuladen sie sperrte.
  bool _refocusAfterReload = false;

  @override
  void dispose() {
    _choice.dispose();
    super.dispose();
  }

  /// Gibt den Fokus nach dem Neuladen dem gewählten Segment zurück, oder
  /// dem ersten, wenn keines gewählt ist.
  void _restoreFocus() {
    final List<FocusNode> segments = _choice.traversalDescendants.toList();
    if (segments.isEmpty) {
      return;
    }
    final FocusNode target = segments.firstWhere(
      (FocusNode node) =>
          node.context
              ?.findAncestorWidgetOfExactType<HSegment<String>>()
              ?.selected ??
          false,
      orElse: () => segments.first,
    );
    target.requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    final ConfigValues config = widget.config;
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Die Liste für die Einstellung: ohne Backend der Sitzung (HUM-440).
    final AsyncValue<SandboxBackendList> answer = ref.watch(
      sandboxBackendsProvider(null),
    );
    // Die Liste kommt später als ein Sprung hierher (Abhilfe „Einstellungen
    // öffnen“): Kommt sie, erhält das Feld den Fokus, den der Sprung ihm
    // geben wollte.
    ref.listen<AsyncValue<SandboxBackendList>>(sandboxBackendsProvider(null), (
      AsyncValue<SandboxBackendList>? before,
      AsyncValue<SandboxBackendList> now,
    ) {
      // Das Neuladen nach einer Änderung der Konfiguration sperrt die
      // Segmente, und ein gesperrtes Segment verliert den Fokus; wer eben
      // mit der Tastatur gewählt hat, bekommt ihn danach zurück (HUM-510).
      final bool wasOutdated =
          before != null && sandboxBackendsOutdated(before);
      final bool isOutdated = sandboxBackendsOutdated(now);
      if (!wasOutdated && isOutdated) {
        _refocusAfterReload = _choice.hasFocus;
      } else if (wasOutdated && !isOutdated && _refocusAfterReload) {
        _refocusAfterReload = false;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) {
            _restoreFocus();
          }
        });
      }
      if (before?.value == null && now.value != null) {
        final SettingsController? scope = SettingsScope.maybeOf(context);
        WidgetsBinding.instance.addPostFrameCallback((_) {
          // Die Zeile kann bis zum nächsten Bild verschwunden sein.
          if (context.mounted) {
            scope?.refocus(FormKeys.sandboxBackend);
          }
        });
      }
    });
    // Ein Fehler gewinnt, auch wenn Riverpod nach einem gescheiterten
    // Neuladen (etwa für ein anderes Projekt) die alte Liste noch hält: Sie
    // gehört zum vorigen Plan und stünde sonst als aktuell da (HUM-510).
    final SandboxBackendList? list = answer.hasError ? null : answer.value;
    if (list == null) {
      final Object? error = answer.error;
      return SettingRow(
        key: SandboxBackendRow.rowKey,
        settingId: FormKeys.sandboxBackend,
        status: <Widget>[
          if (error != null)
            Text(
              l10n.sandboxBackendListFailed(
                DiagnosticL10n.resolve(
                  DaemonConnection.diagnosticOf(error),
                  l10n,
                ).cause,
              ),
              key: const Key('settings-sandbox-backend-failed'),
              style: tokens.typography.ui12.tinted(tokens.colors.warningText),
            ),
        ],
        child: const SizedBox.shrink(),
      );
    }
    final String written = config.sandboxBackend.trim();
    // Nach einer Änderung der Konfiguration steht bis zur neuen Antwort die
    // Liste des vorigen Plans da: gedämpft und ohne Wahl (HUM-510).
    final bool pending = sandboxBackendsOutdated(answer);
    return SettingRow(
      key: SandboxBackendRow.rowKey,
      settingId: FormKeys.sandboxBackend,
      refusalKey: const Key('settings-sandbox-backend-refusal'),
      // Die Zeilen der Backends samt Grund und Befehl stehen unter Wahl und
      // Hilfe in voller Breite (HUM-494): Neben vier Segmenten blieb ihnen
      // ein Drittel der Zeile, der Grund lief über ein Dutzend Zeilen und
      // der Befehl zum Kopieren schrumpfte auf einen Streifen.
      footer: <Widget>[
        SandboxBackendLines(
          list: list,
          keyPrefix: 'settings-sandbox-backend',
          pending: pending,
        ),
      ],
      child: offersBackendChoice(list)
          ? Focus(
              focusNode: _choice,
              child: HSegmented<String>(
                key: const Key('settings-sandbox-backend'),
                options: sandboxBackendOptions(l10n, list),
                selected: written,
                onSelect: (String backend) {
                  if (backend != written) {
                    writeSetting(context, FormKeys.sandboxBackend, backend);
                  }
                },
                enabled: !pending,
              ),
            )
          : const SizedBox.shrink(),
    );
  }
}
