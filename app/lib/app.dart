/// The root widget: theme, localisation, overlay and the connection gate
/// (HUM-019 Widget-Baum). No Material, no Navigator: the shell is one window
/// with an `IndexedStack`, sheets and modals are widgets in an overlay.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/ui/ui.dart';
import 'features/shell/connection_gate.dart';
import 'features/shell/providers/theme.dart';
import 'l10n/l10n.dart';

/// The application. Mount it below a `ProviderScope`.
class HumanitlApp extends ConsumerWidget {
  /// Creates the application.
  const HumanitlApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HThemeMode mode = ref.watch(themeModeProvider);
    return WidgetsApp(
      color: HColors.bg0,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      onGenerateTitle: (BuildContext context) => context.l10n.appTitle,
      builder: (BuildContext context, Widget? child) {
        final HTokens tokens = mode.resolve(
          MediaQuery.platformBrightnessOf(context),
        );
        return HTheme(
          tokens: tokens,
          child: ColoredBox(
            color: tokens.colors.bg0,
            child: Overlay(
              initialEntries: <OverlayEntry>[
                OverlayEntry(
                  builder: (BuildContext context) => const ConnectionGate(),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
