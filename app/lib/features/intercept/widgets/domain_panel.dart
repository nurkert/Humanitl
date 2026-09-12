/// Das rechte Pane: was über das Ziel bekannt ist (HUM-031, HUM-094).
///
/// Drei Zustände, und keiner davon rät: die Katalog-Karte für ein Ziel, das der
/// Daemon einem Eintrag zugeordnet hat; die Unbekannt-Karte für eines, das er
/// keinem zuordnen konnte; die Zusammenfassung der Sitzung, solange nichts
/// ausgewählt ist. Darunter die Schnellregeln, die eine Regel anlegen und nie
/// entscheiden.
library;

import 'dart:async';

// `Flow` ist hier ein Domänentyp, nicht das Layout-Widget gleichen Namens.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/time/now.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/catalog.dart';
import '../providers/flows.dart';
import '../rule_sentence.dart';
import 'catalog_card.dart';
import 'unknown_domain_card.dart';

/// Wie viele Hosts die Zusammenfassung der Sitzung aufzählt.
const int sessionTopHosts = 5;

/// Das Domain-Pane.
class DomainPanel extends ConsumerWidget {
  /// Erzeugt das Pane für [flow]; ohne Auswahl steht die Zusammenfassung.
  const DomainPanel({required this.flow, super.key});

  /// Die ausgewählte Anfrage, oder null.
  final Flow? flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Flow? flow = this.flow;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(left: BorderSide(color: tokens.colors.line)),
      ),
      // Das Pane scrollt: Bei doppelter Textskalierung ist sein Inhalt höher
      // als jedes Fenster, und eine Spalte, die überläuft, verschluckt den Rest
      // schweigend (`docs/UX.md` 6).
      child: SingleChildScrollView(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              l10n.interceptDomainTitle,
              style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
            ),
            SizedBox(height: tokens.spacing.x3),
            if (flow == null)
              const _SessionSummary()
            else ...<Widget>[
              _CardFor(flow: flow),
              SizedBox(height: tokens.spacing.x4),
              _QuickRules(flow: flow),
            ],
          ],
        ),
      ),
    );
  }
}

/// Die Karte zum ausgewählten Fluss: bekannt oder unbekannt.
class _CardFor extends ConsumerWidget {
  const _CardFor({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Der Dienst kommt aus der Zeile, die der Daemon geschickt hat
    // (`FlowSummary.catalog_id`), der Eintrag dazu aus dem gebündelten
    // Katalog. Rang und Zähler stehen nur im Detail; solange das lädt, lässt
    // die Karte sie weg, statt eine Null zu zeigen (HUM-094).
    final CatalogEntry? entry = ref.watch(catalogEntryProvider(flow.catalogId));
    final DomainInfo? domain = ref
        .watch(flowDetailProvider(flow.id))
        .value
        ?.domain;
    return entry == null
        ? UnknownDomainCard(flow: flow, domain: domain)
        : CatalogCard(entry: entry, flow: flow, domain: domain);
  }
}

/// Die Zusammenfassung der Sitzung, solange nichts ausgewählt ist.
class _SessionSummary extends ConsumerWidget {
  const _SessionSummary();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<MapEntry<String, int>> top = topHosts(
      ref.watch(flowsProvider).values,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          l10n.domainSessionHosts(top.length),
          key: const Key('intercept-domain-session-hosts'),
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
        if (top.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x3),
          Text(
            l10n.domainSessionTop,
            style: tokens.typography.ui11.tinted(tokens.colors.fg2),
          ),
          SizedBox(height: tokens.spacing.x1),
          for (final MapEntry<String, int> host in top.take(
            sessionTopHosts,
          )) ...<Widget>[
            Padding(
              padding: EdgeInsets.only(bottom: tokens.spacing.x1),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: Text(
                      host.key,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  HBadge(
                    text: '${host.value}',
                    mono: true,
                    semanticsLabel: l10n.domainSeenCount(host.value),
                  ),
                ],
              ),
            ),
          ],
        ],
      ],
    );
  }
}

/// Die Hosts dieser Sitzung, die mit den meisten Anfragen zuerst.
///
/// Gezählt wird jede Anfrage, nicht jede gehaltene: Die Zusammenfassung sagt,
/// was diese Sitzung getan hat, nicht, worüber gerade entschieden wird. Bei
/// gleicher Zahl entscheidet der Name, damit die Liste zwischen zwei Bildern
/// nicht springt.
List<MapEntry<String, int>> topHosts(Iterable<Flow> flows) {
  final Map<String, int> counts = <String, int>{};
  for (final Flow flow in flows) {
    counts[flow.host] = (counts[flow.host] ?? 0) + 1;
  }
  return counts.entries.toList()
    ..sort((MapEntry<String, int> a, MapEntry<String, int> b) {
      final int byCount = b.value.compareTo(a.value);
      return byCount != 0 ? byCount : a.key.compareTo(b.key);
    });
}

