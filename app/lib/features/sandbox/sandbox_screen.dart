/// The Sandbox section: what the agent gets, and start and stop (HUM-040).
///
/// The person who opens this screen is asking one question -- does the agent
/// get my whole disk? -- and everything on it serves the answer: the sentence
/// says it, the mounts table proves it, the environment says what the agent
/// was told, and the command line at the bottom is the source all three were
/// read from. Nothing here is derived in the application; the daemon answers
/// and the screen shows the answer (ADR-018).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/ui/fix_control.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/sandbox_status_provider.dart';
import 'sandbox_text.dart';
import 'widgets/argv_sheet.dart';
import 'widgets/arrive.dart';
import 'widgets/coming_pane.dart';
import 'widgets/env_tab.dart';
import 'widgets/log_tab.dart';
import 'widgets/mounts_tab.dart';
import 'widgets/sandbox_header.dart';
import 'widgets/sandbox_tabs.dart';
import 'widgets/stop_dialog.dart';

/// How much of the height the terminal keeps.
///
/// The terminal is where the work happens, so it is the larger half even
/// while it is a placeholder: the panes must not move under the person once
/// HUM-042 fills it (CONVENTIONS 4.13, "Vorhersagbarkeit").
const double sandboxTerminalFraction = 0.6;

/// How many skeleton rows the first load draws.
const int sandboxSkeletonRows = 6;

/// The Sandbox section.
class SandboxScreen extends ConsumerStatefulWidget {
  /// Creates the section.
  const SandboxScreen({super.key});

  @override
  ConsumerState<SandboxScreen> createState() => _SandboxScreenState();
}

class _SandboxScreenState extends ConsumerState<SandboxScreen> {
  bool _visible = false;
  bool _asking = false;
  bool _argvOpen = false;

  /// True while the shell actually paints this section. The shell keeps every
  /// section built inside an `IndexedStack`, which wraps each child in a
  /// `Visibility`; reading it here keeps the check inside this feature
  /// (ARCHITECTURE 5).
  bool _isVisible(BuildContext context) => Visibility.of(context);

  void _askStop() => setState(() => _asking = true);

  void _cancelStop() => setState(() => _asking = false);

  void _confirmStop() {
    setState(() => _asking = false);
    unawaited(ref.read(sandboxStatusProvider.notifier).stop());
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool visible = _isVisible(context);
    if (visible && !_visible) {
      // A session may have started next door -- from the command line, or in
      // a second window. One call catches up with all of it.
      WidgetsBinding.instance.addPostFrameCallback(
        (Duration _) => ref.read(sandboxStatusProvider.notifier).refresh(),
      );
    }
    _visible = visible;

    final AsyncValue<SandboxStatus> snapshot = ref.watch(sandboxStatusProvider);
    final SandboxStatus status = snapshot.value ?? const SandboxStatus();
    return ColoredBox(
      color: tokens.colors.bg0,
      child: Stack(
        fit: StackFit.expand,
        children: <Widget>[
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              SandboxHeader(status: status, onAskStop: _askStop),
              const HHairline(),
              _Diagnostics(status: status),
              Expanded(
                child: switch (snapshot) {
                  AsyncError(:final Object error) => _CannotRead(error: error),
                  _ => HWait(
                    loading: snapshot.isLoading && !snapshot.hasValue,
                    skeleton: Padding(
                      padding: EdgeInsets.all(tokens.spacing.x3),
                      child: HSkeleton(
                        rows: sandboxSkeletonRows,
                        rowHeight: tokens.sizes.rowHistory,
                      ),
                    ),
                    child: _Body(status: status),
                  ),
                },
              ),
              const HHairline(),
              SandboxStatusBar(
                status: status,
                onShowArgv: () => setState(() => _argvOpen = true),
              ),
            ],
          ),
          if (_argvOpen)
            Align(
              alignment: Alignment.centerRight,
              child: ArgvSheet(
                argv: status.argvPreview,
                onClose: () => setState(() => _argvOpen = false),
              ),
            ),
          if (_asking)
            StopDialog(onCancel: _cancelStop, onConfirm: _confirmStop),
        ],
      ),
    );
  }
}

/// The terminal above, the four tabs below.
class _Body extends ConsumerWidget {
  const _Body({required this.status});

  final SandboxStatus status;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final SandboxTab tab = ref.watch(sandboxTabChoiceProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(
          flex: (sandboxTerminalFraction * 100).round(),
          child: ComingPane(
            title: l10n.sandboxTerminalTitle,
            text: l10n.sandboxTerminalPlaceholder,
          ),
        ),
        const HHairline(),
        SandboxTabs<SandboxTab>(
          selected: tab,
          onSelect: ref.read(sandboxTabChoiceProvider.notifier).go,
          entries: <SandboxTabEntry<SandboxTab>>[
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.mounts,
              label: l10n.sandboxTabMounts,
              key: const Key('sandbox-tab-mounts'),
            ),
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.env,
              label: l10n.sandboxTabEnv,
              key: const Key('sandbox-tab-env'),
            ),
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.isolation,
              label: l10n.sandboxTabIsolation,
              key: const Key('sandbox-tab-isolation'),
            ),
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.log,
              label: l10n.sandboxTabLog,
              key: const Key('sandbox-tab-log'),
            ),
          ],
        ),
        const HHairline(),
        Expanded(
          flex: (100 - sandboxTerminalFraction * 100).round(),
          child: switch (tab) {
            SandboxTab.mounts => MountsTab(status: status),
            SandboxTab.env => EnvTab(status: status),
            SandboxTab.isolation => ComingPane(
              text: l10n.sandboxIsolationPlaceholder,
            ),
            SandboxTab.log => const LogTab(),
          },
        ),
      ],
    );
  }
}

