/// Die Katalog-Karte des rechten Panes (HUM-031, HUM-094).
///
/// Sie ist der „De-Panicker": Ein erkannter Dienst und null Funde heißt, dass
/// ein Stapel ohne schlechtes Gefühl freigegeben werden kann. Genau deshalb
/// sagt sie nur, was der Katalog behauptet — Name, Einordnung, Beschreibung,
/// wobei ein Agent hier üblicherweise landet — und nie, dass diese Anfrage in
/// Ordnung sei: derselbe Host trägt ein `git clone` und einen Datenabfluss
/// (`BACKLOG.md` 4.3).
library;

// `Flow` ist hier ein Domänentyp, nicht das Layout-Widget gleichen Namens.
import 'package:flutter/widgets.dart' hide Flow;

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'domain_facts.dart';

/// Was der Katalog über den ausgewählten Fluss sagt.
class CatalogCard extends StatelessWidget {
  /// Erzeugt die Karte zu [entry] für [flow].
  const CatalogCard({
    required this.entry,
    required this.flow,
    required this.domain,
    super.key,
  });

  /// Der Eintrag, den der Daemon über `catalog_id` benannt hat.
  final CatalogEntry entry;

  /// Die ausgewählte Anfrage.
  final Flow flow;

  /// Was der Daemon zum Ziel weiß: Rang und Zähler. Null, solange das Detail
  /// lädt; die Karte lässt die Zeilen dann weg, statt eine Null zu zeigen.
  final DomainInfo? domain;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String language = Localizations.localeOf(context).languageCode;
    final String description = entry.descriptionFor(language);
    final String riskNote = entry.riskNoteFor(language);
    final String typical = entry.firstTypical;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // Kein Zeichen des Dienstes: `catalog/icons/` liegt im Repository,
        // aber die Anwendung bringt keinen SVG-Renderer mit, und ein geholtes
        // Favicon wäre ein Abruf, den niemand erlaubt hat (ADR-006). Der Name
        // steht deshalb allein, statt neben einem Platzhalter, der ein Bild
        // verspricht (HUM-031, offener Punkt im Audit zu HUM-094).
        Text(
          entry.name,
          key: const Key('intercept-domain-name'),
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: tokens.typography.ui14.semibold.tinted(tokens.colors.fg0),
        ),
        SizedBox(height: tokens.spacing.x2),
        Align(
          alignment: Alignment.centerLeft,
          child: HBadge(
            key: const Key('intercept-domain-category'),
            text: categoryLabel(entry.category, l10n),
          ),
        ),
        if (description.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Text(
            description,
            key: const Key('intercept-domain-description'),
            style: tokens.typography.ui13.tinted(tokens.colors.fg1),
          ),
        ],
        if (typical.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Text(
            l10n.domainTypicalFor(typical),
            key: const Key('intercept-domain-typical'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ],
        SizedBox(height: tokens.spacing.x2),
        DomainFacts(flow: flow, domain: domain),
        if (riskNote.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Text(
            riskNote,
            key: const Key('intercept-domain-risk'),
            // Die einzige Chroma dieser Karte. Der Hinweis steht am Eintrag,
            // nicht an der Anfrage: er sagt, was an diesem Dienst möglich ist,
            // nicht, dass es gerade geschieht.
            style: tokens.typography.ui12.tinted(tokens.state.error),
          ),
        ],
      ],
    );
  }
}

/// Das Schild der Kategorie, in der Sprache des Nutzers.
///
/// Eine unbekannte Kategorie wird als „other" gezeigt, nicht weggelassen: Der
/// Katalog behauptet eine, und eine Fassung der Anwendung, die sie nicht kennt,
/// verschweigt sie nicht.
String categoryLabel(String category, AppLocalizations l10n) =>
    switch (category) {
      'registry' => l10n.catalogCategoryRegistry,
      'scm' => l10n.catalogCategoryScm,
      'docs' => l10n.catalogCategoryDocs,
      'ci' => l10n.catalogCategoryCi,
      'cloud' => l10n.catalogCategoryCloud,
      'ai' => l10n.catalogCategoryAi,
      'cdn' => l10n.catalogCategoryCdn,
      'search' => l10n.catalogCategorySearch,
      'os' => l10n.catalogCategoryOs,
      _ => l10n.catalogCategoryOther,
    };
