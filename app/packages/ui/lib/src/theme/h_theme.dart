import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'shadcn_theme.dart';

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
/// them takes a colour argument it could have looked up. Dies ist zugleich die
/// eine Stelle, die von `shadcn_flutter` weiß: sie baut aus den Token das
/// `ThemeData` der Bibliothek ([HShadcnTheme]) und veröffentlicht daneben die
/// Komponententhemen, aus denen Button, Eingabefeld, Kästchen, Haarlinie,
/// Karte und Fokusring der Bibliothek ihre Farben nehmen. Ein Bildschirm sieht
/// davon nichts; er sieht `HButton`, `HRow`, `HModal`.
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

  /// Stellt sicher, dass [child] ein Theme der Bibliothek über sich hat.
  ///
  /// `Theme.of` der Bibliothek bricht ab, wenn keines da ist, und jedes
  /// `H*`-Widget, das eine ihrer Komponenten aufhängt, liefe damit ohne
  /// [HTheme] in eine Zusicherung statt in die dunklen Token. [HTheme.of]
  /// verspricht seit jeher den Rückfall, also hält ihn diese Funktion.
  ///
  /// Im Normalfall — und der ist in der Anwendung wie im Testgerüst immer
  /// gegeben — steht ein [HTheme] darüber, und die Funktion gibt [child]
  /// unverändert zurück. Sie kostet dann kein einziges Element, was in einer
  /// Liste über zehntausend Zeilen der Unterschied zwischen einem Token-Lesen
  /// und zehntausend zusätzlichen `InheritedWidget`s ist.
  static Widget host(BuildContext context, Widget child) =>
      maybeOf(context) == null
      ? shad.Theme(data: HShadcnTheme.of(HTokens.dark), child: child)
      : child;

  @override
  Widget build(BuildContext context) {
    final HShadcnBundle bundle = HShadcnTheme.bundle(tokens);
    return _HThemeScope(
      tokens: tokens,
      child: shad.Theme(
        data: bundle.theme,
        child: shad.ComponentTheme<shad.FocusOutlineTheme>(
          data: bundle.focusOutline,
          child: shad.ComponentTheme<shad.TextFieldTheme>(
            data: bundle.textField,
            child: shad.ComponentTheme<shad.CheckboxTheme>(
              data: bundle.checkbox,
              child: shad.ComponentTheme<shad.DividerTheme>(
                data: bundle.divider,
                child: shad.ComponentTheme<shad.CardTheme>(
                  data: bundle.card,
                  child: shad.ComponentTheme<shad.OutlinedContainerTheme>(
                    data: bundle.outlinedContainer,
                    child: shad.ComponentTheme<shad.BadgeTheme>(
                      data: bundle.badge,
                      child: shad.ComponentTheme<shad.PrimaryButtonTheme>(
                        data: bundle.primaryButton,
                        child: shad.ComponentTheme<shad.SecondaryButtonTheme>(
                          data: bundle.secondaryButton,
                          child: shad.ComponentTheme<shad.GhostButtonTheme>(
                            data: bundle.ghostButton,
                            child:
                                shad.ComponentTheme<
                                  shad.DestructiveButtonTheme
                                >(
                                  data: bundle.destructiveButton,
                                  child:
                                      shad.ComponentTheme<shad.TextButtonTheme>(
                                        data: bundle.textButton,
                                        child:
                                            shad.ComponentTheme<
                                              shad.MutedButtonTheme
                                            >(
                                              data: bundle.mutedButton,
                                              child: DefaultTextStyle(
                                                style: tokens.typography.ui13
                                                    .tinted(tokens.colors.fg0),
                                                child: child,
                                              ),
                                            ),
                                      ),
                                ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
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
