/// The last maintenance run of an agent at its card (HUM-521 points 4 and 5).
///
/// While a run goes on: what runs, and the way to its terminal, which is the
/// terminal of the session like for any session. During a login: the URLs
/// the agent printed, with "Open in browser" only where the daemon marks one
/// openable (a login host of the adapter), and the field for the callback
/// URL only where the daemon says the adapter needs it. After a
/// configuration run: the changed files and Accept or Discard. After the
/// end: one line, or the findings of a failed run.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agents_provider.dart';
import 'agent_config_files.dart';
import 'agent_inset.dart';

/// The run [run] of [entry].
class AgentRunPanel extends ConsumerStatefulWidget {
  /// Creates the panel.
  const AgentRunPanel({
    required this.entry,
    required this.run,
    required this.busy,
    required this.onSend,
    this.onOpenSession,
    super.key,
  });

  /// The agent.
  final AgentDepot entry;

  /// Its last run.
  final AgentMaintenance run;

  /// A request is under way.
  final bool busy;

  /// Sends an operation of the run (Accept, Discard, the callback); the
  /// card shows a refusal.
  final Future<bool> Function(AgentsOp op) onSend;

  /// Shows the terminal of the running session; null without a shell.
  final VoidCallback? onOpenSession;

  @override
  ConsumerState<AgentRunPanel> createState() => _AgentRunPanelState();
}

class _AgentRunPanelState extends ConsumerState<AgentRunPanel> {
  final TextEditingController _callback = TextEditingController();
  bool _delivered = false;
  bool _openFailed = false;

  @override
  void initState() {
    super.initState();
    _callback.addListener(_typed);
  }

  @override
  void dispose() {
    _callback
      ..removeListener(_typed)
      ..dispose();
    super.dispose();
  }

  /// A new text: the line about the last delivery no longer speaks for it,
  /// and Send follows whether there is something to send.
  void _typed() => setState(() {
    if (_callback.text.isNotEmpty) {
      _delivered = false;
    }
  });

  Future<void> _open(String url) async {
    setState(() => _openFailed = false);
    final bool opened = await ref.read(loginUrlOpenerProvider)(url);
    if (mounted) {
      setState(() => _openFailed = !opened);
    }
  }

  Future<void> _deliver() async {
    final String url = _callback.text.trim();
    if (url.isEmpty) {
      return;
    }
    final bool taken = await widget.onSend(
      AgentsOp.deliverCallback(widget.entry.agent, url),
    );
    if (mounted) {
      setState(() => _delivered = taken);
      if (taken) {
        _callback.clear();
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentMaintenance run = widget.run;
    final String agent = widget.entry.agent;
    return switch (run.state) {
      AgentRunState.running => AgentInset(
        key: ValueKey<String>('agent-run-$agent'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            _RunningLine(
              agent: agent,
              text: switch (run.task) {
                AgentRunTask.install => l10n.settingsAgentsRunInstall,
                AgentRunTask.update => l10n.settingsAgentsRunUpdate,
                AgentRunTask.login => l10n.settingsAgentsRunLogin,
                _ => l10n.settingsAgentsRunConfig,
              },
              onOpenSession: widget.onOpenSession,
            ),
            if (run.loginUrls.isNotEmpty) ..._urls(tokens, l10n, run),
            if (run.callbackPaste) ..._callbackField(tokens, l10n),
          ],
        ),
      ),
      AgentRunState.confirm => AgentInset(
        key: ValueKey<String>('agent-run-$agent'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            AgentInsetTitle(l10n.settingsAgentsConfigChanged),
            SizedBox(height: tokens.spacing.x3),
            AgentConfigFiles(agent: agent, files: run.configChanges),
            AgentInsetActions(
              children: <Widget>[
                HButton(
                  key: ValueKey<String>('agent-config-discard-$agent'),
                  variant: HButtonVariant.secondary,
                  onPressed: widget.busy
                      ? null
                      : () => unawaited(
                          widget.onSend(AgentsOp.configDiscard(agent)),
                        ),
                  child: Text(l10n.settingsAgentsDiscard),
                ),
                HButton(
                  key: ValueKey<String>('agent-config-accept-$agent'),
                  variant: HButtonVariant.primary,
                  onPressed: widget.busy
                      ? null
                      : () => unawaited(
                          widget.onSend(AgentsOp.configAccept(agent)),
                        ),
                  child: Text(l10n.settingsAgentsAccept),
                ),
              ],
            ),
          ],
        ),
      ),
      AgentRunState.failed => Column(
        key: ValueKey<String>('agent-run-$agent'),
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            l10n.settingsAgentsFailed,
            style: tokens.typography.ui12.tinted(tokens.stateText.error),
          ),
          for (final Diagnostic found in run.diagnostics) ...<Widget>[
            SizedBox(height: tokens.spacing.x2),
            AgentFinding(diagnostic: found),
          ],
        ],
      ),
      _ => Row(
        key: ValueKey<String>('agent-run-$agent'),
        children: <Widget>[
          if (run.state == AgentRunState.discarded)
            HGlyphIcon(HGlyph.close, size: 12, color: tokens.colors.fg1)
          else
            HGlyphIcon(HGlyph.check, size: 12, color: tokens.stateText.allowed),
          SizedBox(width: tokens.spacing.x1),
          Flexible(
            child: Text(
              _doneLine(l10n, run),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
        ],
      ),
    };
  }

