/// The category Requests: how long a held request waits and where it is
/// asked (HUM-404).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../settings_scope.dart';
import '../widgets/setting_controls.dart';
import '../widgets/setting_row.dart';
import 'egress_category.dart' show egressStatusProvider;

/// `hold.timeout_secs`, written when the field is left or submitted.
class TimeoutRow extends StatefulWidget {
  /// Creates the row for [config].
  const TimeoutRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  State<TimeoutRow> createState() => _TimeoutRowState();
}

class _TimeoutRowState extends State<TimeoutRow> {
  late final TextEditingController _text = TextEditingController(
    text: '${widget.config.holdTimeoutSecs}',
  );

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final String text = _text.text.trim();
    if (text.isEmpty || text == '${widget.config.holdTimeoutSecs}') {
      return;
    }
    await SettingsScope.maybeOf(context)?.write(FormKeys.holdTimeoutSecs, text);
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SettingRow(
      settingId: FormKeys.holdTimeoutSecs,
      refusalKey: const Key('settings-timeout-refusal'),
      child: SizedBox(
        // Eine Zahl von Sekunden, kein Satz: Das Feld ist so breit wie der
        // längste sinnvolle Wert und steht an der rechten Kante.
        width: 120,
        child: Focus(
          onFocusChange: (bool focused) {
            if (!focused) {
              unawaited(_save());
            }
          },
          child: HTextField(
            key: const Key('settings-timeout'),
            controller: _text,
            semanticsLabel: l10n.settingsTimeoutLabel,
            digitsOnly: true,
            onSubmitted: (String _) => unawaited(_save()),
          ),
        ),
      ),
    );
  }
}

/// `hold.ask_mode`: in the app, in the terminal, nowhere, or at the decision
/// proxy behind Humanitl (HUM-512).
///
/// The fourth choice can be made only with an address in `egress.proxy`; the
/// daemon refuses it otherwise, and the row says why the segment is off
/// instead of letting a click run into that refusal.
class AskModeRow extends ConsumerWidget {
  /// Creates the row for [config].
  const AskModeRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    // `host:port` as the daemon reads `egress.proxy` (HUM-511); until its
    // answer arrives, the address as the configuration holds it.
    final String proxy =
        ref.watch(egressStatusProvider).value?.configured.proxy ??
        config.egressProxy;
    final bool upstream = config.holdAskMode == 'upstream';
    return SettingRow(
      settingId: FormKeys.holdAskMode,
      status: <Widget>[
        if (upstream && proxy.isNotEmpty)
          Text(
            l10n.settingsAskModeUpstreamActive(proxy),
            key: const Key('settings-ask-mode-upstream'),
            style: tokens.typography.ui12.tinted(tokens.colors.warningText),
          )
        else if (proxy.isEmpty)
          Text(
            l10n.settingsAskModeUpstreamNeedsProxy,
            key: const Key('settings-ask-mode-needs-proxy'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
      ],
      child: HSegmented<String>(
        key: const Key('settings-ask-mode'),
        options: <HSegmentOption<String>>[
          HSegmentOption<String>(value: 'ui', label: l10n.settingsAskModeUi),
          HSegmentOption<String>(
            value: 'terminal',
            label: l10n.settingsAskModeTerminal,
          ),
          HSegmentOption<String>(
            value: 'none',
            label: l10n.settingsAskModeNone,
          ),
          HSegmentOption<String>(
            value: 'upstream',
            label: l10n.settingsAskModeUpstream,
            semanticsLabel: l10n.settingsAskModeUpstreamSemantics,
            enabled: proxy.isNotEmpty || upstream,
          ),
        ],
        selected: config.holdAskMode,
        onSelect: (String mode) =>
            writeSetting(context, FormKeys.holdAskMode, mode),
      ),
    );
  }
}
