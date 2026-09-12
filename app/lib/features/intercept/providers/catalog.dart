/// Der gebündelte Domain-Katalog als Provider (HUM-094).
///
/// Der Daemon schickt je Fluss nur eine Kennung (`FlowSummary.catalog_id`).
/// Was hinter ihr steht — Name, Kategorie, Beschreibung, das Typische —, liest
/// die Oberfläche aus dem Asset `assets/catalog/domains.yaml`, einer erzeugten
/// Kopie von `catalog/domains.yaml`, die `make catalog-lint` Byte für Byte
/// gegen das Original hält. Kein Netz und kein zweiter Aufruf: der Katalog ist
/// gebündelt (ADR-006).
///
/// Die Zuordnung Host zu Kennung steht ausdrücklich **nicht** hier. Sie ist
/// eine Behauptung über ein Ziel, und die trifft der Daemon mit der
/// Glob-Semantik der Regeln; eine zweite Zuordnung in der Oberfläche wäre eine
/// zweite Antwort auf dieselbe Frage (HUM-091 hat das für die Public Suffix
/// List schon einmal gekostet).
library;

import 'package:flutter/services.dart' show rootBundle;
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:yaml/yaml.dart';

import '../../../core/domain/domain.dart';

part 'catalog.g.dart';

/// Der Pfad des gebündelten Katalogs.
const String catalogAsset = 'assets/catalog/domains.yaml';

/// Der Katalog als Abbildung Kennung zu Eintrag.
///
/// Einmal je Sitzung gelesen: Die Datei ist gebündelt, ändert sich also zur
/// Laufzeit nicht, und ein Neulesen je Auswahl läge auf dem Weg zwischen
/// Ankunft und Entscheidung.
///
/// Ein Eintrag, dessen Form die Datei verletzt, fehlt in der Abbildung, statt
/// das Lesen abzubrechen: Der Katalog ist Beiwerk zur Entscheidung, und eine
/// leere rechte Spalte ist besser als eine Warteschlange, die nicht lädt. Die
/// Form selbst hält `catalog/domains.schema.json` in CI.
///
/// Aus demselben Grund wirft dieser Provider nicht. Fehlt das Asset im Bündel
/// oder ist die Datei keine lesbare YAML, ist die Abbildung leer, und leer
/// heißt überall dasselbe wie eine unbekannte Kennung: Der Kopf einer Gruppe
/// nennt dann den Host, die rechte Spalte zeigt die Unbekannt-Karte. Ein
/// geworfener Fehler machte daraus eine Warteschlange mit einem roten Pane,
/// und ein fehlender Katalogeintrag ist kein Grund, eine Entscheidung
/// aufzuhalten.
@Riverpod(keepAlive: true)
Future<Map<String, CatalogEntry>> catalog(Ref ref) async {
  final String source;
  try {
    source = await rootBundle.loadString(catalogAsset);
  } on Object {
    // `FlutterError` für ein fehlendes Asset, `PlatformException` für ein
    // Bündel, das gar nicht antwortet: Beide enden hier gleich.
    return const <String, CatalogEntry>{};
  }
  return parseCatalog(source);
}

/// Liest den Katalog aus dem Wortlaut der YAML-Datei.
///
/// Eigene Funktion, damit ein Test sie ohne Asset-Bündel aufrufen kann; der
/// Provider reicht nur den Inhalt der Datei herein.
Map<String, CatalogEntry> parseCatalog(String source) {
  final Object? document;
  try {
    document = loadYaml(source);
  } on Object {
    // `YamlException` für eine Datei, die keine YAML ist. Eine kaputte Kopie
    // nimmt der Warteschlange ihre Namen, nicht ihre Arbeit.
    return const <String, CatalogEntry>{};
  }
  if (document is! YamlMap) {
    return const <String, CatalogEntry>{};
  }
  final Object? entries = document['entries'];
  if (entries is! YamlList) {
    return const <String, CatalogEntry>{};
  }
  final Map<String, CatalogEntry> byId = <String, CatalogEntry>{};
  for (final Object? node in entries) {
    if (node is! YamlMap) {
      continue;
    }
    final Object? id = node['id'];
    final Object? name = node['name'];
    final Object? category = node['category'];
    if (id is! String || name is! String || category is! String) {
      continue;
    }
    byId[id] = CatalogEntry(
      id: id,
      name: name,
      category: category,
      description: _text(node['description']),
      typical: _strings(node['typical']),
      icon: node['icon'] is String ? node['icon'] as String : '',
      homepage: node['homepage'] is String ? node['homepage'] as String : '',
      riskNote: _text(node['risk_note']),
    );
  }
  return Map<String, CatalogEntry>.unmodifiable(byId);
}

/// Der Eintrag zu [catalogId], oder null.
///
/// Null heißt dreierlei und in jedem Fall dasselbe für die Karte: Die Kennung
/// ist leer (der Daemon kennt den Dienst nicht), sie steht nicht im Katalog
/// dieser Fassung, oder das Asset lädt noch. Gezeichnet wird dann die
/// Unbekannt-Karte, nie eine geratene.
@riverpod
CatalogEntry? catalogEntry(Ref ref, String catalogId) {
  if (catalogId.isEmpty) {
    return null;
  }
  return ref
      .watch(catalogProvider)
      .maybeWhen(
        data: (Map<String, CatalogEntry> byId) => byId[catalogId],
        orElse: () => null,
      );
}

/// Eine Abbildung Sprachkennung zu Text, wie `description` und `risk_note` sie
/// führen. Alles andere ergibt eine leere Abbildung.
Map<String, String> _text(Object? node) {
  if (node is! YamlMap) {
    return const <String, String>{};
  }
  final Map<String, String> text = <String, String>{};
  for (final MapEntry<Object?, Object?> entry in node.entries) {
    final Object? key = entry.key;
    final Object? value = entry.value;
    if (key is String && value is String) {
      text[key] = value;
    }
  }
  return Map<String, String>.unmodifiable(text);
}

/// Eine Liste von Zeichenketten; alles andere ergibt eine leere Liste.
List<String> _strings(Object? node) {
  if (node is! YamlList) {
    return const <String>[];
  }
  return List<String>.unmodifiable(<String>[
    for (final Object? item in node)
      if (item is String) item,
  ]);
}
