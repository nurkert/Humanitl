/// Der Dialog „Über Humanitl“: Name, Version, die Lizenz des Programms, die
/// Namensnennung der mitgelieferten Rangliste und der Weg zu Flutters
/// Lizenzseite (HUM-031).
///
/// Gebaut aus Flutters [AboutDialog] und [LicensePage], nicht von Hand. Beide
/// sind Material, der Rest der Anwendung ist es nicht; ein [Theme] aus den
/// Token liegt deshalb um den Dialog, damit er in beiden Helligkeiten die
/// Farben der Anwendung trägt. Die Lizenzseite erbt es, weil
/// `showLicensePage` die Themen des Dialogs mitnimmt.
library;

import 'package:flutter/material.dart';

import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';

/// Die Version dieses Baus, wie das `flutter`-Werkzeug sie aus
/// `pubspec.yaml` gelesen hat (`FLUTTER_BUILD_NAME`); leer, wo es sie nicht
/// gesetzt hat.
const String appVersion = String.fromEnvironment('FLUTTER_BUILD_NAME');

/// Öffnet den Dialog über dem Navigator von [context].
///
/// Das ist, was `showAboutDialog` tut, mit einem Unterschied: Dessen Dialog
/// nimmt kein Thema an, und ohne eines stünde er in einem dunklen Fenster
/// hell da.
///
/// Thema und Texte werden im `builder` gelesen, aus dem Kontext des Dialogs:
/// Wechselt die Helligkeit oder die Sprache, während der Dialog oder die
/// Lizenzseite offen ist, zieht beides nach, statt beim Stand des Öffnens zu
/// bleiben.
Future<void> showHumanitlAbout(BuildContext context) {
  return showDialog<void>(
    context: context,
    builder: (BuildContext dialogContext) {
      final AppLocalizations l10n = dialogContext.l10n;
      return Theme(
        data: _materialTheme(HTheme.of(dialogContext)),
        child: AboutDialog(
          applicationName: l10n.appTitle,
          applicationVersion: appVersion.isEmpty ? null : appVersion,
          applicationLegalese: l10n.aboutLegalese,
          children: <Widget>[
            const SizedBox(height: 16),
            // Markierbar, damit sich die beiden Adressen kopieren lassen; ein
            // Link bräuchte ein Plugin, das den Browser öffnet.
            SelectableText(l10n.aboutRanks, key: const Key('about-ranks')),
          ],
        ),
      );
    },
  );
}

/// Das Material-Thema von Dialog und Lizenzseite, aus [tokens].
///
/// Nur die Flächen, der Text und der Akzent; alles andere leitet Material aus
/// dem Akzent ab.
ThemeData _materialTheme(HTokens tokens) {
  final HSurfaceColors colors = tokens.colors;
  final ColorScheme scheme =
      ColorScheme.fromSeed(
        seedColor: colors.accent,
        brightness: tokens.brightness,
      ).copyWith(
        primary: colors.accentText,
        surface: colors.bg1,
        onSurface: colors.fg0,
        onSurfaceVariant: colors.fg1,
        surfaceContainerLowest: colors.bg0,
        surfaceContainerLow: colors.bg1,
        surfaceContainer: colors.bg1,
        surfaceContainerHigh: colors.bg2,
        surfaceContainerHighest: colors.bg3,
        outline: colors.lineStrong,
        outlineVariant: colors.line,
      );
  return ThemeData(
    colorScheme: scheme,
    scaffoldBackgroundColor: colors.bg0,
    fontFamily: HType.uiFamily,
    fontFamilyFallback: HType.uiFallback,
  );
}
