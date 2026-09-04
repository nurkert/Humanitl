/// Der Rumpf der ausgewählten Anfrage, in vier Ansichten.
///
/// Jeder Rumpf hier kommt aus dem Netz, durch einen Agenten, den niemand
/// kontrolliert, und ein Mensch entscheidet auf dieser Grundlage über die
/// Freigabe. Vier Zusagen halten diese Ansicht deshalb ein, und sie sind der
/// Grund für fast jede Entscheidung in dieser Datei und ihren Nachbarn:
///
/// * **Nichts wird ausgeführt und nichts wird verlinkt.** Kein Markdown, kein
///   Link, kein Berührungsziel im Inhalt. Anfassbar ist genau der Umschalter
///   und das Aufklappen im Baum — beides gehört uns, nicht dem Absender.
/// * **Der Inhalt bleibt als Inhalt erkennbar.** Alles steht in Monospace auf
///   der `fg`-Leiter, in einer Fläche mit eigener Kopfzeile; die einzige
///   Chroma sind Funde (`docs/UX.md` 3.3, Regel 7). Ein Rumpf, der wie eine
///   Meldung dieses Programms geschrieben ist, sieht trotzdem aus wie ein
///   Rumpf.
/// * **Kein Zeichen dreht die Anzeige um.** Richtungssteuerzeichen und
///   unsichtbare Trenner werden vor dem Zeichnen ersetzt, längentreu, damit
///   die Fundstellen bleiben, wo sie sind (`sanitizeBodyText`).
/// * **Was fehlt, wird gesagt.** Ein abgebrochener Strom, ein zu großer Rumpf,
///   ein gekappter Baum, ein Fund, der in dieser Ansicht nicht sitzt: alles
///   steht als Satz über der Ansicht. „Leer" und „nicht lesbar" sind zwei
///   Aussagen, und die zweite darf nie wie die erste aussehen.
library;

import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/h_collapsible.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../finding_text.dart';
import '../format.dart';
import '../providers/flow_body_provider.dart';
import 'body_decode.dart';
import 'body_kind.dart';
import 'body_marks.dart';
import 'body_parser.dart';
import 'body_span.dart';
import 'form_view.dart';
import 'hex_view.dart';
import 'json_tree_view.dart';
import 'raw_view.dart';

/// Wie hoch die Ansicht höchstens wird.
///
/// Sie sitzt in einer scrollenden Karte, also braucht sie eine Grenze; kurze
/// Rümpfe bleiben darunter, statt leere Fläche zu belegen.
const double bodyViewMaxHeight = 320;

/// Wie viele Zeilen das Skelett zeigt, solange der Rumpf unterwegs ist.
const int bodyViewSkeletonRows = 6;

/// Der Rumpf-Abschnitt der Anfragekarte.
class BodyView extends ConsumerStatefulWidget {
  /// Creates the section for [flowId] and [body].
  const BodyView({required this.flowId, required this.body, super.key});

  /// Der Flow, zu dem der Rumpf gehört.
  final FlowId flowId;

  /// Der Verweis auf den Rumpf, oder null, solange das Detail unterwegs ist.
  final BodyRef? body;

  @override
  ConsumerState<BodyView> createState() => _BodyViewState();
}

class _BodyViewState extends ConsumerState<BodyView> {
  BodyFinding? _hovered;
  BodyFinding? _focused;

  void _hover(BodyFinding? finding) {
    if (_hovered != finding) {
      setState(() => _hovered = finding);
    }
  }

  void _focus(BodyFinding finding) => setState(() => _focused = finding);

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final BodyRef? body = widget.body;
    if (body == null) {
      // Solange das Detail fehlt, wird nichts behauptet: weder „kein Rumpf"
      // noch eine Größe (`docs/UX.md` 2.11).
      return HCollapsible(
        title: l10n.interceptSectionBodyPending,
        child: const HSkeleton(
          rows: bodyViewSkeletonRows,
          rowHeight: HSize.rowBody,
        ),
      );
    }
    final String contentType = body.contentType.isEmpty
        ? l10n.interceptContentTypeUnknown
        : body.contentType;
    return HCollapsible(
      title: l10n.interceptSectionBody(formatBytes(body.size), contentType),
      child: body.isEmpty
          ? Text(
              l10n.interceptBodyEmpty,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            )
          : _Loaded(
              flowId: widget.flowId,
              body: body,
              hovered: _hovered,
              focused: _focused,
              onHover: _hover,
              onFocus: _focus,
            ),
    );
  }
}

