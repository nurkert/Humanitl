/// The saved model servers in the inference step of the guided agent
/// setup (GitHub #111).
///
/// One row per saved server, as `LlmServers` lists it: the name, the host,
/// "Local" for a server of the own network (`LlmServer.local`, #115) and
/// the star of the default; a server the daemon calls not available for
/// this project (`LLM_014`) keeps its row, disabled, with the reason. Under
/// the rows a form adds a server with name and address (`LlmServers(add)`).
///
/// A chosen server is tested first (`ProbeLlm` with its own key, only to
/// its own origin, only on the button) and taken over only after a clean
/// answer: for this project in its start choice, or for all projects as the
/// default server (ADR-0025).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../core/ui/untranslated_wording.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agent_setup.dart';
import '../start_page.dart';
import 'setup_choice.dart';
import 'setup_step_frame.dart';
import 'setup_view.dart';

/// The server [setup] would start with: the one the project uses now,
/// else the default, else the first saved one; empty without any.
String initialServer(AgentSetup setup) {
  final List<SavedServer> servers =
      setup.servers?.servers ?? const <SavedServer>[];
  final String project = setup.proposal?.field('llm_server')?.value ?? '';
  if (project.isNotEmpty && setup.servers?.named(project) != null) {
    return project;
  }
  final String active = setup.servers?.activeServer?.name ?? '';
  if (active.isNotEmpty) {
    return active;
  }
  return servers.isEmpty ? '' : servers.first.name;
}

/// Whether the saved server [name] may be taken over now: tested with a
/// clean answer, available for the project, and nothing under way.
bool serverUsable(AgentSetup setup, String name) {
  final LlmProbe? probe = setup.probedServer == name ? setup.probe : null;
  return setup.servers?.named(name) != null &&
      probe != null &&
      probe.isClean &&
      (_option(setup.proposal?.field('llm_server'), name)?.available ?? true) &&
      !setup.busy;
}

ProposalOption? _option(ProposalField? field, String name) {
  for (final ProposalOption option
      in field?.options ?? const <ProposalOption>[]) {
    if (option.value == name) {
      return option;
    }
  }
  return null;
}

/// The servers, the test and the use. The chosen server and the level live
/// with the step, which needs them to know whether its one primary action
/// is open.
class InferenceServers extends ConsumerStatefulWidget {
  /// Creates the list over [view]. Using a server also sets
  /// `agents.<id>.inference` back to the model server when it was `own`
  /// ([AgentSetupNotifier.useServer]).
  const InferenceServers({
    required this.view,
    required this.chosen,
    required this.onChosen,
    required this.level,
    required this.onLevel,
    super.key,
  });

  /// The setup as this step sees it.
  final SetupView view;

  /// The chosen server.
  final String chosen;

  /// Chooses a server.
  final ValueChanged<String> onChosen;

  /// Where the chosen server is saved.
  final ServerLevel level;

  /// Chooses the level.
  final ValueChanged<ServerLevel> onLevel;

  @override
  ConsumerState<InferenceServers> createState() => _InferenceServersState();
}

