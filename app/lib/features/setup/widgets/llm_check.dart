/// The second row: the model server, and the one gesture that contacts it
/// (HUM-044, HUM-039).
///
/// # What goes out, and when
///
/// The endpoint is the one address of this product that traffic reaches
/// **without** passing the queue (`BACKLOG.md` 4.2). `Doctor()` takes no
/// argument in which a client could ask for a connection, so its `llm` line
/// always arrives as `DOCTOR_013`, "not contacted". A connection happens on the
/// button of the field and on `Enter` in it, and in one more place: when Setup
/// opens, the host contacts the endpoint saved in the configuration once
/// (`SetupHost`, HUM-287), and nothing while none is saved. Never while
/// somebody types, because the name would be in DNS before anybody had decided
/// on it.
///
/// # The model names came from the network
///
/// They are what an unauthenticated server on the LAN answered, word for word.
/// The daemon caps the body and sanitises every string, [LlmProbe.shownModels]
/// caps how many and how long, and the chips below draw them as text and as
/// nothing else. **A model name never becomes a command**, a link or a path.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/llm_endpoint_field.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';
import 'setup_check_row.dart';

/// The model row.
class LlmCheck extends StatelessWidget {
  /// Creates the row for [check].
  const LlmCheck({
    required this.check,
    required this.probe,
    required this.controller,
    required this.onProbe,
    required this.onDiscover,
    required this.onEdited,
    required this.enabled,
    this.activeServer = '',
    this.view = SetupStepView.focused,
    this.agent,
    super.key,
  });

  /// The doctor's line about the agent, shown as a line of this step
  /// (HUM-278, HUM-287).
  ///
  /// Shown, not folded in: the agent stays one of the machine's lines and
  /// decides the machine step as before, so a model server that answered is
  /// never reported as unmeasured because of another check. Here it only
  /// says which agent a session would run with the model above it.
  final DoctorCheck? agent;

  /// Open or one line, in front or not (HUM-287).
  final SetupStepView view;

  /// The name of the active saved server whose endpoint the field shows,
  /// or empty when no server is saved (HUM-289).
  final String activeServer;

  /// What the row says.
  final SetupCheck check;

  /// What the last probe answered, or nothing.
  final LlmProbeState probe;

  /// The text of the endpoint field.
  final TextEditingController controller;

  /// Called with the endpoint somebody wants contacted.
  final void Function(String endpoint) onProbe;

  /// Opens the search for servers in the local network (HUM-076).
  ///
  /// Opening it contacts nothing; the button inside the sheet starts the
  /// search, and the sentence above that button says what it will do.
  final VoidCallback onDiscover;

  /// Called with every text a person types into the field.
  final ValueChanged<String> onEdited;

  /// False while no daemon can carry out a probe.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Verlangt der Befund einen anderen Schlüssel oder eine andere Einstellung
    // und kann das Blatt der Einstellungen das (HUM-275), ist dessen Knopf
    // die eine Aktion dieses Schritts; sonst ist es „Ansprechen" (HUM-287).
    final bool fixIsPrimary = FixControl.offersButton(
      context,
      check.diagnostic?.fix,
    );
    final LlmProbe? answered = probe.probe;
    final DoctorCheck? agent = this.agent;
    return SetupCheckRow(
      check: check,
      title: setupCheckTitle(l10n, SetupCheckKind.llm),
      view: view,
      primaryFix: fixIsPrimary,
      // Die eine Zeile sagt dasselbe wie das offene Feld: was gemessen wurde,
      // oder dass niemand gefragt hat.
      summary: answered != null ? _measured(l10n, answered) : null,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          if (activeServer.isNotEmpty) ...<Widget>[
            Text(
              l10n.setupLlmActiveServer(activeServer),
              key: const Key('setup-llm-active-server'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x1),
          ],
          LlmEndpointField(
            controller: controller,
            label: l10n.setupLlmEndpointLabel,
            hint: l10n.setupLlmEndpointHint,
            probeLabel: l10n.setupLlmProbe,
            onProbe: onProbe,
            onChanged: onEdited,
            busy: probe.busy,
            enabled: enabled,
            primary: view.current && !fixIsPrimary,
          ),
          SizedBox(height: tokens.spacing.x2),
          if (answered != null)
            _Models(probe: answered)
          else
            Text(
              check.detail.isEmpty
                  ? setupNoEvidence(l10n, check.state)
                  : check.detail,
              key: const Key('setup-evidence-llm'),
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          // Der Agent ist eine Zeile dieses Schritts: Er und sein Modell sind
          // das, was eine Session laufen lässt (HUM-278, HUM-287).
          if (agent != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            _AgentLine(line: agent),
          ],
        ],
      ),
      actions: <Widget>[
        // Der zweite Weg zu einer Adresse, für alle, die keine kennen
        // (Prinzip 9). Er steht neben dem Feld und nicht darin: Suchen ist
        // ein eigener Vorgang mit eigener Ankündigung, kein Zubehör eines
        // Textfeldes.
        HButton(
          key: const Key('setup-llm-discover'),
          variant: HButtonVariant.ghost,
          onPressed: enabled ? onDiscover : null,
          child: Text(l10n.setupLlmDiscoverOpen),
        ),
      ],
    );
  }
}

