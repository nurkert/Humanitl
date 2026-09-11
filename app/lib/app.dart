/// The root widget: theme, localisation, navigator and the connection gate
/// (HUM-019 Widget-Baum). No Material in the shell: it is one window with an
/// `IndexedStack`, and sheets and modals are widgets in the navigator's
/// overlay.
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

  /// Die eine Seite des Navigators: ohne Übergang und ohne eigenen
  /// [PrimaryScrollController].
  ///
  /// Den Navigator gibt es seit HUM-031, und nur, damit Flutters About-Dialog
  /// und seine Lizenzseite eine Route haben, auf die sie sich legen können.
  /// Die Shell navigiert nicht; sie ist die einzige Seite darin und soll sich
  /// so verhalten wie vorher, als ein nacktes [Overlay] sie trug. Eine
  /// `ModalRoute` stellt ihrer Seite einen [PrimaryScrollController] hin, und
  /// auf den Plattformen, die ihn erben lassen, hängte sich jede senkrechte
  /// Liste der Shell ohne eigenen Controller daran; zwei davon an einem
  /// Controller vertragen sich nicht. [PrimaryScrollController.none] nimmt ihn
  /// wieder weg.
  static PageRoute<T> _page<T>(RouteSettings settings, WidgetBuilder builder) =>
      PageRouteBuilder<T>(
        settings: settings,
        transitionDuration: Duration.zero,
        reverseTransitionDuration: Duration.zero,
        pageBuilder: (
          BuildContext context,
          Animation<double> animation,
          Animation<double> secondaryAnimation,
        ) => PrimaryScrollController.none(child: builder(context)),
      );

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HThemeMode mode = ref.watch(themeModeProvider);
    return WidgetsApp(
      color: HColors.bg0,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      onGenerateTitle: (BuildContext context) => context.l10n.appTitle,
      pageRouteBuilder: _page,
      home: const ConnectionGate(),
      builder: (BuildContext context, Widget? child) {
        final HTokens tokens = mode.resolve(
          MediaQuery.platformBrightnessOf(context),
        );
        return HTheme(
          tokens: tokens,
          child: ColoredBox(color: tokens.colors.bg0, child: child),
        );
      },
    );
  }
}
