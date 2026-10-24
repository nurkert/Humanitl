// Das Banner „This desktop has no tray" bleibt geschlossen, auch nach einem
// Neustart (HUM-276).
//
// Ein Neustart ist hier ein zweiter Aufbau der App über einem neuen Container
// und demselben Zustandsspeicher: dieselben Einstellungen, alles andere neu.
// Ein frischer Speicher ist eine frische Installation, und dort erscheint das
// Banner wieder.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/providers/coach_mark.dart';
import 'package:humanitl/features/shell/providers/notice_dismissal.dart';
import 'package:humanitl/features/tray/providers/notice.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';
import 'package:humanitl/features/tray/widgets/attention_notice.dart';

import 'fixtures.dart';

FakeDesktop noTray() => FakeDesktop(
  missing: TrayDiagnostics.trayUnavailable('no watcher on the session bus'),
);

/// Baut die App neu, wie ein Neustart sie baut: Erst ein leerer Baum, damit
/// der alte Container nichts mehr trägt, dann die App über [store].
Future<ProviderContainer> restart(
  WidgetTester tester,
  UiStateFile store,
) async {
  await tester.pumpWidget(const SizedBox.shrink());
  return pumpTrayShell(tester, desktop: noTray(), uiState: store);
}

void main() {
  testWidgets('notice_dismissal_persists_test', (WidgetTester tester) async {
    final UiStateFile store = trayUiState();

    // Erster Start: Das Tray fehlt, das Banner steht.
    final FakeDesktop first = noTray();
    await pumpTrayShell(tester, desktop: first, uiState: store);
    expect(first.tray.starts, 1);
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);

    await tester.tap(find.byKey(const Key('attention-notice-dismiss')));
    await tester.pump();
    expect(find.byType(AttentionNoticeBanner), findsNothing);
    // Geschrieben unter dem Code, und nur unter ihm.
    expect(store.read(), <String, Object?>{
      dismissedNoticeKey(DiagnosticCodes.noTray): true,
    });

    // Zweiter Start mit denselben Einstellungen: Das Tray fehlt noch immer,
    // erkannt wird es wie vorher, das Banner bleibt weg.
    final ProviderContainer again = await restart(tester, store);
    expect(again.read(dismissedNoticesProvider), <String>{
      DiagnosticCodes.noTray,
    });
    expect(find.byType(AttentionNoticeBanner), findsNothing);
    expect(again.read(attentionNoticeProvider), isNull);

    // Frische Einstellungen: Das Banner ist wieder da.
    await restart(tester, trayUiState());
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);
    expect(find.text(DiagnosticCodes.noTray), findsOneWidget);
  });

  testWidgets('a_notice_about_one_event_is_not_remembered', (
    WidgetTester tester,
  ) async {
    // `IPC_003` sagt etwas über eine Benachrichtigung, die zu spät beantwortet
    // wurde. Wer sie schließt, will die nächste trotzdem sehen.
    final UiStateFile store = trayUiState();
    final ProviderContainer container = await pumpTrayShell(
      tester,
      desktop: FakeDesktop(),
      uiState: store,
    );
    container
        .read(attentionNoticeProvider.notifier)
        .show(TrayDiagnostics.decidedAlready(trayFlowId(1)));
    await tester.pump();
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);

    await tester.tap(find.byKey(const Key('attention-notice-dismiss')));
    await tester.pump();
    expect(find.byType(AttentionNoticeBanner), findsNothing);
    expect(store.read(), isEmpty);
    expect(container.read(dismissedNoticesProvider), isEmpty);
  });
}
