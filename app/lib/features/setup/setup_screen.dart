/// The setup screen: what somebody sees the first time they open this
/// application, and on a machine where something is not yet right (HUM-044).
///
/// Four steps in the order of the work -- background service, model server
/// with its agent, project folder, machine -- and under them the one button
/// that starts a session. Every verdict on this screen was made in the daemon;
/// the screen shows them and hands the gestures that change something back to
/// the shell (ADR-018).
///
/// # A short onboarding, not a list of cards (HUM-287)
///
/// A step that is green is one line with its evidence. The first step that is
/// not green is open: a headline in plain words, the step's controls, its
/// finding inside the step and its action as the primary button, with the
/// step after it named beside that button. Every other step that is not
/// green stays one line too and keeps its remedy on that line where it has a
/// button of its own (the folder chooser). With all four green the heading
/// says "Humanitl is ready" and the one primary action is the start, or the
/// way to the queue while a session already runs. The design was picked by
/// the owner from rendered variants on 2026-10-24 (`docs/UX.md` 4.10).
///
/// Which step is in front is the order of the four and nothing else: the
/// screen judges no step, it only finds the first one that is not green.
///
/// # What the button hangs on, and what the line under it says
///
/// The button is on when all four rows are green, and off otherwise -- failed,
/// still being asked, measured and off, or never measured at all. The line
/// under it names the row in the way of the start and what would clear it, so
/// that a grey button is never the whole answer (`docs/UX.md` 5.3). The
/// reasoning stands in `features/setup/providers/setup_provider.dart`.
///
/// # Why this screen shows up full-screen sometimes
///
/// With no daemon there is no shell to put it in: every section would be an
/// error card. `ConnectionGate` therefore renders this screen alone, and the
/// first row carries the finding with its fix -- the one important thing of
/// that screen (`docs/UX.md` 3.1, 4.2). With a daemon it is the sixth section
/// of the shell, so the header keeps showing held requests and `Ctrl+1` still
/// switches to them.
library;

import 'package:flutter/widgets.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/fix_control.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/setup_provider.dart';
import 'setup_text.dart';
import 'widgets/daemon_check.dart';
import 'widgets/llm_check.dart';
import 'widgets/llm_discover_sheet.dart';
import 'widgets/project_check.dart';
import 'widgets/sandbox_check.dart';
import 'widgets/setup_check_row.dart';

/// Width of the centred column.
const double setupColumnWidth = 720;

/// The setup screen.
class SetupScreen extends StatefulWidget {
  /// Creates the screen over what the shell knows.
  const SetupScreen({
    required this.state,
    required this.report,
    required this.probe,
    required this.llmEndpoint,
    required this.workMode,
    required this.sandboxLocked,
    required this.onRetry,
    required this.onRecheck,
    required this.onProbeLlm,
    required this.onAddress,
    required this.onPickWorkDir,
    required this.onWorkMode,
    required this.onStart,
    this.daemonAttempts = 0,
    this.activeServer = '',
    this.onOpenIntercept,
    super.key,
  });

  /// Shows the queue. The ready screen offers it as its one action while a
  /// session already runs and nothing may start a second one (HUM-287).
  final VoidCallback? onOpenIntercept;

  /// The name of the active saved model server, or empty when none is
  /// saved; the model row names it above the field (HUM-289).
  final String activeServer;

  /// How many attempts to reach the daemon failed so far, or 0 while it
  /// answers. The first row shows the count, so a screen that keeps trying
  /// says so (HUM-058, row 1).
  final int daemonAttempts;

  /// The four rows.
  final SetupState state;

  /// The doctor's lines, for the machine section.
  final DoctorReport report;

  /// What the endpoint probe last answered.
  final LlmProbeState probe;

  /// The endpoint the daemon has in its configuration; seeds the field.
  final String llmEndpoint;

  /// How the project folder would be mounted.
  final WorkMode workMode;

  /// True while the mount cannot change because a sandbox is up.
  final bool sandboxLocked;

  /// Tries the connection again.
  final VoidCallback onRetry;

  /// Asks the daemon to look at the machine again.
  final VoidCallback onRecheck;

  /// Asks the daemon to contact an endpoint.
  final void Function(String endpoint) onProbeLlm;

