/// Der gebündelte Domain-Katalog, wie die Oberfläche ihn liest (HUM-031,
/// HUM-094).
///
/// Über die Leitung kommt nur die Kennung (`FlowSummary.catalog_id`,
/// `DomainInfo.catalog_id`). Name, Beschreibung und das Typische stehen in
/// `catalog/domains.yaml`, das als Asset mitreist: ein Name auf dem Bildschirm
/// ist damit dieselbe Behauptung wie die im Repository, und niemand rät sie aus
/// einem Hostnamen. Wer die Zuordnung Host zu Kennung sucht, findet sie im
/// Daemon; hier steht nur, was zu einer schon zugeordneten Kennung gehört.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

part 'catalog.freezed.dart';

/// Ein Eintrag des Katalogs: was hinter einem Dienst steckt.
///
/// Die Felder spiegeln `catalog/domains.yaml` und damit
/// `humanitl_catalog::CatalogEntry`. `description` und [riskNote] sind
/// Abbildungen von Sprachkennung auf Text (`en` ist die Quelle, `de` die
/// Übersetzung), keine einzelnen Strings: die Datei führt beide Sprachen, und
/// eine Karte, die nur `en` läse, zeigte im deutschen Bild englische Sätze.
///
/// Ein Eintrag sagt, was ein Dienst ist. Er sagt nie, dass eine Anfrage dorthin
/// in Ordnung ist: derselbe Host trägt ein `git clone` und einen Datenabfluss
/// (`BACKLOG.md` 4.3).
@freezed
abstract class CatalogEntry with _$CatalogEntry {
  /// Erzeugt einen Eintrag.
  const factory CatalogEntry({
    required String id,
    required String name,

    /// `registry`, `scm`, `docs`, `ci`, `cloud`, `ai`, `cdn`, `search`, `os`
    /// oder `other`. Eine Einordnung, keine Bewertung.
    required String category,
    @Default(<String, String>{}) Map<String, String> description,
    @Default(<String>[]) List<String> typical,
    @Default('') String icon,
    @Default('') String homepage,
    @Default(<String, String>{}) Map<String, String> riskNote,
  }) = _CatalogEntry;

  const CatalogEntry._();

  /// Die Beschreibung in [languageCode], sonst die englische.
  ///
  /// Verglichen wird nur die Sprachkennung, nicht das ganze Gebietsschema:
  /// `de_AT` fragt hier als `de` an. Fehlt auch `en`, bleibt der leere String
  /// — eine Karte ohne Beschreibung, nie eine erfundene.
  String descriptionFor(String languageCode) =>
      description[languageCode] ?? description['en'] ?? '';

  /// Der Risikohinweis in [languageCode], sonst der englische; leer, wo der
  /// Eintrag keinen führt.
  String riskNoteFor(String languageCode) =>
      riskNote[languageCode] ?? riskNote['en'] ?? '';

  /// Der erste Eintrag von `typical`, sonst der leere String.
  ///
  /// Die Karte zeigt genau einen: „Looks like: npm install" ist eine Zeile, und
  /// eine Liste an dieser Stelle läse sich wie eine Aufzählung dessen, was
  /// gerade passiert — es ist aber nur, wobei ein Agent hier üblicherweise
  /// landet.
  String get firstTypical => typical.isEmpty ? '' : typical.first;
}
