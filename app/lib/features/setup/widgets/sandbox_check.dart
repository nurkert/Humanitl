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
    super.key,
  });

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
    final List<DoctorCheck> shown = machineLines(report);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SetupCheckRow(
          check: check,
          title: setupCheckTitle(l10n, SetupCheckKind.sandbox),
          control: HButton(
            key: const Key('setup-sandbox-recheck'),
            size: HButtonSize.sm,
            onPressed: check.state == SetupCheckState.checking
                ? null
                : onRecheck,
            child: Text(l10n.setupRecheck),
          ),
        ),
        if (shown.isNotEmpty)
          Padding(
            padding: EdgeInsets.only(left: SetupMark.size + tokens.spacing.x3),
            child: HCollapsible(
              title: l10n.setupDoctorLines(shown.length),
              // Zu, solange nichts daran ist: Elf Zeilen über einer Liste von
              // vier machen die vier unlesbar. Ist eine davon rot oder
              // ungemessen, steht sie offen -- das ist dann das eine Wichtige
              // dieses Bildschirms (`docs/UX.md` 3.1).
              initiallyOpen: check.state != SetupCheckState.ok,
              child: DoctorList(checks: shown),
            ),
          ),
      ],
    );
  }
}
