/// One step of the setup onboarding: one line while it needs nobody, an open
/// panel while it is the step in front (HUM-044, HUM-287).
///
/// The four steps are the same shape on purpose. The mark is always in the
/// same place, the title always beside it, and what follows the title is
/// always the evidence of that step -- somebody who has read one line has read
/// all four (CONVENTIONS 4.13, Vorhersagbarkeit).
///
/// # One line, or open
///
/// A step that is green, or one that is not the first to need the person,
/// stands as one line: mark, title, state word, evidence, and a quiet
/// "Details" that opens it. The first step that needs the person is open: a
/// headline in plain words, the step's own controls, its finding inside the
/// step and its one action as the primary button, with the step after it
/// named beside that button. Which step that is, the screen decides from the
/// order of the four (`setup_screen.dart`); this widget only draws it.
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
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';

/// How the screen wants one step drawn.
@immutable
class SetupStepView {
  /// Creates the view of one step.
  const SetupStepView({
    this.open = false,
    this.current = false,
    this.onToggle,
    this.next,
  });

  /// An open step that is the one in front, for a screen that shows a single
  /// step on its own.
  static const SetupStepView focused = SetupStepView(open: true, current: true);

  /// True when the step is drawn open.
  final bool open;

  /// True for the first step that needs the person. Its action is the
  /// primary button and takes the focus; at most one step has it.
  final bool current;

  /// Opens or folds the step; null for the step in front, which stays open
  /// while it needs the person.
  final VoidCallback? onToggle;

  /// The sentence beside the primary action that names the step after this
  /// one, or null.
  final String? next;
}

/// One step of the onboarding.
class SetupCheckRow extends StatelessWidget {
  /// Creates the step for [check].
  const SetupCheckRow({
    required this.check,
    required this.title,
    this.view = SetupStepView.focused,
    this.body,
    this.summary,
    this.actions = const <Widget>[],
    this.lineAction,
    this.showDiagnostic = true,
    this.primaryFix = false,
    this.onChooseFolder,
    this.chooseFolderLocked,
    this.notice,
    this.last = false,
    super.key,
  });

  /// What this step says.
  final SetupCheck check;

  /// The heading, already localised.
  final String title;

  /// Open or one line, in front or not.
  final SetupStepView view;

  /// What stands in the open step instead of the evidence line: the step's
  /// own controls, with the evidence among them.
  final Widget? body;

  /// The evidence on the one line, instead of the one this widget builds
  /// from [SetupCheck.detail].
  final String? summary;

  /// The buttons of the open step, under its finding.
  final List<Widget> actions;

  /// A button on the one line, for a step that blocks but is not in front:
  /// its remedy stays one click away without opening it.
  final Widget? lineAction;

  /// False for a step that draws its finding itself.
  final bool showDiagnostic;

  /// True when the fix of the finding is the primary action of this step.
  final bool primaryFix;

  /// Why the folder button of the finding is off ([FixControl.chooseFolderLocked],
  /// HUM-282). Null on every step but a locked project step.
  final String? chooseFolderLocked;

  /// A finding of this screen's own controls, drawn above the step's finding:
  /// the folder dialog that did not open (`UI_003`, HUM-282). It does not
  /// change the state of the step; the step still says what was measured.
  final Diagnostic? notice;

  /// Opens the folder chooser, for a finding whose fix is another folder
  /// ([FixControl.onChooseFolder], HUM-262). Null on every step but the
  /// project step.
  final VoidCallback? onChooseFolder;

  /// True for the last of the four, which draws no line under itself.
  final bool last;

  /// Height of the one line, and of the header line of an open step: the
  /// row of the layout grid (`docs/UX.md` 3.7), as a minimum, so a line grows
  /// with the text instead of clipping it.
  static const double lineHeight = HSize.paneHeader;

  /// Width the title takes at least, so the evidence of the four lines
  /// starts in one column.
  static const double titleWidth = 168;

