// Der gebündelte Domain-Katalog (HUM-094): Er wird wirklich gelesen, er
// enthält wirklich die Einträge, die die Karte behauptet, und eine Kennung, die
// er nicht kennt, liefert null statt eines Wurfs.
//
// Der erste Test liest das echte Asset, nicht eine Attrappe: Ein Test über
// einen selbstgeschriebenen YAML-Schnipsel bewiese nur, dass der Parser
// funktioniert, nicht, dass `app/assets/catalog/domains.yaml` gebündelt ist und
// die Form hat, die `catalog/domains.yaml` heute trägt.

import 'dart:ui' show Locale;

import 'package:flutter/foundation.dart' show ByteData;
import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/providers/catalog.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';
import 'package:humanitl/features/intercept/widgets/group_header_row.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

void main() {
  // `rootBundle` braucht die Bindung, bevor zum ersten Mal geladen wird; sonst
  // scheitert das Parsen mit einer irreführenden Meldung.
  TestWidgetsFlutterBinding.ensureInitialized();

  test('catalog_parses_bundled_asset', () async {
    final ProviderContainer container = ProviderContainer();
    addTearDown(container.dispose);

    final Map<String, CatalogEntry> byId = await container.read(
      catalogProvider.future,
    );

    expect(
      byId.length,
      greaterThanOrEqualTo(25),
      reason: 'der gebündelte Katalog trägt die Einträge von HUM-031',
    );
    final CatalogEntry npm = byId['npm']!;
    expect(npm.name, 'npm registry');
    expect(npm.category, 'registry');
    expect(npm.typical.first, 'npm install');
    expect(npm.firstTypical, 'npm install');
    expect(
      npm.descriptionFor('en'),
      contains('Node.js'),
      reason: 'die Beschreibung kommt aus der Datei, nicht aus ARB',
    );
    expect(
      npm.descriptionFor('de'),
      isNot(npm.descriptionFor('en')),
      reason: 'die Datei führt beide Sprachen getrennt',
    );
  });

  test('catalog_asset_is_the_file_of_the_repository', () async {
    // Die Kopie ist erzeugt, nicht gepflegt: Wäre sie es nicht, nennte der
    // Bildschirm einen anderen Dienst, als der Daemon zugeordnet hat.
    // `make catalog-lint` hält beide Byte für Byte gegeneinander; hier steht
    // nur, dass die gebündelte Fassung überhaupt erreichbar ist.
    final String source = await rootBundle.loadString(catalogAsset);
    expect(source, startsWith('# Der gebündelte Domain-Katalog'));
  });

  test('catalog_description_falls_back_to_en', () {
    const CatalogEntry onlyEnglish = CatalogEntry(
      id: 'x',
      name: 'X',
      category: 'other',
      description: <String, String>{'en': 'Only English.'},
    );

    expect(onlyEnglish.descriptionFor('de'), 'Only English.');
    expect(onlyEnglish.descriptionFor('en'), 'Only English.');
    expect(
      const CatalogEntry(
        id: 'y',
        name: 'Y',
        category: 'other',
      ).descriptionFor('de'),
      '',
      reason: 'ohne jede Beschreibung wird keine erfunden',
    );
    expect(
      const CatalogEntry(
        id: 'z',
        name: 'Z',
        category: 'other',
        description: <String, String>{'en': 'English.', 'de': 'Deutsch.'},
      ).descriptionFor('de'),
      'Deutsch.',
      reason: 'wo es eine Übersetzung gibt, steht sie da',
    );
  });

  test('catalog_entry_unknown_id_is_null', () async {
    final ProviderContainer container = ProviderContainer();
    addTearDown(container.dispose);
    await container.read(catalogProvider.future);

    expect(container.read(catalogEntryProvider('nope')), isNull);
    expect(
      container.read(catalogEntryProvider('')),
      isNull,
      reason: 'eine leere Kennung heißt „der Daemon kennt den Dienst nicht"',
    );
    expect(container.read(catalogEntryProvider('npm')), isNotNull);
  });

  test('catalog_skips_an_entry_it_cannot_read', () {
    final Map<String, CatalogEntry> byId = parseCatalog('''
version: 1
entries:
  - id: good
    name: Good
    category: registry
    description:
      en: "A registry."
      de: "Eine Registry."
    typical: ["good install"]
  - name: nameless
    category: registry
''');

    expect(byId.keys, <String>['good'], reason: 'ein Eintrag ohne Kennung');
    expect(byId['good']!.typical, <String>['good install']);
  });

  test('catalog_of_a_file_without_entries_is_empty', () {
    expect(parseCatalog('version: 1\n'), isEmpty);
    expect(parseCatalog('not a map\n'), isEmpty);
  });

  test('catalog_of_malformed_yaml_is_empty_and_does_not_throw', () {
    // Eine Datei, die keine YAML ist: `loadYaml` wirft dafuer eine
    // `YamlException`. Der Katalog ist Beiwerk zur Entscheidung; eine kaputte
    // Kopie nimmt der Warteschlange ihre Namen, nicht ihre Arbeit.
    expect(
      parseCatalog('entries:\n  - id: broken\n   name: "unclosed'),
      isEmpty,
    );
    expect(parseCatalog('{[}'), isEmpty);
    expect(parseCatalog('a: b\n  c: d\n'), isEmpty);
  });

  test(
    'catalog_without_the_asset_is_empty_and_the_head_names_the_host',
    () async {
      // Kein Buendel, keine Antwort: `rootBundle.loadString` wirft dann. Der
      // Provider gibt trotzdem eine leere Abbildung zurueck, und leer heisst
      // ueberall dasselbe wie eine unbekannte Kennung.
      rootBundle.clear();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMessageHandler('flutter/assets', (ByteData? _) async => null);
      addTearDown(() {
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMessageHandler('flutter/assets', null);
        rootBundle.clear();
      });

      final ProviderContainer container = ProviderContainer();
      addTearDown(container.dispose);

      final Map<String, CatalogEntry> byId = await container.read(
        catalogProvider.future,
      );
      expect(byId, isEmpty, reason: 'kein Wurf, eine leere Abbildung');
      expect(container.read(catalogProvider).hasError, isFalse);

      // Und die ehrliche Folge davon: Es gibt keinen Eintrag, also nennt der
      // Kopf einer Gruppe den Host und die rechte Spalte zeigt die
      // Unbekannt-Karte.
      expect(container.read(catalogEntryProvider('npm')), isNull);
      final AppLocalizations l10n = await AppLocalizations.delegate.load(
        const Locale('en'),
      );
      final HeldGroup group = groupFlows(<Flow>[
        heldFlow(
          n: 1,
          deadline: testStart.add(const Duration(minutes: 5)),
          host: 'registry.npmjs.org',
          apex: 'npmjs.org',
        ).copyWith(catalogId: 'npm'),
        heldFlow(
          n: 2,
          deadline: testStart.add(const Duration(minutes: 6)),
          host: 'registry.npmjs.org',
          apex: 'npmjs.org',
        ).copyWith(catalogId: 'npm'),
      ]).groups.single;
      expect(groupTitle(group, null, l10n), 'registry.npmjs.org');
    },
  );
}
