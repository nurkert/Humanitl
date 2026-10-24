// Goldens der Shell bei 1280×800, dunkel und hell (HUM-019). Erneuern mit
// `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/providers/theme.dart';

import '../harness/ui_state.dart';

Widget shell(HThemeMode mode) => ProviderScope(
  overrides: [
    uiStateOverride(),
    daemonClientProvider.overrideWithValue(FakeDaemonClient.empty()),
    connectionHeartbeatProvider.overrideWithValue(null),
    themeModeProvider.overrideWith(() => _FixedTheme(mode)),
  ],
  child: const HumanitlApp(),
);

class _FixedTheme extends ThemeModeSetting {
  _FixedTheme(this.mode);

  final HThemeMode mode;

  @override
  HThemeMode build() => mode;
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  goldenTest(
    'shell_dark',
    fileName: 'shell_dark',
    constraints: window,
    builder: () => shell(HThemeMode.dark),
  );

  goldenTest(
    'shell_light',
    fileName: 'shell_light',
    constraints: window,
    builder: () => shell(HThemeMode.light),
  );

  // Das offene Einstellungsblatt neben der Queue (HUM-286): Die Kopfzeile des
  // Blatts endet auf derselben Linie wie die der Queue, und das Blatt hängt
  // zwischen Kopf- und Statusleiste.
  for (final HThemeMode mode in <HThemeMode>[
    HThemeMode.dark,
    HThemeMode.light,
  ]) {
    goldenTest(
      'shell_settings_${mode.name}',
      fileName: 'shell_settings_${mode.name}',
      constraints: window,
      pumpBeforeTest: _openSettings,
      builder: () => shell(mode),
    );
  }
}

/// Öffnet das Blatt mit `Ctrl+,`, wie es ein Mensch täte.
Future<void> _openSettings(WidgetTester tester) async {
  await tester.pumpAndSettle();
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.comma);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pumpAndSettle();
}
