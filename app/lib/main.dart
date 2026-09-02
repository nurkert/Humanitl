/// Entry point of the Humanitl desktop application.
library;

import 'dart:io' show Platform;

import 'package:flutter/widgets.dart';

import 'core/ui/ui.dart';
import 'features/settings/gallery_screen.dart';

/// Environment variable that opens the design gallery instead of the shell.
const String galleryEnvironmentVariable = 'HUMANITL_GALLERY';

/// Command line flag that opens the design gallery instead of the shell.
const String galleryFlag = '--gallery';

/// Starts the application, or the design gallery when it was asked for.
void main(List<String> args) {
  runApp(galleryRequested(args) ? const GalleryScreen() : const HumanitlApp());
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
  final String value =
      (env[galleryEnvironmentVariable] ?? '').trim().toLowerCase();
  return value.isNotEmpty && value != '0' && value != 'false';
}

/// Placeholder root widget; replaced by the real shell in HUM-019.
class HumanitlApp extends StatelessWidget {
  /// Creates the placeholder root widget.
  const HumanitlApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const ColoredBox(color: HColors.bg0);
  }
}