class _Loaded extends ConsumerWidget {
  const _Loaded({
    required this.flowId,
    required this.body,
    required this.hovered,
    required this.focused,
    required this.onHover,
    required this.onFocus,
  });

  final FlowId flowId;
  final BodyRef body;
  final BodyFinding? hovered;
  final BodyFinding? focused;
  final ValueChanged<BodyFinding?> onHover;
  final ValueChanged<BodyFinding> onFocus;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final AsyncValue<ParsedBody> parsed = ref.watch(
      parsedBodyProvider(flowId, body),
    );
    final ParsedBody? value = parsed.value;
    if (parsed.hasError && value == null) {
      return Text(
        l10n.interceptBodyUnreadable,
        key: const Key('body-unreadable'),
        style: tokens.typography.ui12.tinted(
          tokens.stateTextOf(tokens.stateColor(HFlowState.error)),
        ),
      );
    }
    return HWait(
      loading: value == null,
      skeleton: const HSkeleton(
        rows: bodyViewSkeletonRows,
        rowHeight: HSize.rowBody,
      ),
      child: value == null
          ? const SizedBox.shrink()
          : _Panes(
              flowId: flowId,
              body: body,
              parsed: value,
              hovered: hovered,
              focused: focused,
              onHover: onHover,
              onFocus: onFocus,
            ),
    );
  }
}

class _Panes extends ConsumerWidget {
  const _Panes({
    required this.flowId,
    required this.body,
    required this.parsed,
    required this.hovered,
    required this.focused,
    required this.onHover,
    required this.onFocus,
  });

