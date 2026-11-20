/// The card above the agents (HUM-523): whether the credential files of the
/// depot lie encrypted, as a switch, with the findings of the daemon.
///
/// The card decides nothing itself (ADR-018). The switch shows what the
/// depot says on disk (`CredentialEncryption.enabled`) and calls
/// `Agents.set_encryption`; the daemon checks the keyring, converts the
/// files and answers with the state afterwards. Without a keyring the switch
/// stays off and the finding stands at the card.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agents_provider.dart';
import '../setting_controls.dart';
import '../setting_row.dart';
import 'agent_inset.dart';

/// The card of [encryption].
class CredentialEncryptionCard extends ConsumerStatefulWidget {
  /// Creates the card.
  const CredentialEncryptionCard({required this.encryption, super.key});

  /// The state as `Agents` answered.
  final CredentialEncryption encryption;

  @override
  ConsumerState<CredentialEncryptionCard> createState() =>
      _CredentialEncryptionCardState();
}

class _CredentialEncryptionCardState
    extends ConsumerState<CredentialEncryptionCard> {
  bool _busy = false;
  Diagnostic? _refusal;

  Future<void> _switch(bool on) async {
    setState(() {
      _busy = true;
      _refusal = null;
    });
    final (AgentsStatus? _, Diagnostic? refusal) = await ref
        .read(agentsProvider.notifier)
        .run(AgentsOp.setEncryption(enabled: on));
    if (!mounted) {
      return;
    }
    setState(() {
      _busy = false;
      _refusal = refusal;
    });
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final CredentialEncryption state = widget.encryption;
    final Diagnostic? refusal = _refusal;
    // Die Absage des Daemons steht nur einmal da, wenn der Stand denselben
    // Befund schon nennt.
    final List<Diagnostic> findings = <Diagnostic>[
      ...state.diagnostics,
      if (refusal != null &&
          !state.diagnostics.any(
            (Diagnostic found) =>
                found.code == refusal.code && found.why == refusal.why,
          ))
        refusal,
    ];
    return DecoratedBox(
      key: const Key('agents-encryption'),
      decoration: BoxDecoration(
        color: tokens.colors.bg2,
        border: Border.all(color: tokens.colors.line),
        borderRadius: BorderRadius.circular(tokens.radii.card),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        l10n.settingsAgentsEncryption,
                        style: tokens.typography.ui13.medium.tinted(
                          tokens.colors.fg0,
                        ),
                      ),
                      Text(
                        l10n.settingsAgentsEncryptionHelp,
                        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                      ),
                    ],
                  ),
                ),
                SizedBox(width: SettingRowGrid.gutter),
                HBadge(
                  key: const Key('agents-encryption-state'),
                  text: state.enabled
                      ? l10n.settingsAgentsEncryptionOn
                      : l10n.settingsAgentsEncryptionOff,
                  color: state.enabled
                      ? tokens.state.allowed
                      : tokens.colors.fg2,
                ),
                SizedBox(width: tokens.spacing.x3),
                Semantics(
                  label: l10n.settingsAgentsEncryption,
                  child: OnOffControl(
                    key: const Key('agents-encryption-switch'),
                    value: state.enabled,
                    // Ohne Schlüsselbund bleibt der Schalter aus; ein
                    // eingeschalteter lässt sich immer ausschalten.
                    enabled: !_busy && (state.keyring || state.enabled),
                    onChanged: _switch,
                  ),
                ),
              ],
            ),
            for (int i = 0; i < findings.length; i++) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              AgentFinding(
                key: ValueKey<String>('agents-encryption-finding-$i'),
                diagnostic: findings[i],
              ),
            ],
          ],
        ),
      ),
    );
  }
}
