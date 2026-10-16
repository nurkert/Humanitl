// Kein Abschnitt der App läuft im Deutschen über (HUM-054, Fallstrick
// `no_overflow_in_de`).
//
// Die Goldens rendern Text als Blöcke; ein deutscher Satz, der länger ist als
// sein englischer, zeigt sich dort nur als breiterer Block, und ein Überlauf
// fiele erst auf, wenn jemand die PNG genau ansieht. Flutter meldet einen
// Überlauf im Test dagegen als Ausnahme. Dieser Test geht deshalb im
// Standardfenster von 1280×800 mit deutscher Oberfläche durch jeden Abschnitt
// und das Einstellungsblatt und verlangt nach jedem Schritt, dass keine
// Ausnahme ansteht.
//
// Die Schrift des Tests ist breiter als jede echte (jedes Zeichen ein Geviert);
// was hier passt, passt mit der Schrift der App erst recht.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/shell/providers/language.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../intercept/fixtures.dart';

/// Deutsch, unabhängig von Konfiguration und Rechner.
class German extends LanguageSetting {
  @override
  AppLanguage? build() => AppLanguage.de;
}

/// Ein Daemon mit dem Standard-Szenario, einer Maschine mit einem roten
/// Punkt, einem Arbeitsordner und einem Schlüssel: die längsten Zeilen, die
/// die Abschnitte zeigen können.
FakeDaemonClient richDaemon() => FakeDaemonClient(clock: () => testStart)
  ..doctorReport = fakeDoctorFailing()
  ..configValues = const ConfigValues(
    values: <String, Object?>{
      'hold': <String, Object?>{'timeout_secs': 300, 'ask_mode': 'ui'},
      'ui': <String, Object?>{
        'language': 'de',
        'theme': 'dark',
        'notifications': true,
      },
      'sandbox': <String, Object?>{'work_dir': '/home/u/src/acme'},
      'llm': <String, Object?>{
        'endpoint': 'http://192.168.1.20:11434/',
        'models': <Object?>['qwen2.5-coder:14b'],
      },
    },
    configPath: '/home/u/.config/humanitl/config.toml',
  )
  ..llmKey = const LlmKeyStatus(
    set: true,
    store: LlmKeyStore.secretService,
    keyRef: 'secret-service:humanitl/llm',
  );

void main() {
  testWidgets('no_overflow_in_de', (WidgetTester tester) async {
    await pumpApp(
      tester,
      client: richDaemon(),
      overrides: <Override>[languageProvider.overrideWith(German.new)],
    );
    // Das Szenario spielt seine Anfragen in den ersten Sekunden ein.
    await tester.pump(const Duration(seconds: 3));
    await tester.pump();
    expect(tester.takeException(), isNull, reason: 'first screen');

    for (final Section section in Section.values) {
      await pressCtrl(tester, navigationKeys[section.index]);
      await tester.pump(const Duration(milliseconds: 600));
      await tester.pump();
      expect(tester.takeException(), isNull, reason: section.name);
      if (section == Section.intercept) {
        // Deutsch, und eine Warteschlange, in der etwas steht: Ohne beides
        // prüfte der Test eine englische oder leere Oberfläche.
        expect(
          tester
              .widget<Text>(find.byKey(const Key('header-section-title')))
              .data,
          lookupAppLocalizations(const Locale('de')).shellNavIntercept,
        );
        expect(find.byType(QueueRow), findsWidgets);
      }
    }

    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pump();
    expect(find.byKey(const Key('settings-sheet')), findsOne);
    expect(tester.takeException(), isNull, reason: 'settings sheet');

    // Die Uhr des Szenarios hinterlässt Fristen; sie laufen hier ab, statt
    // als offene Timer am Ende des Tests zu stehen.
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(minutes: 10));
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
