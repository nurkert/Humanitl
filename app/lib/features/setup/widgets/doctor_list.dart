/// The eleven lines of `humanitl doctor`, as the setup screen shows them
/// (HUM-075, HUM-044).
///
/// The daemon answers `Doctor()` with one line per precondition, in display
/// order, each with a status, the evidence it measured and -- for every line
/// that is not green -- a finding with cause and proposal. This widget draws
/// that list and nothing else. It sorts nothing, hides nothing and judges
/// nothing: a line this build does not know by name keeps its identifier as
/// its heading and stays in the list, because a daemon that grew a twelfth
/// check must be able to show it (ADR-018).
///
/// The two lines with a row of their own on the screen -- `daemon` and `llm` --
/// do not reach this widget. Which two those are, and therefore how many lines
/// the heading over this list promises, is decided in one place:
/// `machineLines` in `providers/setup_provider.dart`.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';
import 'setup_check_row.dart';

/// The list of doctor lines.
class DoctorList extends StatelessWidget {
  /// Creates the list for [checks].
  const DoctorList({required this.checks, super.key});

  /// The lines, in the order the daemon sent them.
  final List<DoctorCheck> checks;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    if (checks.isEmpty) {
      return Text(
        context.l10n.setupDoctorEmpty,
        key: const Key('setup-doctor-empty'),
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      );
    }
    return Column(
      key: const Key('setup-doctor-list'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        for (final DoctorCheck check in checks) _DoctorRow(check: check),
      ],
    );
  }
}

/// One line: mark, heading, evidence, and the finding when there is one.
class _DoctorRow extends StatelessWidget {
  const _DoctorRow({required this.check});

  final DoctorCheck check;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SetupCheckState state = doctorLineState(check);
    final Diagnostic? diagnostic = check.diagnostic;
    return Padding(
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              SetupMark(state: state),
              SizedBox(width: tokens.spacing.x2),
              SizedBox(
                width: headingWidth,
                child: Text(
                  doctorCheckTitle(l10n, check.id),
                  key: Key('doctor-title-${check.id}'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg0),
                  maxLines: 2,
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              Expanded(
                child: Text(
                  check.evidence.isEmpty
                      ? setupNoEvidence(l10n, state)
                      : check.evidence,
                  key: Key('doctor-evidence-${check.id}'),
                  style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                  maxLines: 3,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
          // Nur ein Vorschlag, keine ganze Karte: Elf Karten untereinander
          // wären elfmal dasselbe Gerüst um je einen Satz. Der Grund steht im
          // Beleg darüber, und der Vorschlag ist das, was jemand anfassen kann
          // (`docs/UX.md` 4.4).
          if (diagnostic?.fix != null)
            Padding(
              padding: EdgeInsets.only(
                left: SetupMark.size + tokens.spacing.x2,
                top: tokens.spacing.x1,
              ),
              child: FixControl(
                fix: diagnostic?.fix,
                copyKey: Key('doctor-fix-${check.id}'),
              ),
            ),
        ],
      ),
    );
  }

  /// Width of the heading column, so eleven headings line up.
  static const double headingWidth = 120;
}
