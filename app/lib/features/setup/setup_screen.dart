/// The setup screen: what somebody sees the first time they open this
/// application, and on a machine where something is not yet right (HUM-044).
///
/// Four rows -- background service, model server, project folder, machine --
/// and under them the one button that starts the agent. Every verdict on this
/// screen was made in the daemon; the screen shows them and hands the two
/// gestures that change something back to the shell (ADR-018).
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
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/setup_provider.dart';
import 'setup_text.dart';
import 'widgets/daemon_check.dart';
import 'widgets/llm_check.dart';
import 'widgets/project_check.dart';
import 'widgets/sandbox_check.dart';
import 'widgets/setup_check_row.dart';

/// Width of the centred column.
const double setupColumnWidth = 640;

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
    required this.onForgetProbe,
    required this.onPickWorkDir,
    required this.onWorkMode,
    required this.onStart,
    super.key,
  });

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

  /// Drops the last probe result.
  ///
  /// Called as soon as the text in the field is no longer the address the
  /// result is about: a measurement shown next to another address would claim
  /// to be about that one (CONVENTIONS 4.13).
  final VoidCallback onForgetProbe;

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
  }

  @override
  void dispose() {
    _endpoint.dispose();
    super.dispose();
  }

  /// Drops a result that is no longer about the text in the field.
  void _edited(String text) {
    final String? measured = widget.probe.probe?.endpoint;
    if (measured != null && text.trim() != measured) {
      widget.onForgetProbe();
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SetupState state = widget.state;
    return ColoredBox(
      color: tokens.colors.bg0,
      child: SingleChildScrollView(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Center(
          child: SizedBox(
            width: setupColumnWidth,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  l10n.setupTitle,
                  style: tokens.typography.ui20.semibold.tinted(
                    tokens.colors.fg0,
                  ),
                ),
                SizedBox(height: tokens.spacing.x2),
                Text(
                  l10n.setupIntro,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                ),
                SizedBox(height: tokens.spacing.x4),
                DaemonCheck(
                  check: state[SetupCheckKind.daemon],
                  onRetry: widget.onRetry,
                ),
                const HHairline(),
                LlmCheck(
                  check: state[SetupCheckKind.llm],
                  probe: widget.probe,
                  controller: _endpoint,
                  onProbe: widget.onProbeLlm,
                  onEdited: _edited,
                  enabled:
                      state[SetupCheckKind.daemon].state == SetupCheckState.ok,
                ),
                const HHairline(),
                ProjectCheck(
                  check: state[SetupCheckKind.project],
                  workMode: widget.workMode,
                  locked: widget.sandboxLocked,
                  onPick: widget.onPickWorkDir,
                  onMode: widget.onWorkMode,
                ),
                const HHairline(),
                SandboxCheck(
                  check: state[SetupCheckKind.sandbox],
                  report: widget.report,
                  onRecheck: widget.onRecheck,
                ),
                SizedBox(height: tokens.spacing.x5),
                _StartRow(state: state, onStart: widget.onStart),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The one button of this screen, and the sentence that says what it knows.
class _StartRow extends StatelessWidget {
  const _StartRow({required this.state, required this.onStart});

  final SetupState state;
  final VoidCallback? onStart;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool ready = state.canStart && onStart != null;
    final SetupCallFailure? failure = state.sandboxFailure;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        HButton(
          key: const Key('setup-start'),
          variant: HButtonVariant.primary,
          size: HButtonSize.md,
          onPressed: ready ? onStart : null,
          child: Text(l10n.setupStart),
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
  /// machine row would have gone amber over, so a count above zero and a green
  /// machine row cannot happen together. The number is asked separately one
  /// state further up, where the row is amber and the sentence has to say how
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
class _FailureCard extends StatelessWidget {
  const _FailureCard({required this.failure});

  final Diagnostic failure;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final (String, String) text = setupDiagnosticText(l10n, failure);
    return HDiagnosticCard(
      key: const Key('setup-start-failure'),
      code: failure.code,
      severityLabel: setupSeverityLabel(l10n, failure.severity),
      color: setupSeverityColor(tokens, failure.severity),
      title: text.$1,
      why: text.$2,
      detail: text.$2 == failure.why ? null : failure.why,
      fix: FixControl(
        fix: failure.fix,
        copyKey: const Key('setup-fix-copy-start'),
      ),
      docsUrl: failure.docsUrl,
      width: setupCardWidth,
    );
  }
}
