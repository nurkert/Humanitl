/// The small building blocks the agent cards share (HUM-521): the recessed
/// panel of a question or a run, and a finding of the daemon at the card.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/diagnostic_severity.dart';
import '../../../../core/ui/fix_control.dart';
import '../../../../core/ui/h_diagnostic_card.dart';
import '../../../../core/ui/ui.dart';
import '../../../../core/ui/untranslated_wording.dart';
import '../../../../l10n/l10n.dart';

/// A recessed panel inside a card: the version choice, the question before
/// a removal, a maintenance run, the configuration files.
class AgentInset extends StatelessWidget {
  /// Creates the panel around [child].
  const AgentInset({required this.child, super.key});

  /// The content.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border.all(color: tokens.colors.line),
        borderRadius: BorderRadius.circular(tokens.radii.card),
      ),
      child: Padding(padding: EdgeInsets.all(tokens.spacing.x3), child: child),
    );
  }
}

/// The title line of an [AgentInset].
class AgentInsetTitle extends StatelessWidget {
  /// Creates the line.
  const AgentInsetTitle(this.text, {super.key});

  /// The title.
  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Text(
      text,
      style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
    );
  }
}

/// The buttons at the end of an [AgentInset]: right aligned, the quiet one
/// first, the one that acts last.
class AgentInsetActions extends StatelessWidget {
  /// Creates the row of [children].
  const AgentInsetActions({required this.children, super.key});

  /// The buttons.
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.only(top: tokens.spacing.x3),
      child: Wrap(
        alignment: WrapAlignment.end,
        spacing: tokens.spacing.x2,
        runSpacing: tokens.spacing.x2,
        children: children,
      ),
    );
  }
}

/// One finding of the daemon at a card, with its cause and fix (HUM-521
/// point 6).
class AgentFinding extends StatelessWidget {
  /// Creates the card of [diagnostic].
  const AgentFinding({required this.diagnostic, super.key});

  /// The finding.
  final Diagnostic diagnostic;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DiagnosticText text = DiagnosticL10n.resolve(diagnostic, l10n);
    return HDiagnosticCard(
      code: diagnostic.code,
      severityLabel: severityLabel(l10n, diagnostic.severity),
      color: severityColor(tokens, diagnostic.severity),
      title: text.title,
      why: text.cause,
      original: UntranslatedWording.of(text),
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
      dense: true,
      fix: FixControl(
        fix: diagnostic.fix,
        copyKey: ValueKey<String>('agent-finding-copy-${diagnostic.code}'),
        diagnosticCode: diagnostic.code,
      ),
    );
  }
}
