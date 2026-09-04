/// Entry point of the Humanitl desktop application.
library;

import 'dart:async' show unawaited;
import 'dart:io' show Platform;

import 'package:flutter/services.dart' show MissingPluginException;
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import 'app.dart';
import 'core/ipc/client_providers.dart';
import 'core/ipc/launch_options.dart';
import 'features/settings/gallery_screen.dart';
import 'features/tray/desktop_ports.dart';
import 'features/tray/platform/dbus_notifications.dart';
import 'features/tray/platform/sni_tray.dart';
import 'features/tray/platform/window_port.dart';
import 'features/tray/providers/attention.dart';

export 'app.dart' show HumanitlApp;

/// Environment variable that opens the design gallery instead of the shell.
const String galleryEnvironmentVariable = 'HUMANITL_GALLERY';

/// Command line flag that opens the design gallery instead of the shell.
const String galleryFlag = '--gallery';

/// The window title. The product name is the same in every language
/// (`appTitle` in the ARB files); it is set here, before any `BuildContext`
/// exists, because the Linux runner names the window from native code.
const String windowTitle = 'Humanitl';

/// The smallest window the three-pane layout fits into (HUM-019 Schritt 7).
const Size windowMinimumSize = Size(1100, 700);

/// Starts the application, or the design gallery when it was asked for.
Future<void> main(List<String> args) async {
  if (galleryRequested(args)) {
    runApp(const GalleryScreen());
    return;
  }
  WidgetsFlutterBinding.ensureInitialized();
  await configureWindow();
  final LaunchOptions options = LaunchOptions.resolve(args);
  final DesktopPorts ports = desktopPortsForPlatform();
  runApp(
    ProviderScope(
      overrides: [
        launchOptionsProvider.overrideWithValue(options),
        // `overrideWith` and not `overrideWithValue`: a value has no
        // lifetime, and these three ports hold two D-Bus connections and a
        // bus name. The `dbus` package says of its own client that a process
        // which leaves one open may not end at all, so the scope releases
        // them when it goes; the tray menu's `Quit` releases them before it
        // destroys the window.
        desktopPortsProvider.overrideWith((Ref ref) {
          ref.onDispose(() => unawaited(ports.dispose()));
          return ports;
        }),
      ],
      child: const HumanitlApp(),
    ),
  );
}

/// The desktop this run talks to: the real one on Linux, none anywhere else.
///
/// This is the only place the real tray, the real notification and the real
/// window are built. Every test and every headless run gets
/// [DesktopPorts.inert], so nothing below the ports ever needs a session bus
/// to be present (HUM-034).
DesktopPorts desktopPortsForPlatform() {
  if (!Platform.isLinux) {
    return DesktopPorts.inert();
  }
  try {
    return DesktopPorts(
      window: WindowManagerPort(),
      notifications: DBusNotificationPort(),
      tray: SniTrayPort(),
    );
  } on Object {
    // No session bus at all -- a headless run, a broken environment. The
    // program is about the window, not about the tray.
    return DesktopPorts.inert();
  }
}

/// Title and minimum size of the window, after `ensureInitialized` (HUM-019
/// Fallstricke). Quietly does nothing where the plugin is absent (tests).
Future<void> configureWindow() async {
  try {
    await windowManager.ensureInitialized();
    await windowManager.setTitle(windowTitle);
    await windowManager.setMinimumSize(windowMinimumSize);
  } on MissingPluginException {
    // No native window: a test binding or an unsupported platform.
  }
}

/// True when [args] or [environment] ask for the design gallery.
///
/// [environment] defaults to the process environment; tests pass their own so
/// that they never depend on the machine they run on.
bool galleryRequested(List<String> args, {Map<String, String>? environment}) {
  if (args.contains(galleryFlag)) {
    return true;
  }
  final Map<String, String> env = environment ?? Platform.environment;
  final String value = (env[galleryEnvironmentVariable] ?? '')
      .trim()
      .toLowerCase();
  return value.isNotEmpty && value != '0' && value != 'false';
}