  final FlowId flowId;
  final BodyRef body;
  final ParsedBody parsed;
  final BodyFinding? hovered;
  final BodyFinding? focused;
  final ValueChanged<BodyFinding?> onHover;
  final ValueChanged<BodyFinding> onFocus;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final BodyKind kind = parsed.problem == BodyProblem.tooLarge
        ? BodyKind.tooLarge
        : parsed.kind;
    final List<BodyPane> panes = availableBodyPanes(parsed, kind);
    final BodyPane? chosen = ref.watch(bodyViewModeProvider(flowId));
    final BodyPane pane = panes.isEmpty
        ? BodyPane.raw
        : (chosen != null && panes.contains(chosen) ? chosen : panes.first);
    // Die Hex-Ansicht liest denselben Byteraum, in dem die Fundstellen
    // liegen: den ausgepackten, wenn ausgepackt wurde, sonst den rohen.
    final Uint8List bytes = parsedBodyBytes(
      parsed,
      ref.watch(flowBodyProvider(body)).value,
    );
    final List<String> notes = bodyNotes(parsed, pane, bytes.length, l10n);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Row(
          children: <Widget>[
            if (panes.length > 1)
              HSegmented<BodyPane>(
                options: <HSegmentOption<BodyPane>>[
                  for (final BodyPane option in panes)
                    HSegmentOption<BodyPane>(
                      value: option,
                      label: bodyPaneLabel(option, l10n),
                    ),
                ],
                selected: pane,
                onSelect: (BodyPane chosen) => ref
                    .read(bodyViewModeProvider(flowId).notifier)
                    .select(chosen),
              ),
            SizedBox(width: tokens.spacing.x2),
            Expanded(
              // Der Platz der Zeile steht immer, auch wenn nichts darin steht:
              // Hover verschiebt nichts (`docs/UX.md` 2.9).
              child: SizedBox(
                height: _lineHeight(context, tokens),
                child: hovered == null
                    ? null
                    : Text(
                        l10n.interceptBodyFindingHover(
                          _findingName(hovered!, l10n),
                          _tierName(hovered!, l10n),
                        ),
                        key: const Key('body-finding-hover'),
                        style: tokens.typography.ui11.tinted(
                          tokens.stateTextOf(
                            bodyFindingColor(tokens, hovered!.tone),
                          ),
                        ),
                        maxLines: 1,
                      ),
              ),
            ),
          ],
        ),
        if (parsed.findings.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x1),
          Wrap(
            spacing: tokens.spacing.x1,
            runSpacing: tokens.spacing.x1,
            children: <Widget>[
              for (final BodyFinding finding in parsed.findings)
                HBadge(
                  key: Key('body-finding-chip-${finding.index}'),
                  text: _findingName(finding, l10n),
                  color: bodyFindingColor(tokens, finding.tone),
                  onTap: () => onFocus(finding),
                ),
            ],
          ),
        ],
        SizedBox(height: tokens.spacing.x2),
        for (final String note in notes) ...<Widget>[
          Text(note, style: tokens.typography.ui11.tinted(tokens.colors.fg1)),
          SizedBox(height: tokens.spacing.x1),
        ],
        ConstrainedBox(
          constraints: BoxConstraints(
            maxHeight: bodyViewMaxHeight,
            minHeight: HSize.rowBody * 2,
          ),
          child: SizedBox(
            height: _height(context, pane),
            child: _pane(pane, bytes),
          ),
        ),
      ],
    );
  }

  /// Die Höhe einer Zeile der kleinsten Schrift, skaliert.
  double _lineHeight(BuildContext context, HTokens tokens) {
    final TextStyle style = tokens.typography.ui11;
    return MediaQuery.textScalerOf(context)
        .scale((style.fontSize ?? 11) * (style.height ?? 1));
  }

  /// Wie hoch die Ansicht wird: so hoch wie ihr Inhalt, höchstens
  /// [bodyViewMaxHeight].
  double _height(BuildContext context, BodyPane pane) {
    final double row = MediaQuery.textScalerOf(context).scale(HSize.rowBody);
    final int rows = switch (pane) {
      BodyPane.raw => parsed.text?.rows.length ?? 1,
      BodyPane.form => parsed.form?.length ?? 1,
      BodyPane.hex => 24,
      BodyPane.tree => 24,
    };
    final double wanted = row * (rows < 1 ? 1 : rows);
    return wanted > bodyViewMaxHeight ? bodyViewMaxHeight : wanted;
  }

  Widget _pane(BodyPane pane, Uint8List bytes) {
    final BodyText? text = parsed.text;
    switch (pane) {
      case BodyPane.tree:
        final JsonDocument? document = parsed.json;
        if (document == null) {
          return const SizedBox.shrink();
        }
        return JsonTreeView(
          key: const Key('body-tree'),
          document: document,
          findings: parsed.placedFindings,
          focus: focused,
          onHover: onHover,
        );
      case BodyPane.form:
        return FormView(
          key: const Key('body-form'),
          pairs: parsed.form ?? const <FormPair>[],
          findings: parsed.placedFindings,
          focus: focused,
          onHover: onHover,
        );
      case BodyPane.raw:
        if (text == null) {
          return const SizedBox.shrink();
        }
        return RawBodyView(
          key: const Key('body-raw'),
          text: text,
          findings: parsed.placedFindings,
          focus: focused,
          onHover: onHover,
        );
      case BodyPane.hex:
        return HexView(
          key: const Key('body-hex'),
          bytes: bytes,
          findings: parsed.placedFindings,
          limit: bodyHexLimit,
          focus: focused,
          onHover: onHover,
        );
    }
  }

  String _findingName(BodyFinding finding, AppLocalizations l10n) =>
      findingName(
        Finding(
          kind: finding.kind,
          location: FindingLocation.body,
          spanStart: finding.byteStart,
          spanEnd: finding.byteEnd,
          tier: finding.tier,
        ),
        l10n,
      );

  String _tierName(BodyFinding finding, AppLocalizations l10n) =>
      switch (finding.tier) {
        FindingTier.checksum => l10n.interceptBodyTierChecksum,
        FindingTier.regex => l10n.interceptBodyTierRegex,
        FindingTier.userTerm => l10n.interceptBodyTierUserTerm,
      };
}

/// Welche Ansichten dieser Rumpf wirklich zeigen kann.
///
/// [bodyViewsFor] sagt, was die Art anbietet; hier steht, was das Modell
/// hergibt. Ein als JSON angekündigter Rumpf, der keines ist, bekommt keinen
/// Baum -- und der Umschalter zeigt dann auch keinen, statt auf eine leere
/// Fläche zu schalten.
List<BodyPane> availableBodyPanes(ParsedBody parsed, BodyKind kind) =>
    <BodyPane>[
      for (final BodyPane pane in bodyViewsFor(kind))
        if (switch (pane) {
          BodyPane.tree => parsed.json != null,
          BodyPane.form => parsed.form?.isNotEmpty ?? false,
          BodyPane.raw => parsed.text != null,
          BodyPane.hex => true,
        })
          pane,
    ];

/// Die Beschriftung eines Segments.
String bodyPaneLabel(BodyPane pane, AppLocalizations l10n) => switch (pane) {
  BodyPane.tree => l10n.interceptBodyPaneTree,
  BodyPane.form => l10n.interceptBodyPaneForm,
  BodyPane.raw => l10n.interceptBodyPaneRaw,
  BodyPane.hex => l10n.interceptBodyPaneHex,
};

