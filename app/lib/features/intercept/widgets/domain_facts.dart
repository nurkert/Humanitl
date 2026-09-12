/// Die Tatsachen zum Ziel, die auf beiden Karten des rechten Panes stehen
/// (HUM-031, HUM-094): Host, registrierbare Domäne, Verbreitungsrang und der
/// Zähler dieser Sitzung.
///
/// Ein Block für beide Karten, weil sie dasselbe behaupten müssen. Zwei
/// Fassungen desselben Rangs oder zweierlei „unbekannt" wären zwei Antworten
/// auf eine Frage (`backlog/CONVENTIONS.md` 4.13).
library;

// `Flow` ist hier ein Domänentyp, nicht das Layout-Widget gleichen Namens.
import 'package:flutter/widgets.dart' hide Flow;

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// Host, Domäne, Rang und Zähler zum ausgewählten Fluss.
class DomainFacts extends StatelessWidget {
  /// Erzeugt den Block zu [flow].
  const DomainFacts({required this.flow, required this.domain, super.key});

  /// Die ausgewählte Anfrage.
  final Flow flow;

  /// Was der Daemon zum Ziel weiß; null, solange das Detail lädt.
  final DomainInfo? domain;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DomainInfo? domain = this.domain;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          flow.host,
          key: const Key('intercept-domain-host'),
          style: tokens.typography.mono12.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(
          l10n.interceptDomainApex,
          style: tokens.typography.ui11.tinted(tokens.colors.fg2),
        ),
        Text(
          apexLine(flow, l10n),
          key: const Key('intercept-domain-apex'),
          style: tokens.typography.mono12.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x2),
        Row(
          children: <Widget>[
            HBadge(
              key: const Key('intercept-domain-rank'),
              text: rankLabel(domain?.trancoRank ?? 0, l10n),
              mono: true,
            ),
            SizedBox(width: tokens.spacing.x2),
            Expanded(
              child: Text(
                domain == null
                    ? l10n.interceptDomainFirstSeen
                    : l10n.domainSeenCount(domain.seenCount),
                key: const Key('intercept-domain-seen'),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tokens.typography.ui12.tinted(tokens.colors.fg2),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

/// Was in der Zeile „registrierbare Domäne" steht.
///
/// Für eine Adresse steht dort „IP address": eine Adresse hat keine
/// registrierbare Domäne, und es wird ihr keine erfunden. Für einen Namen, zu
/// dem der Daemon keine kennt, steht „not known yet" — nie der Host und nie ein
/// Rat (`backlog/CONVENTIONS.md` 4.13, HUM-091).
String apexLine(Flow flow, AppLocalizations l10n) {
  if (flow.authority.isIpLiteral) {
    return l10n.domainIpAddress;
  }
  return flow.apex.isEmpty ? l10n.interceptDomainApexUnknown : flow.apex;
}

/// Der Verbreitungsrang als Aufschrift: unter 1000 genau, darüber `1.2k`.
///
/// 0 heißt „kein Rang" und wird „unranked", nie `#0`. Ein Rang sagt, wie
/// verbreitet eine Domäne ist, nie, wie vertrauenswürdig sie ist (HUM-031).
String rankLabel(int rank, AppLocalizations l10n) {
  if (rank <= 0) {
    return l10n.domainRankUnranked;
  }
  if (rank < 1000) {
    return l10n.domainRank('$rank');
  }
  final double thousands = rank / 1000;
  return l10n.domainRank('${thousands.toStringAsFixed(1)}k');
}
