/// One row of the setup checklist: mark, title, evidence, action (HUM-044).
///
/// The four rows are the same shape on purpose. The mark is always in the same
/// place, the action is always on the right, and the sentence under the title
/// is always the evidence of the row above it -- somebody who has read one row
/// has read all four (CONVENTIONS 4.13, Vorhersagbarkeit).
///
/// # The mark says three things, and colour is only one of them
///
/// Shape, colour and word all carry the state (`docs/UX.md` 3.3). The shape is
/// the part that matters most here: **a state that was measured is a disc, a
/// state that was not is a ring.** That is the same idiom the isolation panel
/// uses for a guarantee nobody measured (`sandbox_text.dart`,
/// `isolationSegmentFilled`), and it exists because a check that could not run
/// must never be readable as a paler version of one that passed.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';

/// One row of the checklist.
class SetupCheckRow extends StatelessWidget {
  /// Creates the row for [check].
  const SetupCheckRow({
    required this.check,
    required this.title,
    this.control,
    this.detail,
    this.showDiagnostic = true,
    super.key,
  });

  /// What this row says.
  final SetupCheck check;

  /// The heading, already localised.
  final String title;

  /// The control on the right, when the row has one.
  final Widget? control;

  /// What stands under the title instead of [SetupCheck.detail].
  final Widget? detail;

  /// False for a row that draws its finding itself, further down.
  final bool showDiagnostic;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Diagnostic? diagnostic = check.diagnostic;
    final (String, String) text = diagnostic == null
        ? ('', '')
        : setupDiagnosticText(l10n, diagnostic);
    return Padding(
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Padding(
                padding: EdgeInsets.only(top: tokens.spacing.x1),
                child: SetupMark(state: check.state),
              ),
              SizedBox(width: tokens.spacing.x3),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    // Umbruch statt fester Zeile: Bei doppelter Textgröße
                    // passen Überschrift und Zustandswort nicht mehr
                    // nebeneinander, und eine `Row` verlöre den Rest still
                    // hinter dem Rand (`docs/UX.md` 6, Textskalierung).
                    Wrap(
                      crossAxisAlignment: WrapCrossAlignment.center,
                      spacing: tokens.spacing.x2,
                      runSpacing: tokens.spacing.x1,
                      children: <Widget>[
                        Text(
                          title,
                          style: tokens.typography.ui14.semibold.tinted(
                            tokens.colors.fg0,
                          ),
                        ),
                        // Das Wort steht immer neben der Marke: eine Farbe
                        // allein ist kein Zustand (`docs/UX.md` 3.3).
                        Text(
                          setupStateLabel(l10n, check.state),
                          key: Key('setup-state-${check.kind.name}'),
                          style: tokens.typography.ui12.tinted(
                            setupStateTextColor(tokens, check.state),
                          ),
                        ),
                      ],
                    ),
                    SizedBox(height: tokens.spacing.x1),
                    detail ?? _evidence(tokens, l10n),
                  ],
                ),
              ),
              if (control case final Widget control) ...<Widget>[
                SizedBox(width: tokens.spacing.x3),
                control,
              ],
            ],
          ),
          if (showDiagnostic && diagnostic != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x2),
            Padding(
              padding: EdgeInsets.only(
                left: SetupMark.size + tokens.spacing.x3,
              ),
              child: HDiagnosticCard(
                code: diagnostic.code,
                severityLabel: setupSeverityLabel(l10n, diagnostic.severity),
                color: setupSeverityColor(tokens, diagnostic.severity),
                // Der allgemeine Satz ist der Titel, der gemessene der Grund,
                // und die technische Zeile steht als Detail darunter
                // (`docs/UX.md` 4.4).
                //
                // Die Detailzeile trägt den rohen Satz des Daemons **unter**
                // einem übersetzten. Gibt es zu diesem Code keinen übersetzten
                // -- und den gibt es nur für die vier, die diese Anwendung
                // selbst baut --, dann ist der übersetzte genau der rohe, und
                // eine zweite Kopie darunter sagt dieselbe Sache zweimal.
                // Dann entfällt sie.
                title: text.$1,
                why: text.$2,
                detail: text.$2 == diagnostic.why ? null : diagnostic.why,
                fix: FixControl(
                  fix: diagnostic.fix,
                  copyKey: Key('setup-fix-copy-${check.kind.name}'),
                ),
                docsUrl: diagnostic.docsUrl,
                width: setupCardWidth,
              ),
            ),
          ],
        ],
      ),
    );
  }

  /// The evidence line, or the sentence that says nothing was measured.
  ///
  /// Nothing at all when there is no evidence and a card follows: the card
  /// carries code, cause and proposal, and a line above it claiming "no
  /// detail" would be plainly false.
  Widget _evidence(HTokens tokens, AppLocalizations l10n) {
    if (check.detail.isEmpty && showDiagnostic && check.diagnostic != null) {
      return const SizedBox.shrink();
    }
    final String text = check.detail.isEmpty
        ? setupNoEvidence(l10n, check.state)
        : check.detail;
    return Text(
      text,
      key: Key('setup-evidence-${check.kind.name}'),
      style: tokens.typography.mono12.tinted(tokens.colors.fg1),
      maxLines: 2,
      overflow: TextOverflow.ellipsis,
    );
  }
}

/// The mark of one row: a disc for a measurement, a ring for none.
class SetupMark extends StatelessWidget {
  /// Creates the mark for [state].
  const SetupMark({required this.state, super.key});

  /// Which state to draw.
  final SetupCheckState state;

  /// Edge length of the square the mark occupies.
  static const double size = HSize.glyph;

  /// Diameter of the mark inside that square.
  static const double dotSize = HSize.glyph / 2;

  /// Thickness of the ring drawn for a state nobody measured.
  static const double ringWidth = 2;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color color = setupStateColor(tokens, state);
    final bool filled = setupStateFilled(state);
    // Die Farbe ist das Einzige, was sich hier bewegt, und sie bewegt sich in
    // Ort: eine Zeile, die von „wird geprüft" auf „grün" springt, erklärt
    // nichts (`docs/UX.md` 2.1).
    return HAnimatedFill(
      color: color,
      builder: (BuildContext context, Color color) => SizedBox(
        width: size,
        height: size,
        child: Center(
          child: Container(
            width: dotSize,
            height: dotSize,
            decoration: BoxDecoration(
              color: filled ? color : null,
              border: filled
                  ? null
                  : Border.all(color: color, width: ringWidth),
              shape: BoxShape.circle,
            ),
          ),
        ),
      ),
    );
  }
}

/// Width of the diagnostic card under a row.
///
/// Narrower than the default so that the card stays inside the centred column
/// of the screen even with the mark's indent in front of it.
const double setupCardWidth = 520;
