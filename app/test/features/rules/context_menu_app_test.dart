// Das Kontextmenü eines Eingabefelds im Aufbau der echten Anwendung (HUM-173):
// `HumanitlApp` mit `WidgetsApp`, `HTheme` im `builder` und dem Overlay des
// Navigators darunter. Das Menü hängt in dieses Overlay, also nicht unter das
// Feld; es muss Theme und Tastenkürzel-Anzeige selbst mitbringen.
//
// Unter `TargetPlatformVariant.only(linux)`: Das Desktop-Menü entsteht nur
// dort.

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';

import '../../harness/app_harness.dart';
import '../../harness/fixed_now.dart';

void main() {
  testWidgets('context_menu_opens_on_a_field_of_the_real_app', (
    WidgetTester tester,
  ) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient(),
      overrides: <Override>[
        nowProvider.overrideWith(() => FixedNow(DateTime.now())),
      ],
    );
    ProviderScope.containerOf(tester.element(find.byType(HumanitlApp)))
        .read(navigationProvider.notifier)
        .go(Section.rules);
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    await tester.tap(find.byKey(const Key('rules-new')));
    await tester.pump();
    await tester.tap(
      find.byKey(const Key('rule-host')),
      buttons: kSecondaryButton,
    );
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.text('Copy'), findsOneWidget);
    expect(find.text('Select All'), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
