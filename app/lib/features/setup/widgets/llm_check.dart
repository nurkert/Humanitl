/// The second row: the model server, and the one gesture that contacts it
/// (HUM-044, HUM-039).
///
/// # Nothing here goes out on its own
///
/// The endpoint is the one address of this product that traffic reaches
/// **without** passing the queue (`BACKLOG.md` 4.2). Opening this screen
/// contacts nothing: `Doctor()` takes no argument in which a client could ask
/// for a connection, so its `llm` line always arrives as `DOCTOR_013`, "not
/// contacted". A connection happens on the button of the field and on `Enter`
/// in it, and nowhere else -- never while somebody types, because the name
/// would be in DNS before anybody had decided on it.
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
    super.key,
  });

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
    return SetupCheckRow(
      check: check,
      title: setupCheckTitle(l10n, SetupCheckKind.llm),
      detail: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          LlmEndpointField(
            controller: controller,
            label: l10n.setupLlmEndpointLabel,
            hint: l10n.setupLlmEndpointHint,
            probeLabel: l10n.setupLlmProbe,
            onProbe: onProbe,
            onChanged: onEdited,
            busy: probe.busy,
            enabled: enabled,
          ),
          SizedBox(height: tokens.spacing.x2),
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
          SizedBox(height: tokens.spacing.x2),
          if (probe.probe case final LlmProbe answered)
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
        ],
      ),
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
          l10n.setupLlmMeasured(
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