  /// Called with the text of the endpoint field whenever somebody edits it.
  ///
  /// The step shows a measurement only for the address in the field, so the
  /// shell has to know which one that is (HUM-287, CONVENTIONS 4.13).
  final ValueChanged<String> onAddress;

  /// Called with the folder somebody chose.
  final void Function(String workDir) onPickWorkDir;

  /// Called with the mount mode somebody chose.
  final void Function(WorkMode mode) onWorkMode;

  /// Starts the agent. Null while nothing may start.
  final VoidCallback? onStart;

  @override
  State<SetupScreen> createState() => _SetupScreenState();
}

class _SetupScreenState extends State<SetupScreen> {
  late final TextEditingController _endpoint = TextEditingController(
    text: widget.llmEndpoint,
  );

  /// True while the search sheet is open. Opening it contacts nothing.
  bool _discovering = false;

  /// The steps somebody opened by hand. The step in front is open without
  /// being in here, and a step that turns green stays as it was asked for.
  final Set<SetupCheckKind> _opened = <SetupCheckKind>{};

  void _toggle(SetupCheckKind kind) => setState(() {
    if (!_opened.remove(kind)) {
      _opened.add(kind);
    }
  });

  /// How the screen wants [kind] drawn, given the step in front.
  SetupStepView _view(
    AppLocalizations l10n,
    SetupState state,
    SetupCheckKind kind,
  ) {
    final SetupCheckKind? current = setupCurrentStep(state);
    if (kind == current) {
      final SetupCheckKind? after = setupStepAfter(state, kind);
      return SetupStepView(
        open: true,
        current: true,
        next: after == null
            ? l10n.setupStepNextStart
            : l10n.setupStepNext(setupCheckTitle(l10n, after)),
      );
    }
    return SetupStepView(
      open: _opened.contains(kind),
      onToggle: () => _toggle(kind),
    );
  }

  @override
  void didUpdateWidget(SetupScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Der Daemon nennt den Endpunkt seiner Konfiguration. Er wird nur
    // uebernommen, solange niemand hier etwas anderes getippt hat: einem
    // Menschen das Feld unter den Fingern zu ueberschreiben, waere schlimmer
    // als ein Feld, das kurz leer bleibt.
    if (widget.llmEndpoint != oldWidget.llmEndpoint &&
        _endpoint.text == oldWidget.llmEndpoint) {
      _endpoint.text = widget.llmEndpoint;
    }
    // Ein Schritt, den jemand über „Details" geöffnet hat, faltet sich, sobald
    // er grün wird: Fertig heißt vier Zeilen, nicht eine offene Fläche
    // zwischen drei Zeilen (HUM-287, Akzeptanzkriterium 1). Öffnen lässt er
    // sich danach wieder.
    for (final SetupCheck check in widget.state.checks) {
      if (check.isGreen && !oldWidget.state[check.kind].isGreen) {
        _opened.remove(check.kind);
      }
    }
  }

  @override
  void dispose() {
    _endpoint.dispose();
    super.dispose();
  }