/// Was über der Ansicht steht: alles, was fehlt oder nicht stimmt.
///
/// Eine reine Funktion, damit prüfbar bleibt, dass jeder dieser Fälle einen
/// Satz bekommt und keiner still bleibt.
List<String> bodyNotes(
  ParsedBody parsed,
  BodyPane pane,
  int byteCount,
  AppLocalizations l10n,
) {
  final List<String> notes = <String>[];
  if (parsed.disputedType) {
    notes.add(l10n.interceptBodyTypeDisputed);
  }
  switch (parsed.problem) {
    case BodyProblem.tooLarge:
      notes.add(l10n.interceptBodyTooLarge(parsed.findings.length));
    case BodyProblem.incomplete:
      notes.add(l10n.interceptBodyIncomplete);
    case BodyProblem.truncatedStream:
      notes.add(l10n.interceptBodyStreamTruncated);
    case BodyProblem.undecodedEncoding:
      notes.add(l10n.interceptBodyEncodingUndecoded(parsed.encodingLabel));
    case BodyProblem.notJson:
      notes.add(l10n.interceptBodyNotJson);
    case BodyProblem.notForm:
      notes.add(l10n.interceptBodyNotForm);
    case null:
      break;
  }
  final JsonDocument? document = parsed.json;
  if (document != null && pane == BodyPane.tree) {
    if (document.duplicateKeys) {
      notes.add(l10n.interceptBodyDuplicateKeys);
    }
    if (document.capped || document.depthCapped) {
      notes.add(l10n.interceptBodyTreeCapped);
    }
  }
  final BodyText? text = parsed.text;
  if (text != null && text.rowsCapped && pane == BodyPane.raw) {
    notes.add(l10n.interceptBodyRowsCapped);
  }
  if (pane == BodyPane.hex && byteCount > bodyHexLimit) {
    notes.add(l10n.interceptBodyHexTruncated);
  }
  if (!parsed.findingsPlaced && parsed.findings.isNotEmpty) {
    notes.add(l10n.interceptBodyFindingsNotPlaced(parsed.findings.length));
    return notes;
  }
  final int missing = unmarkedFindings(parsed, pane, byteCount).length;
  if (missing > 0) {
    notes.add(l10n.interceptBodyFindingsElsewhere(missing));
  }
  return notes;
}

/// Die Bytes, aus denen die Hex-Ansicht liest.
///
/// Der Rumpf, wie er zerlegt wurde — bei einer gepackten Anfrage also der
/// ausgepackte. Nur dort liegen die Fundstellen des Daemons.
Uint8List parsedBodyBytes(ParsedBody parsed, RawBody? raw) =>
    parsed.bytes.isEmpty ? (raw?.bytes ?? Uint8List(0)) : parsed.bytes;

/// Welche Funde in [pane] keine Markierung bekommen.
///
/// Der Fund, der in einer Ansicht unsichtbar bleibt, ist gefährlicher als eine
/// fehlende Ansicht; deshalb wird er gezählt und genannt, statt zu
/// verschwinden.
Set<int> unmarkedFindings(ParsedBody parsed, BodyPane pane, int shownBytes) {
  final Set<int> all = <int>{
    for (final BodyFinding finding in parsed.findings) finding.index,
  };
  if (all.isEmpty || !parsed.findingsPlaced) {
    return all;
  }
  switch (pane) {
    case BodyPane.tree:
      return parsed.json == null ? all : parsed.json!.unlocatedFindings;
    case BodyPane.form:
      return all.difference(
        formLocatedFindings(parsed.form ?? const <FormPair>[], parsed.findings),
      );
    case BodyPane.raw:
      final BodyText? text = parsed.text;
      if (text == null || text.rows.isEmpty) {
        return all;
      }
      // Gegen das Ende der **Zeilen**, nicht gegen die Länge des Textes: die
      // Zeilen hören bei `bodyMaxRows` auf, der Text nicht, und ein Fund
      // dahinter bekommt keinen Unterstrich.
      final int lastChar = text.rows.last.charEnd;
      return all.difference(<int>{
        for (final BodyFinding finding in parsed.findings)
          if (finding.hasRange && finding.charStart < lastChar) finding.index,
      });
    case BodyPane.hex:
      final int shown = shownBytes < bodyHexLimit ? shownBytes : bodyHexLimit;
      return all.difference(<int>{
        for (final BodyFinding finding in parsed.findings)
          if (finding.byteEnd > finding.byteStart && finding.byteStart < shown)
            finding.index,
      });
  }
}
