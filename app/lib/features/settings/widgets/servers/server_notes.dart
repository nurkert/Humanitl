/// The small pieces of text of a server's detail view: a line with a glyph,
/// a finding of "Check", and what a check answered (HUM-405, HUM-414).
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/diagnostic_severity.dart';
import '../../../../core/ui/fix_control.dart';
import '../../../../core/ui/h_diagnostic_card.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../setting_row.dart';

/// One line with a glyph before it; the words carry the meaning, the colour
/// only repeats it.
class ServerNote extends StatelessWidget {
  /// A line of [text] after [glyph].
  const ServerNote({
    required this.glyph,
    required this.color,
    required this.textColor,
    required this.text,
    super.key,
  });

  /// The glyph before the text.
  final HGlyph glyph;

  /// The colour of the glyph.
  final Color color;

  /// The colour of the text.
  final Color textColor;

  /// What the line says.
  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(top: 2),
          child: HGlyphIcon(glyph, size: 12, color: color),
        ),
        SizedBox(width: tokens.spacing.x1),
        Expanded(
          child: Text(text, style: tokens.typography.ui12.tinted(textColor)),
        ),
      ],
    );
  }
}

/// [child] on the left edge of the text of the rows.
class ServerInset extends StatelessWidget {
  /// Insets [child].
  const ServerInset({required this.child, super.key});

  /// What stands on the edge.
  final Widget child;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(horizontal: SettingRowGrid.inset),
    child: child,
  );
}

/// One finding of the check, as a card under the address.
class ProbeFinding extends StatelessWidget {
  /// The card of [diagnostic], keyed [findingKey] for tests.
  const ProbeFinding({
    required this.diagnostic,
    required this.findingKey,
    super.key,
  });

  /// The finding.
  final Diagnostic diagnostic;

  /// The key of the card.
  final String findingKey;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DiagnosticText text = DiagnosticL10n.resolve(diagnostic, l10n);
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      child: HDiagnosticCard(
        key: Key(findingKey),
        code: diagnostic.code,
        severityLabel: severityLabel(l10n, diagnostic.severity),
        color: severityColor(tokens, diagnostic.severity),
        title: text.title,
        why: text.cause,
        fix: FixControl(fix: diagnostic.fix),
        docsUrl: diagnostic.docsUrl,
        width: 480,
      ),
    );
  }
}

/// What a check answered: how many models, whether a key went along, and
/// every finding beside the address, not in a log (HUM-140). The models
/// themselves are offered at the model field.
class ProbeResult extends StatelessWidget {
  /// The answer [probe].
  const ProbeResult({required this.probe, super.key});

  /// What the check answered.
  final LlmProbe probe;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool found = probe.models.isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.only(bottom: tokens.spacing.x2),
          child: Column(
            key: const Key('settings-probe-result'),
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              ServerNote(
                glyph: found ? HGlyph.check : HGlyph.minus,
                color: found ? tokens.stateText.allowed : tokens.colors.fg2,
                textColor: tokens.colors.fg1,
                text: found
                    ? l10n.settingsModelsFound(probe.models.length)
                    // Was zurückkam, wenn keine Modellliste kam (HUM-283).
                    : probe.answer.isNotEmpty
                    ? l10n.settingsModelNoList(probe.answer)
                    : l10n.settingsModelNone,
              ),
              if (probe.keySent)
                Text(
                  l10n.settingsKeySent,
                  key: const Key('settings-probe-key-sent'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
            ],
          ),
        ),
        for (final Diagnostic finding in probe.diagnostics)
          ProbeFinding(
            diagnostic: finding,
            findingKey: 'settings-probe-${finding.code}',
          ),
      ],
    );
  }
}
