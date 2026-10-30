// Der Start der Einrichtung, während Änderungen eines Wegwerf-Laufs warten
// (HUM-351): Gesperrt ist nur ein Wegwerf-Start, denn nur den lehnt der
// Daemon ab (`SANDBOX_030`); ein direkter Start geht mit einem Hinweis, und
// beide Fälle führen zum Panel.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/sandbox/providers/disposable_review.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/shell_screen.dart';

import '../../harness/app_harness.dart';

ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(ShellScreen)));

bool startEnabled(WidgetTester tester) =>
    tester.widget<HButton>(find.byKey(const Key('setup-start'))).enabled;

void main() {
  testWidgets('waiting changes block only a disposable start, and the setup '
      'leads to the panel', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.disposable()
      ..doctorReport = fakeDoctorOk();
    await pumpApp(tester, client: client);
    containerOf(tester).read(navigationProvider.notifier).go(Section.setup);
    await tester.pumpAndSettle();

    // Direkt: Der Start geht, und ein Hinweis sagt, dass Änderungen warten.
    expect(startEnabled(tester), isTrue);
    expect(find.byKey(const Key('setup-changes-hint')), findsOneWidget);
    expect(find.text('Review changes (7)'), findsOneWidget);

    // Wegwerf: gesperrt, mit dem Grund unter dem Knopf.
    await tester.tap(find.byKey(const Key('setup-disposable')));
    await tester.pumpAndSettle();
    expect(startEnabled(tester), isFalse);
    expect(
      find.text(
        'The last disposable run waits for a decision on its changes '
        '(SANDBOX_030).',
      ),
      findsOneWidget,
    );

    // Das Panel ging nach dem ersten Lesen von selbst auf; geschlossen, damit
    // der Knopf zeigen muss, dass er es öffnet.
    containerOf(tester).read(disposableReviewProvider.notifier).hide();
    await tester.pumpAndSettle();
    expect(containerOf(tester).read(disposableReviewProvider).open, isFalse);
    await tester.tap(find.byKey(const Key('setup-changes-open')));
    await tester.pumpAndSettle();
    expect(containerOf(tester).read(navigationProvider), Section.sandbox);
    expect(containerOf(tester).read(disposableReviewProvider).open, isTrue);
    expect(client.startModes, isEmpty, reason: 'nothing was started');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('without the capability the setup shows no switch and no hint', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = fakeDoctorOk()
      ..pendingChanges = fakeDisposableChanges();
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('setup-disposable')), findsNothing);
    expect(find.byKey(const Key('setup-changes-hint')), findsNothing);
    expect(find.byKey(const Key('setup-changes-open')), findsNothing);
    expect(startEnabled(tester), isTrue);
  });
}