  /// Width the title and its state word take at most before they wrap.
  static const double titleMaxWidth = 260;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    if (!view.open) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Padding(
            padding: EdgeInsets.symmetric(
              horizontal: tokens.spacing.x3 + HSize.border,
            ),
            child: _line(context, tokens, withEvidence: true),
          ),
          if (!last) const HHairline(),
        ],
      );
    }
    return _panel(context, tokens);
  }

  /// The line of a step: mark, title, state word, and on the one line the
  /// evidence and the way to open it.
  Widget _line(
    BuildContext context,
    HTokens tokens, {
    required bool withEvidence,
  }) {
    final AppLocalizations l10n = context.l10n;
    final VoidCallback? toggle = view.onToggle;
    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: lineHeight),
      child: Row(
        key: Key('setup-step-${check.kind.name}'),
        children: <Widget>[
          SetupMark(state: check.state),
          SizedBox(width: tokens.spacing.x3),
          // Breit genug, dass die Belege der vier Zeilen in einer Spalte
          // beginnen, und begrenzt, damit Titel und Zustandswort bei doppelter
          // Textgröße umbrechen, statt die Zeile über den Rand zu schieben
          // (`docs/UX.md` 6).
          ConstrainedBox(
            constraints: const BoxConstraints(
              minWidth: titleWidth,
              maxWidth: titleMaxWidth,
            ),
            child: Wrap(
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: tokens.spacing.x2,
              children: <Widget>[
                Text(
                  title,
                  style: tokens.typography.ui14.semibold.tinted(
                    check.isGreen && !view.open
                        ? tokens.colors.fg1
                        : tokens.colors.fg0,
                  ),
                ),
                // Das Wort steht immer neben der Marke: eine Farbe allein ist
                // kein Zustand (`docs/UX.md` 3.3).
                Text(
                  setupStateLabel(l10n, check.state),
                  key: Key('setup-state-${check.kind.name}'),
                  style: tokens.typography.ui12.tinted(
                    setupStateTextColor(tokens, check.state),
                  ),
                ),
              ],
            ),
          ),
          SizedBox(width: tokens.spacing.x3),
          Expanded(
            child: withEvidence
                ? _evidence(tokens, l10n, maxLines: 1)
                : const SizedBox.shrink(),
          ),
          if (withEvidence && lineAction != null) ...<Widget>[
            SizedBox(width: tokens.spacing.x2),
            lineAction!,
          ],
          if (toggle != null) ...<Widget>[
            SizedBox(width: tokens.spacing.x2),
            HButton(
              key: Key('setup-step-toggle-${check.kind.name}'),
              variant: HButtonVariant.ghost,
              size: HButtonSize.sm,
              onPressed: toggle,
              child: Text(
                view.open ? l10n.setupStepHide : l10n.setupStepDetails,
              ),
            ),
          ],
        ],
      ),
    );
  }

  /// The open step: a raised panel with headline, controls, finding and
  /// actions, indented under the title like the evidence of a line.
  Widget _panel(BuildContext context, HTokens tokens) {
    final AppLocalizations l10n = context.l10n;
    final Diagnostic? diagnostic = check.diagnostic;
    final String? headline = setupStepHeadline(l10n, check);
    final String? next = view.next;
    return Padding(
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x2),
      child: DecoratedBox(
        key: Key('setup-open-${check.kind.name}'),
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          borderRadius: BorderRadius.circular(tokens.radii.card),
          border: Border.all(
            color: view.current ? tokens.colors.lineStrong : tokens.colors.line,
          ),
        ),
        child: Padding(
          padding: EdgeInsets.fromLTRB(
            tokens.spacing.x3,
            0,
            tokens.spacing.x4,
            tokens.spacing.x4,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              _line(context, tokens, withEvidence: false),
              Padding(
                padding: EdgeInsets.only(
                  left: SetupMark.size + tokens.spacing.x3,
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    if (headline != null) ...<Widget>[
                      Text(
                        headline,
                        key: Key('setup-headline-${check.kind.name}'),
                        style: tokens.typography.ui16.semibold.tinted(
                          tokens.colors.fg0,
                        ),
                      ),
                      SizedBox(height: tokens.spacing.x2),
                    ],
                    body ?? _evidence(tokens, l10n, maxLines: 2),
                    if (notice case final Diagnostic notice) ...<Widget>[
                      SizedBox(height: tokens.spacing.x3),
                      _finding(
                        notice,
                        copyKey: 'setup-fix-copy-notice',
                        primary: false,
                      ),
                    ],
                    if (showDiagnostic && diagnostic != null) ...<Widget>[
                      SizedBox(height: tokens.spacing.x3),
                      _finding(
                        diagnostic,
                        copyKey: 'setup-fix-copy-${check.kind.name}',
                        primary: view.current && primaryFix,
                      ),
                    ],
                    if (actions.isNotEmpty ||
                        (view.current && next != null)) ...<Widget>[
                      SizedBox(height: tokens.spacing.x3),
                      Wrap(
                        spacing: tokens.spacing.x2,
                        runSpacing: tokens.spacing.x2,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: <Widget>[
                          ...actions,
                          if (view.current && next != null)
                            Padding(
                              padding: EdgeInsets.only(
                                left: actions.isEmpty ? 0 : tokens.spacing.x1,
                              ),
                              child: Text(
                                next,
                                key: const Key('setup-step-next'),
                                style: tokens.typography.ui12.tinted(
                                  tokens.colors.fg1,
                                ),
                              ),
                            ),
                        ],
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// The finding of this step, inside it.
  Widget _finding(
    Diagnostic diagnostic, {
    required String copyKey,
    required bool primary,
  }) => SetupFinding(
    diagnostic: diagnostic,
    fix: FixControl(
      fix: diagnostic.fix,
      copyKey: Key(copyKey),
      onChooseFolder: onChooseFolder,
      chooseFolderLocked: chooseFolderLocked,
      diagnosticCode: diagnostic.code,
      primary: primary,
    ),
  );

  /// The evidence line, or the sentence that says nothing was measured.
  ///
  /// On the one line of a step with a finding and no evidence it is the
  /// finding's title: the finding itself only stands in the open step, and
  /// the line must still say what is wrong. In the open step it is nothing
  /// then, because the finding right under it carries code, cause and fix,
  /// and a line above it claiming "no detail" would be plainly false.
  Widget _evidence(
    HTokens tokens,
    AppLocalizations l10n, {
    required int maxLines,
  }) {
    final DaemonInfo? daemon = check.daemon;
    final Diagnostic? diagnostic = check.diagnostic;
    final String text;
    if (summary case final String summary) {
      text = summary;
    } else if (daemon != null) {
      text = l10n.setupEvidenceDaemon(
        daemon.daemonVersion,
        daemon.protoVersion,
      );
    } else if (check.detail.isNotEmpty) {
      text = check.detail;
    } else if (diagnostic != null && showDiagnostic) {
      if (view.open) {
        return const SizedBox.shrink();
      }
      text = setupDiagnosticText(l10n, diagnostic).$1;
    } else {
      text = setupNoEvidence(l10n, check.state);
    }
    return Text(
      text,
      key: Key('setup-evidence-${check.kind.name}'),
      style: tokens.typography.mono12.tinted(
        check.isGreen || view.open
            ? tokens.colors.fg1
            : setupStateTextColor(tokens, check.state),
      ),
      maxLines: maxLines,
      overflow: TextOverflow.ellipsis,
    );
  }
}

/// One finding inside its step: code, severity, title, cause, detail, fix
/// and documentation (`docs/UX.md` 4.4, HUM-287).
///
/// The content is the one `HDiagnosticCard` carries, without the card: a
/// state rail in the colour of the severity and the text beside it. A finding
/// is part of the step it explains, not a second box under a list.
class SetupFinding extends StatelessWidget {
  /// Creates the finding for [diagnostic] with its [fix] control.
  const SetupFinding({required this.diagnostic, this.fix, super.key});

  /// What the daemon, or this client, found.
  final Diagnostic diagnostic;

  /// The control that carries the fix out, when there is one.
  final Widget? fix;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Color color = setupSeverityColor(tokens, diagnostic.severity);
    // Der allgemeine Satz ist der Titel, der gemessene der Grund, und die
    // technische Zeile steht als Detail darunter (`docs/UX.md` 4.4). Gibt es
    // zu diesem Code keinen übersetzten Satz, ist der übersetzte der rohe,
    // und eine zweite Kopie darunter entfällt.
    final (String title, String why) = setupDiagnosticText(l10n, diagnostic);
    final String? detail = why == diagnostic.why ? null : diagnostic.why;
    final String? docsUrl = diagnostic.docsUrl;
    final Widget? fix = this.fix;
    return Semantics(
      container: true,
      label: '${diagnostic.code} $title',
      child: Container(
        key: ValueKey<String>('setup-finding-${diagnostic.code}'),
        width: double.infinity,
        decoration: BoxDecoration(
          border: Border(
            left: BorderSide(color: color, width: HSize.stateRail),
          ),
        ),
        padding: EdgeInsets.only(left: tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Wrap(
              spacing: tokens.spacing.x2,
              runSpacing: tokens.spacing.x1,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                HBadge(text: diagnostic.code, color: color, mono: true),
                HBadge(
                  text: setupSeverityLabel(l10n, diagnostic.severity),
                  color: color,
                ),
                Text(
                  title,
                  style: tokens.typography.ui14.semibold.tinted(
                    tokens.colors.fg0,
                  ),
                ),
              ],
            ),
            SizedBox(height: tokens.spacing.x1),
            Text(why, style: tokens.typography.ui13.tinted(tokens.colors.fg1)),
            if (detail != null && detail.isNotEmpty) ...<Widget>[
              SizedBox(height: tokens.spacing.x1),
              Text(
                detail,
                style: tokens.typography.mono12.tinted(tokens.colors.fg1),
              ),
            ],
            if (fix != null) ...<Widget>[
              SizedBox(height: tokens.spacing.x2),
              fix,
            ],
            if (docsUrl != null && docsUrl.isNotEmpty) ...<Widget>[
              SizedBox(height: tokens.spacing.x2),
              Text(
                docsUrl,
                // Der Akzent ist eine Fläche; ein Wort darauf nimmt seine
                // Textvariante (`docs/UX.md` 6).
                style: tokens.typography.mono12.tinted(
                  tokens.colors.accentText,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// The mark of one step: a disc for a measurement, a ring for none.
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