  /// Tells the shell which address the field shows now.
  void _edited(String text) => widget.onAddress(text);

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SetupState state = widget.state;
    return Stack(
      children: <Widget>[
        _column(context, tokens, l10n, state),
        if (_discovering)
          LlmDiscoverSheet(
            onClose: () => setState(() => _discovering = false),
            onPick: _took,
          ),
      ],
    );
  }

  /// Takes the address of a found server over into the field and contacts it.
  ///
  /// Contacting it is not an extra: the search already knows this server
  /// answers, and the row above the button must show a measurement of the
  /// address that now stands in the field, not of the one before it
  /// (CONVENTIONS 4.13).
  void _took(String endpoint) {
    _endpoint.text = endpoint;
    setState(() => _discovering = false);
    widget.onProbeLlm(endpoint);
  }

  Widget _column(
    BuildContext context,
    HTokens tokens,
    AppLocalizations l10n,
    SetupState state,
  ) {
    final int ready = state.checks.where((SetupCheck c) => c.isGreen).length;
    return ColoredBox(
      color: tokens.colors.bg0,
      child: SingleChildScrollView(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Center(
          child: SizedBox(
            width: setupColumnWidth,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(
                  state.canStart ? l10n.setupTitleReady : l10n.setupTitle,
                  key: const Key('setup-title'),
                  style: tokens.typography.ui20.semibold.tinted(
                    tokens.colors.fg0,
                  ),
                ),
                SizedBox(height: tokens.spacing.x1),
                Text(
                  '${l10n.setupProgress(ready, state.checks.length)} '
                  '${l10n.setupIntro}',
                  key: const Key('setup-progress'),
                  style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                ),
                SizedBox(height: tokens.spacing.x4),
                DaemonCheck(
                  check: state[SetupCheckKind.daemon],
                  onRetry: widget.onRetry,
                  attempts: widget.daemonAttempts,
                  view: _view(l10n, state, SetupCheckKind.daemon),
                ),
                LlmCheck(
                  check: state[SetupCheckKind.llm],
                  probe: widget.probe,
                  controller: _endpoint,
                  activeServer: widget.activeServer,
                  onProbe: widget.onProbeLlm,
                  onDiscover: () => setState(() => _discovering = true),
                  onEdited: _edited,
                  enabled:
                      state[SetupCheckKind.daemon].state == SetupCheckState.ok,
                  view: _view(l10n, state, SetupCheckKind.llm),
                  agent: widget.report[DoctorCheckId.agent],
                ),
                ProjectCheck(
                  check: state[SetupCheckKind.project],
                  workMode: widget.workMode,
                  locked: widget.sandboxLocked,
                  onPick: widget.onPickWorkDir,
                  onMode: widget.onWorkMode,
                  view: _view(l10n, state, SetupCheckKind.project),
                ),
                SandboxCheck(
                  check: state[SetupCheckKind.sandbox],
                  report: widget.report,
                  onRecheck: widget.onRecheck,
                  view: _view(l10n, state, SetupCheckKind.sandbox),
                ),
                SizedBox(height: tokens.spacing.x5),
                _StartRow(
                  state: state,
                  onStart: widget.onStart,
                  onOpenIntercept: widget.sandboxLocked
                      ? widget.onOpenIntercept
                      : null,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The first step that is not green, in the order of the four, or null when
/// all four are (HUM-287).
///
/// The step in front is decided by the order alone. The screen judges no
/// step: green is the daemon's verdict, and the order is the order of the
/// work -- without a background service nothing else can be asked, and
/// without a model and a folder a sound machine starts nothing.
SetupCheckKind? setupCurrentStep(SetupState state) {
  for (final SetupCheck check in state.checks) {
    if (!check.isGreen) {
      return check.kind;
    }
  }
  return null;
}

/// The first step after [kind] that is not green, or null.
SetupCheckKind? setupStepAfter(SetupState state, SetupCheckKind kind) {
  bool past = false;
  for (final SetupCheck check in state.checks) {
    if (past && !check.isGreen) {
      return check.kind;
    }
    if (check.kind == kind) {
      past = true;
    }
  }
  return null;
}

/// The one button of this screen, and the sentence that says what it knows.
class _StartRow extends StatelessWidget {
  const _StartRow({
    required this.state,
    required this.onStart,
    this.onOpenIntercept,
  });

  final SetupState state;
  final VoidCallback? onStart;

  /// Shows the queue; set while a session runs and the start is shut.
  final VoidCallback? onOpenIntercept;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool ready = state.canStart && onStart != null;
    final SetupCallFailure? failure = state.sandboxFailure;
    // Eine Primäraktion je Bildschirm: Solange ein Schritt vorn steht, ist es
    // seine, und der Start tritt zurück (HUM-287). Läuft schon eine Session,
    // ist der Weg zur Warteschlange die eine Aktion des fertigen Setups.
    final VoidCallback? toQueue = state.canStart ? onOpenIntercept : null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Wrap(
          spacing: tokens.spacing.x2,
          runSpacing: tokens.spacing.x2,
          children: <Widget>[
            HButton(
              key: const Key('setup-start'),
              variant: ready
                  ? HButtonVariant.primary
                  : HButtonVariant.secondary,
              size: HButtonSize.md,
              autofocus: ready,
              onPressed: ready ? onStart : null,
              child: Text(l10n.setupStart),
            ),
            if (toQueue != null)
              HButton(
                key: const Key('setup-open-intercept'),
                variant: HButtonVariant.primary,
                size: HButtonSize.md,
                autofocus: true,
                onPressed: toQueue,
                child: Text(l10n.setupGoToIntercept),
              ),
          ],
        ),
        SizedBox(height: tokens.spacing.x2),
        // Ein Control, das aus ist, sagt auf sich selbst warum
        // (`docs/UX.md` 5.3): welche der vier Zeilen im Weg steht und was sie
        // wieder grün macht. Steht keine im Weg, sagt der Satz, worauf sich
        // jemand dann verlassen darf.
        Text(
          _sentence(l10n),
          key: const Key('setup-start-reason'),
          style: tokens.typography.ui12.tinted(
            ready && failure == null
                ? tokens.colors.fg1
                : tokens.stateText.error,
          ),
        ),
        // Ein Aufruf, der nicht angekommen ist, wird dort berichtet, wo er
        // gemacht wurde: unter dem Knopf, auf den jemand gedrückt hat, und
        // nicht auf einer der vier Zeilen, über die er nichts sagt
        // (`docs/UX.md` 4.4).
        if (failure != null) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          _FailureCard(failure: failure.diagnostic),
        ],
      ],
    );
  }

  /// The sentence under the button: which row is in the way, and what clears
  /// it.
  ///
  /// The row it names is the gravest one, by the ranking of
  /// [SetupCheckState]; with four green rows there is none, and the sentence
  /// says what the person may then rely on.
  ///
  /// **The green sentence makes two claims** -- everything was measured, and
  /// everything holds. The rows carry the second one; the machine row is the
  /// one that carries the first, and it is never green while one of its
  /// folded lines was not measured. That is why four green rows are enough for
  /// this sentence: [SetupState.unmeasuredLines] counts exactly the lines the
  /// machine row would have gone yellow (warning) over, so a count above zero and a green
  /// machine row cannot happen together. The number is asked separately one
  /// state further up, where the row wears the warning and the sentence has to say how
  /// many lines that was (HUM-044).
  ///
  /// The last call is the one thing the four rows cannot say. A `Sandbox` call
  /// that did not arrive leaves every verdict as old and as true as it was,
  /// so all four may well be green while nothing started; the sentence says
  /// so, and the card under it carries the finding.
  ///
  /// A call that arrived and was refused gets the other sentence. Saying "did
  /// not arrive" over a `SANDBOX_013` would claim a lost request while the
  /// daemon was in fact answering, and would send somebody looking at the
  /// socket instead of at the guarantee that did not hold (CONVENTIONS 4.13).
  String _sentence(AppLocalizations l10n) {
    final SetupCheckState worst = state.worst;
    final SetupCheck row = state.checks.firstWhere(
      (SetupCheck check) => check.state == worst,
    );
    final String title = setupCheckTitle(l10n, row.kind);
    return switch (worst) {
      SetupCheckState.failed => l10n.setupStartBlocked(title),
      SetupCheckState.checking => l10n.setupStartWaiting(title),
      SetupCheckState.unmeasured =>
        row.kind == SetupCheckKind.sandbox && state.unmeasuredLines > 0
            ? l10n.setupStartUnmeasured(state.unmeasuredLines)
            : l10n.setupStartNotMeasured(title),
      SetupCheckState.warn => l10n.setupStartWarned(title),
      SetupCheckState.ok => switch (state.sandboxFailure) {
        null => l10n.setupStartReady,
        SetupCallFailure(delivered: false) => l10n.setupStartCallFailed,
        SetupCallFailure() => l10n.setupStartRefused,
      },
    };
  }
}

/// The finding of a `Sandbox` call that did not arrive, under the button the
/// gesture was made on.
///
/// The same inline finding the steps carry (HUM-287): code, cause and fix,
/// without a card around them.
class _FailureCard extends StatelessWidget {
  const _FailureCard({required this.failure});

  final Diagnostic failure;

  @override
  Widget build(BuildContext context) {
    return SetupFinding(
      key: const Key('setup-start-failure'),
      diagnostic: failure,
      fix: FixControl(
        fix: failure.fix,
        copyKey: const Key('setup-fix-copy-start'),
      ),
    );
  }
}