class _InferenceServersState extends ConsumerState<InferenceServers> {
  bool _adding = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentSetup setup = widget.view.setup;
    final List<SavedServer> servers =
        setup.servers?.servers ?? const <SavedServer>[];
    final ProposalField? field = setup.proposal?.field('llm_server');
    final String chosen = widget.chosen;
    final ServerLevel level = widget.level;
    final SavedServer? server = setup.servers?.named(chosen);
    final LlmProbe? probe = setup.probedServer == chosen ? setup.probe : null;
    final AgentSetupNotifier notifier = ref.read(agentSetupProvider.notifier);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (servers.isEmpty && !_adding)
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x3),
            child: Text(
              l10n.agentSetupInferenceNoServers,
              key: const Key('agent-setup-servers-empty'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          ),
        if (servers.isNotEmpty)
          StartCard(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                for (final (int i, SavedServer each) in servers.indexed) ...[
                  if (i > 0) const HHairline(),
                  _ServerRow(
                    server: each,
                    option: _option(field, each.name),
                    selected: each.name == chosen,
                    onSelect: () => widget.onChosen(each.name),
                  ),
                ],
              ],
            ),
          ),
        SizedBox(height: tokens.spacing.x2),
        if (_adding)
          _AddServer(
            busy: setup.busy,
            onCancel: () => setState(() => _adding = false),
            onSave: (String name, String endpoint) async {
              await notifier.addServer(name, endpoint);
              if (mounted && ref.read(agentSetupProvider)?.refusal == null) {
                setState(() => _adding = false);
                widget.onChosen(name);
              }
            },
          )
        else
          Align(
            alignment: Alignment.centerLeft,
            child: HButton(
              key: const Key('agent-setup-server-add'),
              variant: HButtonVariant.ghost,
              leading: HGlyphIcon(
                HGlyph.plus,
                size: 14,
                color: tokens.colors.fg1,
              ),
              onPressed: setup.busy
                  ? null
                  : () => setState(() => _adding = true),
              child: Text(l10n.agentSetupInferenceAdd),
            ),
          ),
        if (server != null) ...<Widget>[
          SizedBox(height: tokens.spacing.x4),
          _UseRow(
            level: level,
            projectKnown: setup.target.known,
            onLevel: widget.onLevel,
            probing: setup.probing,
            canTest: !setup.busy && !setup.probing,
            canUse: serverUsable(setup, chosen),
            serverName: server.name,
            onTest: () => unawaited(notifier.probe(server)),
            onUse: () => unawaited(notifier.useServer(server.name, level)),
          ),
          SizedBox(height: tokens.spacing.x2),
          _ProbeLine(probe: probe, serverName: server.name),
        ],
      ],
    );
  }
}

/// One saved server.
class _ServerRow extends StatefulWidget {
  const _ServerRow({
    required this.server,
    required this.option,
    required this.selected,
    required this.onSelect,
  });

  final SavedServer server;
  final ProposalOption? option;
  final bool selected;
  final VoidCallback onSelect;

  @override
  State<_ServerRow> createState() => _ServerRowState();
}