/// Die drei Schnellregeln unter der Karte.
///
/// Sie legen eine Regel an (`Rules(add)`) und entscheiden nie: Über die Anfrage
/// auf dem Tisch entscheidet die Aktionsleiste, und ein Knopf am Rand, der
/// zwölf Anfragen durchließe, wäre genau die Abkürzung, die
/// `backlog/CONVENTIONS.md` 4.13 ausschließt.
///
/// Die Aufschrift nennt das Muster, das die Regel treffen wird; der ganze Satz
/// der Regel steht im Hover, aus demselben Generator, den der Regel-Bildschirm
/// liest. Zwei Wortlaute für dieselbe Regel waren einmal der Fehler, vor dem
/// 4.13 warnt.
class _QuickRules extends ConsumerWidget {
  const _QuickRules({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DateTime clock = ref.watch(nowProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final RuleDraft draft in quickRuleDrafts(flow))
          // Ohne Muster entsteht keine Regel: Kennt der Daemon die
          // registrierbare Domäne nicht, fehlt der Knopf, statt ausgegraut
          // etwas zu versprechen, das nie entstünde (HUM-091).
          if (hostPattern(draft, _apexOf).isNotEmpty)
            Padding(
              padding: EdgeInsets.only(bottom: tokens.spacing.x2),
              child: HoverLabel(
                label: ruleSentence(draft, l10n, apexOf: _apexOf, now: clock),
                child: HButton(
                  key: Key(
                    'intercept-quick-rule-'
                    '${draft.action.name}-${draft.target.name}',
                  ),
                  variant: HButtonVariant.ghost,
                  onPressed: () => unawaited(_add(ref, draft, clock)),
                  child: Text(
                    quickRuleLabel(draft, l10n, _apexOf),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
            ),
      ],
    );
  }

  /// Legt die Regel an, über `Rules(add)` und nie über `Decide`.
  ///
  /// Der Aufruf geht geradewegs an den Klienten und nicht über den Notifier des
  /// Regel-Bildschirms: Ein Feature darf kein anderes importieren
  /// (`docs/ARCHITECTURE.md` 5, geprüft von `tools/check-deps.sh`). Der
  /// Regel-Bildschirm fragt ohnehin neu, sobald er sichtbar wird.
  ///
  /// Lehnt der Daemon ab, bleibt es bei seiner Ablehnung: Die Warteschlange
  /// erfindet keine Regel und tut auch nicht so, als sei eine entstanden.
  Future<void> _add(WidgetRef ref, RuleDraft draft, DateTime clock) async {
    final Rule? rule = buildRule(draft, now: clock, apexOf: _apexOf);
    if (rule == null) {
      return;
    }
    try {
      await ref.read(daemonClientProvider).addRule(rule);
    } on DaemonException {
      // Der Streifen über der Warteschlange trägt, was der Daemon meldet; ein
      // zweiter Weg dafür wäre eine zweite Meldung zu einem Vorgang.
    }
  }

  /// Die registrierbare Domäne des ausgewählten Flusses, sonst leer.
  ///
  /// Beantwortet wird nur der eigene Host: Der Wert kommt aus der Zeile, die
  /// der Daemon geschickt hat (`FlowSummary.apex`, HUM-091), und für einen
  /// fremden Host hat dieses Pane keine Antwort. Leer heißt „unbekannt", und
  /// die Regel entsteht dann nicht.
  String _apexOf(String host) => host == flow.host ? flow.apex : '';
}

/// Die Aufschrift eines Schnellregel-Knopfes: was die Regel treffen wird.
///
/// Das Muster kommt aus [hostPattern], also aus derselben Funktion, aus der die
/// Regel es nimmt; die Aufschrift kann deshalb nichts anderes versprechen als
/// die Regel hält.
String quickRuleLabel(
  RuleDraft draft,
  AppLocalizations l10n,
  String Function(String host) apexOf,
) {
  final String pattern = hostPattern(draft, apexOf);
  return draft.action == RuleAction.block
      ? l10n.domainQuickBlock(pattern)
      : l10n.domainQuickAllow(pattern);
}

/// Die drei Entwürfe, die das Pane anbietet.
///
/// Die Domänen-Regel steht oben, weil sie die weiteste ist und der Katalog
/// gerade den Dienst benannt hat. Der Block steht unten und gilt für den Host,
/// nicht für die Domäne: Blocken ist die engere Tat, und eine Domänen-Sperre
/// aus einem Knopf am Rand wäre weiter, als der Blick auf eine Anfrage trägt.
List<RuleDraft> quickRuleDrafts(Flow flow) => <RuleDraft>[
  RuleDraft(
    duration: RememberDuration.session,
    target: RememberTarget.apex,
    flow: flow,
  ),
  RuleDraft(
    duration: RememberDuration.session,
    target: RememberTarget.host,
    flow: flow,
  ),
  RuleDraft(
    duration: RememberDuration.session,
    target: RememberTarget.host,
    flow: flow,
    action: RuleAction.block,
  ),
];