/// Every finding of the last operation, newest last, right under the header.
///
/// A finding is anchored where the action was: the start button is above it,
/// and the reason it did not start stands directly below (`docs/UX.md` 4.4).
class _Diagnostics extends StatelessWidget {
  const _Diagnostics({required this.status});

  final SandboxStatus status;

  @override
  Widget build(BuildContext context) {
    if (status.diagnostics.isEmpty) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          for (final Diagnostic diagnostic in status.diagnostics)
            SandboxArrive(
              key: ValueKey<String>('diagnostic-${diagnostic.code}'),
              child: Padding(
                padding: EdgeInsets.only(bottom: tokens.spacing.x2),
                child: HDiagnosticCard(
                  code: diagnostic.code,
                  severityLabel: _severityLabel(l10n, diagnostic.severity),
                  color: _severityColor(tokens, diagnostic.severity),
                  title: diagnostic.title.isEmpty
                      ? diagnostic.code
                      : diagnostic.title,
                  // The daemon's own sentence. The application writes the
                  // title, never the reason (`docs/UX.md` 4.4).
                  why: diagnostic.why,
                  docsUrl: diagnostic.docsUrl,
                  fix: FixControl(fix: diagnostic.fix),
                  width: double.infinity,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// The status bar of the section: session, uptime, and the way to the proof.
class SandboxStatusBar extends StatelessWidget {
  /// Creates the bar for [status]. [onShowArgv] opens the command sheet.
  const SandboxStatusBar({
    required this.status,
    required this.onShowArgv,
    super.key,
  });

  /// What the daemon last said.
  final SandboxStatus status;

  /// Opens the command sheet.
  final VoidCallback onShowArgv;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SessionId? session = status.sessionId;
    final DateTime? started = status.startedAt;
    return Container(
      constraints: const BoxConstraints(minHeight: HSize.statusBar),
      color: tokens.colors.bg1,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
      child: Row(
        children: <Widget>[
          Text(
            session == null
                ? l10n.sandboxSessionNone
                : l10n.sandboxSession(session.short),
            style: tokens.typography.mono11.tinted(tokens.colors.fg2),
          ),
          if (started != null) ...<Widget>[
            SizedBox(width: tokens.spacing.x3),
            _Uptime(startedAt: started),
          ],
          const Spacer(),
          HButton(
            key: const Key('sandbox-show-argv'),
            variant: HButtonVariant.ghost,
            size: HButtonSize.sm,
            onPressed: onShowArgv,
            child: Text(l10n.sandboxShowArgv),
          ),
        ],
      ),
    );
  }
}

/// How long the sandbox has been up, counted in whole seconds.
///
/// Its own widget with its own timer, so the second that ticks rebuilds this
/// text and nothing else (`docs/UX.md` 7). The digits jump; they never roll
/// (2.9).
class _Uptime extends StatefulWidget {
  const _Uptime({required this.startedAt});

  final DateTime startedAt;

  @override
  State<_Uptime> createState() => _UptimeState();
}

class _UptimeState extends State<_Uptime> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(HMotion.clockTick, (Timer _) => setState(() {}));
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Text(
      l10n.sandboxUptime(
        sandboxUptimeText(DateTime.now().difference(widget.startedAt)),
      ),
      style: tokens.typography.mono11.tinted(tokens.colors.fg2),
    );
  }
}

/// The daemon did not answer. Its sentence, and the way to ask again.
class _CannotRead extends ConsumerWidget {
  const _CannotRead({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String why = error is DaemonException
        ? (error as DaemonException).diagnostic.why
        : '$error';
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            l10n.sandboxCannotRead(why),
            style: tokens.typography.ui13.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x3),
          HButton(
            key: const Key('sandbox-retry'),
            variant: HButtonVariant.secondary,
            onPressed: () =>
                unawaited(ref.read(sandboxStatusProvider.notifier).refresh()),
            child: Text(l10n.sandboxRetry),
          ),
        ],
      ),
    );
  }
}

/// The label of a severity, in the person's language.
String _severityLabel(AppLocalizations l10n, Severity severity) =>
    switch (severity) {
      Severity.info => l10n.diagSeverityInfo,
      Severity.warning => l10n.diagSeverityWarning,
      Severity.error => l10n.diagSeverityError,
      Severity.blocking => l10n.diagSeverityBlocking,
    };

/// The hue of a severity. Never the blocked red: red means blocked
/// (`docs/UX.md` 3.3, rule 6).
Color _severityColor(HTokens tokens, Severity severity) => switch (severity) {
  Severity.info => tokens.colors.accent,
  Severity.warning => tokens.state.held,
  Severity.error || Severity.blocking => tokens.state.error,
};
