/// Die Pause beim Senden mit offenen Funden (HUM-049).
///
/// Sie steht in der Karte an der Stelle der Entscheidungsleiste, nicht über
/// ihr: kein Modal, keine Ebene, nichts, was sich wegklicken ließe, ohne die
/// Liste gelesen zu haben (`docs/UX.md` 4.7, 5.4). Oben steht, wie viele Funde
/// die Anfrage trägt, darunter je Fund Art, gekürzter Wert und Ort, und am
/// Fuß die drei Wege weiter: trotzdem senden, pseudonymisieren, blockieren.
///
/// Das Widget entscheidet nichts. Jeder Knopf ruft, was der Aufrufer
/// hineinreicht; die Aktionsleiste leitet das an denselben Notifier weiter wie
/// jede andere Entscheidung (ADR-018).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/text/finding_text.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/decision.dart';

/// Wie hoch die Liste der Funde höchstens wird, bevor sie scrollt.
///
/// Die drei Knöpfe am Fuß müssen immer sichtbar bleiben: Eine Anfrage mit
/// zwanzig Funden darf die Entscheidung nicht aus dem Pane schieben
/// (`docs/UX.md` 6). Genug für fünf Zeilen in normaler Schriftgröße.
const double findingsPauseListMaxHeight = 160;

/// Die Pause mit offenen Funden.
class FindingsPause extends StatelessWidget {
  /// Baut die Pause über [findings].
  const FindingsPause({
    required this.findings,
    required this.onSendAnyway,
    required this.onBlock,
    required this.onBack,
    this.onPseudonymize,
    this.enabled = true,
    super.key,
  });

  /// Die offenen Funde der Anfrage, soweit der Daemon sie beschrieben hat.
  final FindingSet findings;

  /// Sendet die Anfrage, wie sie ist.
  final VoidCallback onSendAnyway;

  /// Öffnet den Editor mit allen Funden ersetzt, oder null ohne Editor.
  ///
  /// Null lässt den Knopf weg, statt ihn tot hinzustellen: Ein Control, das
  /// nichts tun kann, ist schlechter als keines (`backlog/CONVENTIONS.md`
  /// 4.13).
  final VoidCallback? onPseudonymize;

  /// Blockt die Anfrage.
  final VoidCallback onBlock;

  /// Schließt die Pause, ohne zu entscheiden.
  final VoidCallback onBack;

  /// Falsch, solange nicht entschieden werden kann.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final int undescribed = findings.count - findings.known.length;
    final String title = l10n.interceptFindingsPauseTitle(findings.count);
    return Semantics(
      container: true,
      label: title,
      child: DecoratedBox(
        key: const Key('intercept-findings-pause'),
        decoration: BoxDecoration(
          color: tokens.colors.bg2,
          border: Border.all(color: tokens.state.held),
          borderRadius: BorderRadius.circular(tokens.radii.card),
        ),
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x3),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                title,
                key: const Key('intercept-findings-pause-title'),
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
              ),
              SizedBox(height: tokens.spacing.x2),
              // Die Liste nimmt, was sie braucht, höchstens
              // [findingsPauseListMaxHeight], und scrollt darüber hinaus.
              ConstrainedBox(
                constraints: const BoxConstraints(
                  maxHeight: findingsPauseListMaxHeight,
                ),
                child: SingleChildScrollView(
                  key: const Key('intercept-findings-pause-list'),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      for (int i = 0; i < findings.known.length; i++)
                        _FindingRow(index: i, finding: findings.known[i]),
                    ],
                  ),
                ),
              ),
              // Was der Daemon noch nicht beschrieben hat, zählt trotzdem:
              // Eine Liste, die kürzer ist als die Zahl darüber, sähe sonst
              // vollständig aus (`backlog/CONVENTIONS.md` 4.13).
              if (undescribed > 0)
                Text(
                  l10n.interceptFindingsPauseUndescribed(undescribed),
                  key: const Key('intercept-findings-pause-undescribed'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
              SizedBox(height: tokens.spacing.x3),
              Wrap(
                spacing: tokens.spacing.x3,
                runSpacing: tokens.spacing.x2,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  _PauseButton(
                    key: const Key('intercept-findings-pause-send'),
                    variant: HButtonVariant.secondary,
                    label: l10n.interceptFindingsPauseSendAnyway,
                    shortcut: l10n.interceptFindingsPauseKeySend,
                    onPressed: enabled ? onSendAnyway : null,
                  ),
                  if (onPseudonymize != null)
                    _PauseButton(
                      key: const Key('intercept-findings-pause-pseudonymize'),
                      variant: HButtonVariant.primary,
                      label: l10n.interceptFindingsPausePseudonymize,
                      shortcut: l10n.interceptFindingsPauseKeyPseudonymize,
                      onPressed: enabled ? onPseudonymize : null,
                    ),
                  _PauseButton(
                    key: const Key('intercept-findings-pause-block'),
                    variant: HButtonVariant.danger,
                    label: l10n.interceptFindingsPauseBlock,
                    shortcut: l10n.interceptKeyBlock,
                    onPressed: enabled ? onBlock : null,
                  ),
                  _PauseButton(
                    key: const Key('intercept-findings-pause-back'),
                    variant: HButtonVariant.ghost,
                    label: l10n.interceptFindingsPauseBack,
                    shortcut: l10n.interceptFindingsPauseKeyBack,
                    onPressed: onBack,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Eine Zeile der Liste: Art, gekürzter Wert, Ort.
class _FindingRow extends StatelessWidget {
  const _FindingRow({required this.index, required this.finding});

  final int index;
  final Finding finding;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // `display_prefix` kommt vom Daemon schon in seiner Anzeigeform
    // (`daemon/crates/findings/src/display.rs`): maskiert und, wo gekürzt, mit
    // eigenem `…`, etwa `a***@x.de`, `GB82 …` oder `**** 1111`. IPv4-Adressen
    // und eigene Begriffe stehen dort vollständig. Die Oberfläche zeigt ihn,
    // wie er kommt; ein zweites Auslassungszeichen wäre falsch.
    final String prefix = finding.displayPrefix;
    return Padding(
      key: Key('intercept-findings-pause-row-$index'),
      padding: EdgeInsets.only(bottom: tokens.spacing.x1),
      child: Wrap(
        spacing: tokens.spacing.x2,
        runSpacing: tokens.spacing.x1,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: <Widget>[
          HBadge(text: findingName(finding, l10n), color: tokens.state.held),
          if (prefix.isNotEmpty)
            Text(
              prefix,
              key: Key('intercept-findings-pause-prefix-$index'),
              style: tokens.typography.mono12.tinted(tokens.colors.fg0),
            ),
          Text(
            findingWhere(finding, l10n),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ],
      ),
    );
  }
}

/// Ein Knopf am Fuß der Pause, mit seiner Taste daneben.
class _PauseButton extends StatelessWidget {
  const _PauseButton({
    required this.variant,
    required this.label,
    required this.shortcut,
    required this.onPressed,
    super.key,
  });

  final HButtonVariant variant;
  final String label;
  final String shortcut;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return HButton(
      variant: variant,
      onPressed: onPressed,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(label),
          SizedBox(width: tokens.spacing.x2),
          Text(
            shortcut,
            style: tokens.typography.mono11.tinted(tokens.colors.fg1),
          ),
        ],
      ),
    );
  }
}