  List<Widget> _urls(
    HTokens tokens,
    AppLocalizations l10n,
    AgentMaintenance run,
  ) => <Widget>[
    SizedBox(height: tokens.spacing.x3),
    Text(
      l10n.settingsAgentsLoginUrls,
      style: tokens.typography.ui12.tinted(tokens.colors.fg2),
    ),
    for (int i = 0; i < run.loginUrls.length; i++) ...<Widget>[
      SizedBox(height: tokens.spacing.x2),
      _UrlRow(
        key: ValueKey<String>('agent-url-${widget.entry.agent}-$i'),
        url: run.loginUrls[i],
        index: i,
        agent: widget.entry.agent,
        onOpen: (String url) => unawaited(_open(url)),
      ),
    ],
    if (_openFailed) ...<Widget>[
      SizedBox(height: tokens.spacing.x1),
      Text(
        l10n.settingsAgentsOpenFailed,
        style: tokens.typography.ui12.tinted(tokens.stateText.error),
      ),
    ],
  ];

  List<Widget> _callbackField(HTokens tokens, AppLocalizations l10n) {
    final String agent = widget.entry.agent;
    return <Widget>[
      SizedBox(height: tokens.spacing.x3),
      Text(
        l10n.settingsAgentsCallback,
        style: tokens.typography.ui12.medium.tinted(tokens.colors.fg0),
      ),
      Text(
        l10n.settingsAgentsCallbackHelp,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      ),
      SizedBox(height: tokens.spacing.x2),
      Row(
        children: <Widget>[
          Expanded(
            child: HTextField(
              key: ValueKey<String>('agent-callback-input-$agent'),
              controller: _callback,
              semanticsLabel: l10n.settingsAgentsCallback,
              hint: l10n.settingsAgentsCallbackHint,
              onSubmitted: (String _) => unawaited(_deliver()),
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: ValueKey<String>('agent-callback-send-$agent'),
            onPressed: widget.busy || _callback.text.trim().isEmpty
                ? null
                : () => unawaited(_deliver()),
            child: Text(l10n.settingsAgentsCallbackSend),
          ),
        ],
      ),
      if (_delivered) ...<Widget>[
        SizedBox(height: tokens.spacing.x1),
        Text(
          l10n.settingsAgentsCallbackSent,
          key: ValueKey<String>('agent-callback-sent-$agent'),
          style: tokens.typography.ui12.tinted(tokens.stateText.allowed),
        ),
      ],
    ];
  }

  static String _doneLine(AppLocalizations l10n, AgentMaintenance run) =>
      switch (run.task) {
        _ when run.state == AgentRunState.discarded =>
          l10n.settingsAgentsDoneDiscard,
        AgentRunTask.login => l10n.settingsAgentsDoneLogin,
        AgentRunTask.config => l10n.settingsAgentsDoneConfig,
        AgentRunTask.install => l10n.settingsAgentsDoneInstall(run.version),
        _ when run.previous.isEmpty || run.previous == run.version =>
          l10n.settingsAgentsDoneCurrent(run.version),
        _ => l10n.settingsAgentsDoneUpdate(run.previous, run.version),
      };
}

/// What runs, and the way to its terminal.
class _RunningLine extends StatelessWidget {
  const _RunningLine({
    required this.agent,
    required this.text,
    required this.onOpenSession,
  });

  final String agent;
  final String text;
  final VoidCallback? onOpenSession;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final VoidCallback? open = onOpenSession;
    return Row(
      children: <Widget>[
        HGlyphIcon(HGlyph.hourglass, size: 14, color: tokens.colors.fg1),
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          child: Text(
            text,
            key: ValueKey<String>('agent-run-state-$agent'),
            style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
          ),
        ),
        if (open != null) ...<Widget>[
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: ValueKey<String>('agent-open-terminal-$agent'),
            onPressed: open,
            child: Text(context.l10n.settingsAgentsOpenTerminal),
          ),
        ],
      ],
    );
  }
}

/// One URL of a login: the URL itself, always; "Open in browser" only when
/// the daemon marks it openable. A URL of another host has no button, and
/// a click on its text opens nothing.
class _UrlRow extends StatelessWidget {
  const _UrlRow({
    required this.url,
    required this.index,
    required this.agent,
    required this.onOpen,
    super.key,
  });

  final AgentLoginUrl url;
  final int index;
  final String agent;
  final ValueChanged<String> onOpen;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              HMiddleEllipsis(
                url.url,
                textKey: ValueKey<String>('agent-url-text-$agent-$index'),
                style: tokens.typography.mono12.tinted(tokens.colors.fg0),
              ),
              if (!url.openable)
                Text(
                  l10n.settingsAgentsUrlNotOpenable,
                  key: ValueKey<String>('agent-url-shown-$agent-$index'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                ),
            ],
          ),
        ),
        if (url.openable) ...<Widget>[
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: ValueKey<String>('agent-url-open-$agent-$index'),
            variant: HButtonVariant.primary,
            leading: HGlyphIcon(
              HGlyph.arrowUpRight,
              size: 14,
              color: tokens.colors.onAccent,
            ),
            onPressed: () => onOpen(url.url),
            child: Text(l10n.settingsAgentsOpenInBrowser),
          ),
        ],
      ],
    );
  }
}
