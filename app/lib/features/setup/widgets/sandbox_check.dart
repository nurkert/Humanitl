/// The fourth row: will a sandbox start on this machine? (HUM-044, HUM-075)
///
/// The row is a summary of the doctor's lines, and the lines themselves fold
/// out under it. The summary is a ranking and never a count: one failing line
/// makes the row red however many green ones stand beside it, and a line that
/// carried no measurement keeps the row out of green even when nothing failed.
/// Every one of the eleven verdicts was made in the daemon
/// (`humanitl_sandbox::doctor`); what happens on this screen is the ranking
/// and the drawing (ADR-018).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/h_collapsible.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';
import 'doctor_list.dart';
import 'setup_check_row.dart';

/// The machine row.
class SandboxCheck extends StatelessWidget {
  /// Creates the row for [check] over [report].
  const SandboxCheck({
    required this.check,
    required this.report,
    required this.onRecheck,
    this.view = SetupStepView.focused,
    super.key,
  });

  /// Open or one line, in front or not (HUM-287).
  final SetupStepView view;

  /// What the row says.
  final SetupCheck check;

  /// The lines the row summarises.
  final DoctorReport report;

  /// Asks the daemon to look again.
  final VoidCallback onRecheck;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Die Zahl im Kopf zaehlt genau die Zeilen, die darunter stehen, und sie
    // nimmt dafuer dieselbe Auswahl wie die Zeile selbst. Der Bericht traegt
    // elf, die Liste zeigt neun: `daemon` und `llm` haben eine eigene Zeile auf
    // diesem Bildschirm, und eine Ueberschrift, die sie mitzaehlt, verspricht
    // zwei Zeilen, die niemand findet.
    final List<DoctorCheck> shown = shownDoctorChecks(machineLines(report));
    return SetupCheckRow(
      check: check,
      title: setupCheckTitle(l10n, SetupCheckKind.sandbox),
      view: view,
      last: true,
      // Grün sagt die eine Zeile, wie viele Zeilen hinter ihr stehen; sonst
      // steht dort die Zeile des Doctors, die den Schritt entschieden hat.
      summary: check.isGreen && shown.isNotEmpty
          ? l10n.setupDoctorLines(shown.length)
          : null,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          if (check.detail.isNotEmpty || shown.isEmpty)
            Text(
              check.detail.isEmpty
                  ? setupNoEvidence(l10n, check.state)
                  : check.detail,
              key: const Key('setup-evidence-sandbox'),
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          if (shown.isNotEmpty)
            HCollapsible(
              title: l10n.setupDoctorLines(shown.length),
              // Zu, solange nichts daran ist: Zeilen über einer Liste von
              // vier Schritten machen die Schritte unlesbar. Ist eine davon
              // rot oder ungemessen, steht sie offen -- das ist dann das eine
              // Wichtige dieses Bildschirms (`docs/UX.md` 3.1).
              initiallyOpen: check.state != SetupCheckState.ok,
              child: DoctorList(checks: shown),
            ),
        ],
      ),
      actions: <Widget>[
        HButton(
          key: const Key('setup-sandbox-recheck'),
          variant: view.current
              ? HButtonVariant.primary
              : HButtonVariant.secondary,
          autofocus: view.current,
          onPressed: check.state == SetupCheckState.checking ? null : onRecheck,
          child: Text(l10n.setupRecheck),
        ),
      ],
    );
  }
}
