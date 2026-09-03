// Goldens der Shell bei 1280×800, dunkel und hell (HUM-019). Erneuern mit
// `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/providers/theme.dart';

Widget shell(HThemeMode mode) => ProviderScope(
  overrides: [
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
}