class _ServerRowState extends State<_ServerRow> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer server = widget.server;
    final ProposalOption? option = widget.option;
    final bool available = option?.available ?? true;
    final Color fill = widget.selected
        ? tokens.colors.bg2
        : (_hovered ? tokens.colors.bg2 : const Color(0x00000000));
    return Semantics(
      inMutuallyExclusiveGroup: true,
      checked: widget.selected,
      button: true,
      label: server.name,
      child: FocusableActionDetector(
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        mouseCursor: SystemMouseCursors.click,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              widget.onSelect();
              return null;
            },
          ),
        },
        child: GestureDetector(
          key: Key('agent-setup-server-${server.name}'),
          behavior: HitTestBehavior.opaque,
          onTap: widget.onSelect,
          child: HFocusRing.inline(
            visible: _focused,
            child: Container(
              constraints: BoxConstraints(minHeight: tokens.sizes.row + 8),
              padding: EdgeInsets.symmetric(
                horizontal: tokens.spacing.x3,
                vertical: tokens.spacing.x2,
              ),
              decoration: BoxDecoration(color: fill),
              child: Row(
                children: <Widget>[
                  _RadioMark(selected: widget.selected),
                  SizedBox(width: tokens.spacing.x3),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        Row(
                          children: <Widget>[
                            Flexible(
                              child: Text(
                                server.name,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: tokens.typography.mono12.medium.tinted(
                                  available
                                      ? tokens.colors.fg0
                                      : tokens.colors.fg2,
                                ),
                              ),
                            ),
                            if (server.active) ...<Widget>[
                              SizedBox(width: tokens.spacing.x2),
                              HGlyphIcon(
                                HGlyph.star,
                                size: 12,
                                color: tokens.colors.accent,
                              ),
                              SizedBox(width: tokens.spacing.x1),
                              Text(
                                l10n.agentSetupInferenceDefault,
                                style: tokens.typography.ui11.medium.tinted(
                                  tokens.colors.accentText,
                                ),
                              ),
                            ],
                          ],
                        ),
                        HMiddleEllipsis(
                          displayEndpoint(server.endpoint),
                          style: tokens.typography.mono11.tinted(
                            tokens.colors.fg2,
                          ),
                        ),
                        if (!available &&
                            (option?.reasons.isNotEmpty ?? false)) ...<Widget>[
                          SizedBox(height: tokens.spacing.x1),
                          DiagnosticCause(
                            diagnostic: option!.reasons.first,
                            style: tokens.typography.ui12.tinted(
                              tokens.colors.fg2,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                  if (server.local) ...<Widget>[
                    SizedBox(width: tokens.spacing.x3),
                    HBadge(
                      key: Key('agent-setup-server-local-${server.name}'),
                      text: l10n.agentSetupInferenceLocal,
                      color: tokens.state.allowed,
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _RadioMark extends StatelessWidget {
  const _RadioMark({required this.selected});

  final bool selected;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      width: 14,
      height: 14,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        border: Border.all(
          color: selected ? tokens.colors.fg0 : tokens.colors.lineStrong,
          width: HSize.ringStroke,
        ),
      ),
      child: selected
          ? Container(
              width: 6,
              height: 6,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: tokens.colors.fg0,
              ),
            )
          : null,
    );
  }
}

/// Where the server is saved, its test, and its use.
class _UseRow extends StatelessWidget {
  const _UseRow({
    required this.level,
    required this.projectKnown,
    required this.onLevel,
    required this.probing,
    required this.canTest,
    required this.canUse,
    required this.serverName,
    required this.onTest,
    required this.onUse,
  });

  final ServerLevel level;
  final bool projectKnown;
  final ValueChanged<ServerLevel> onLevel;
  final bool probing;
  final bool canTest;
  final bool canUse;
  final String serverName;
  final VoidCallback onTest;
  final VoidCallback onUse;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Die Ebene steht über ihrer Wahl, und beide Teile brechen um, statt
    // sich in einer schmalen Spalte zu verstecken (deutsche Wörter sind
    // länger); Test und Übernahme darunter an derselben Kante.
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SetupLabelledRow(
          label: l10n.agentSetupInferenceSaveFor,
          stacked: true,
          child: HChoiceChips<ServerLevel>(
            key: const Key('agent-setup-server-level'),
            selected: <ServerLevel>{level},
            options: <HSegmentOption<ServerLevel>>[
              HSegmentOption<ServerLevel>(
                value: ServerLevel.project,
                label: l10n.agentSetupInferenceForProject,
                enabled: projectKnown,
              ),
              HSegmentOption<ServerLevel>(
                value: ServerLevel.all,
                label: l10n.agentSetupInferenceForAll,
              ),
            ],
            // Zwei Teile, von denen genau einer gilt: Ein Klick wählt ihn,
            // ein zweiter auf denselben lässt ihn gewählt.
            onToggle: onLevel,
          ),
        ),
        SizedBox(height: tokens.spacing.x3),
        Wrap(
          spacing: tokens.spacing.x2,
          runSpacing: tokens.spacing.x2,
          children: <Widget>[
            HButton(
              key: const Key('agent-setup-server-test'),
              onPressed: canTest ? onTest : null,
              child: Text(
                probing
                    ? l10n.agentSetupInferenceTesting
                    : l10n.agentSetupInferenceTest,
              ),
            ),
            HButton(
              key: const Key('agent-setup-server-use'),
              variant: HButtonVariant.primary,
              onPressed: canUse ? onUse : null,
              child: Text(l10n.agentSetupInferenceUse(serverName)),
            ),
          ],
        ),
      ],
    );
  }
}

/// The answer of the last test of the chosen server, or what to do first.
class _ProbeLine extends StatelessWidget {
  const _ProbeLine({required this.probe, required this.serverName});

  final LlmProbe? probe;
  final String serverName;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final LlmProbe? probe = this.probe;
    if (probe == null) {
      return Text(
        l10n.agentSetupInferenceTestFirst,
        key: const Key('agent-setup-probe-none'),
        style: tokens.typography.ui12.tinted(tokens.colors.fg2),
      );
    }
    final Diagnostic? first = probe.first;
    if (first != null) {
      return DiagnosticCause(
        diagnostic: first,
        textKey: const Key('agent-setup-probe-failed'),
        style: tokens.typography.ui12.tinted(tokens.stateText.error),
      );
    }
    return SetupNote(
      key: const Key('agent-setup-probe-ok'),
      glyph: HGlyph.check,
      color: tokens.stateText.allowed,
      text: l10n.agentSetupInferenceAnswered(serverName, probe.models.length),
    );
  }
}

/// The form for a new server: a name and an address.
class _AddServer extends StatefulWidget {
  const _AddServer({
    required this.busy,
    required this.onCancel,
    required this.onSave,
  });

  final bool busy;
  final VoidCallback onCancel;
  final Future<void> Function(String name, String endpoint) onSave;

  @override
  State<_AddServer> createState() => _AddServerState();
}

class _AddServerState extends State<_AddServer> {
  final TextEditingController _name = TextEditingController();
  final TextEditingController _endpoint = TextEditingController();

  @override
  void initState() {
    super.initState();
    _name.addListener(_changed);
    _endpoint.addListener(_changed);
  }

  @override
  void dispose() {
    _name.dispose();
    _endpoint.dispose();
    super.dispose();
  }

  void _changed() => setState(() {});

  bool get _complete =>
      isServerName(_name.text.trim()) && _endpoint.text.trim().isNotEmpty;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    Widget field(String label, Widget input) => Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      child: SetupLabelledRow(label: label, child: input),
    );
    return Container(
      key: const Key('agent-setup-server-form'),
      padding: EdgeInsets.all(tokens.spacing.x3),
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border.all(color: tokens.colors.line),
        borderRadius: BorderRadius.circular(tokens.radii.card),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          field(
            l10n.agentSetupInferenceAddAddress,
            HTextField(
              key: const Key('agent-setup-server-endpoint'),
              controller: _endpoint,
              semanticsLabel: l10n.agentSetupInferenceAddAddress,
              hint: l10n.agentSetupInferenceAddAddressHint,
              onChanged: (String text) {
                if (_name.text.isEmpty || _name.text == _proposed) {
                  _proposed = proposeServerName(text.trim());
                  _name.text = _proposed;
                }
              },
            ),
          ),
          field(
            l10n.agentSetupInferenceAddName,
            HTextField(
              key: const Key('agent-setup-server-name'),
              controller: _name,
              semanticsLabel: l10n.agentSetupInferenceAddName,
              hint: l10n.agentSetupInferenceAddNameHint,
            ),
          ),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: tokens.spacing.x2,
            children: <Widget>[
              HButton(
                key: const Key('agent-setup-server-cancel'),
                variant: HButtonVariant.ghost,
                onPressed: widget.onCancel,
                child: Text(l10n.agentSetupCancel),
              ),
              HButton(
                key: const Key('agent-setup-server-save'),
                onPressed: _complete && !widget.busy
                    ? () => unawaited(
                        widget.onSave(_name.text.trim(), _endpoint.text.trim()),
                      )
                    : null,
                child: Text(l10n.agentSetupInferenceAddSave),
              ),
            ],
          ),
        ],
      ),
    );
  }

  /// The name the address proposed last; a name typed by hand stays.
  String _proposed = '';
}