/// The sentence a probe left behind, for the one line of the step.
String _measured(AppLocalizations l10n, LlmProbe probe) =>
    probe.models.isEmpty && probe.answer.isNotEmpty
    ? l10n.setupLlmNoModelList(probe.endpoint, probe.latencyMs, probe.answer)
    : l10n.setupLlmMeasured(
        probe.endpoint,
        probe.models.length,
        probe.latencyMs,
      );

/// The doctor's line about the agent, inside the model step (HUM-278).
///
/// Mark, label, state word and the daemon's evidence -- the command it
/// found, or the directories it searched. Its finding, when it has one,
/// stands in the machine step, which this line still belongs to.
class _AgentLine extends StatelessWidget {
  const _AgentLine({required this.line});

  final DoctorCheck line;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SetupCheckState state = doctorLineState(line);
    return Row(
      key: const Key('setup-agent-line'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SetupMark(state: state),
        SizedBox(width: tokens.spacing.x2),
        Text(
          l10n.setupAgentLine,
          style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
        ),
        SizedBox(width: tokens.spacing.x2),
        Text(
          setupStateLabel(l10n, state),
          key: const Key('setup-agent-state'),
          style: tokens.typography.ui12.tinted(
            setupStateTextColor(tokens, state),
          ),
        ),
        SizedBox(width: tokens.spacing.x3),
        Expanded(
          child: Text(
            line.evidence,
            key: const Key('setup-agent-evidence'),
            style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );
  }
}

/// What the endpoint answered: which API, how long it took, which models.
class _Models extends StatelessWidget {
  const _Models({required this.probe});

  final LlmProbe probe;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final int hidden = probe.hiddenModels;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          // Ohne Modellliste sagt die Zeile, was zurückkam, statt "0 Modelle"
          // neben `LLM_003` zu behaupten (HUM-283).
          probe.models.isEmpty && probe.answer.isNotEmpty
              ? l10n.setupLlmNoModelList(
                  probe.endpoint,
                  probe.latencyMs,
                  probe.answer,
                )
              : l10n.setupLlmMeasured(
                  probe.endpoint,
                  probe.models.length,
                  probe.latencyMs,
                ),
          key: const Key('setup-evidence-llm'),
          style: tokens.typography.mono12.tinted(tokens.colors.fg1),
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
        ),
        if (probe.shownModels.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            key: const Key('setup-llm-models'),
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x1,
            children: <Widget>[
              for (final String model in probe.shownModels)
                // Ein Name aus dem Netz ist Text und nur Text: kein
                // `CopyCommand`, kein Link, kein Pfad. Eine Zeile, höchstens,
                // damit ein Name mit vierzig Zeichen die Zeile nicht sprengt.
                HBadge(text: model),
              if (hidden > 0) HBadge(text: l10n.setupLlmMoreModels(hidden)),
            ],
          ),
        ],
      ],
    );
  }
}
