import 'package:flutter/widgets.dart';

import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// Which theme the application shows.
enum HThemeMode {
  /// Always dark. The design is drawn for this.
  dark,

  /// Always light.
  light,

  /// Follow the platform.
  system;

  /// The tokens this mode resolves to for a given [platformBrightness].
  HTokens resolve(Brightness platformBrightness) => switch (this) {
    HThemeMode.dark => HTokens.dark,
    HThemeMode.light => HTokens.light,
    HThemeMode.system => HTokens.forBrightness(platformBrightness),
  };

  /// The value of `ui.theme` in `config.toml` that selects this mode.
  String get configValue => name;
}

/// Publishes a set of [HTokens] to the widget tree.
///
/// Every widget in this package reads its colours through [HTheme.of]; none of
/// them takes a colour argument it could have looked up. When a component
/// library arrives, this is the one place that has to learn about it.
class HTheme extends StatelessWidget {
  /// Publishes [tokens] to [child].
  const HTheme({required this.tokens, required this.child, super.key});

  /// Publishes the dark tokens.
  HTheme.dark({required this.child, super.key}) : tokens = HTokens.dark;

  /// Publishes the light tokens.
  HTheme.light({required this.child, super.key}) : tokens = HTokens.light;

  /// The tokens made visible to [child].
  final HTokens tokens;

  /// The subtree that reads the tokens.
  final Widget child;

  /// The tokens of the closest enclosing [HTheme].
  ///
  /// Falls back to [HTokens.dark] so that a widget still renders in a test
  /// harness that forgot the theme, instead of throwing.
  static HTokens of(BuildContext context) => maybeOf(context) ?? HTokens.dark;

  /// The tokens of the closest enclosing [HTheme], or null.
  static HTokens? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<_HThemeScope>()?.tokens;

  @override
  Widget build(BuildContext context) {
    return _HThemeScope(
      tokens: tokens,
      child: DefaultTextStyle(
        style: HType.ui13.copyWith(color: tokens.colors.fg0),
        child: child,
      ),
    );
  }
}

class _HThemeScope extends InheritedWidget {
  const _HThemeScope({required this.tokens, required super.child});

  final HTokens tokens;

  @override
  bool updateShouldNotify(_HThemeScope oldWidget) => oldWidget.tokens != tokens;
}
