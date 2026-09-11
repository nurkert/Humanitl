/// Die Lizenzen der Daten, die mit der Anwendung ausgeliefert werden, auf
/// Flutters Lizenzseite (HUM-031).
///
/// Die Seite listet ohnehin jedes Paket mit seiner Lizenz; die Rangliste steht
/// dort daneben, im Wortlaut von `catalog/RANKS-LICENSE` und nicht in einer
/// Zusammenfassung, die mit der nächsten Aktualisierung der Liste auseinander
/// liefe.
///
/// Die Symbole aus `catalog/icons` brauchen keinen eigenen Eintrag: Sie sind
/// für Humanitl gezeichnet, stehen unter GPL-3.0-only wie das Programm selbst
/// (`catalog/icons/LICENSES.md`) und liegen heute nicht im Asset-Bündel.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Wo die Lizenz der Rangliste im Asset-Bündel liegt.
///
/// Eine Kopie von `catalog/RANKS-LICENSE`; ein Test hält sie Byte für Byte
/// gegen das Original.
const String ranksLicenceAsset = 'assets/catalog/RANKS-LICENSE';

/// Der Name, unter dem die Lizenzseite die Rangliste führt.
///
/// Ein Eigenname und in jeder Sprache derselbe; eingetragen wird, bevor es
/// einen `BuildContext` und damit eine Sprache gibt.
const String ranksLicencePackage = 'Majestic Million';

/// Trägt die Lizenzen der gebündelten Daten in die [LicenseRegistry] ein.
///
/// Einmal je Prozess, vor `runApp`. Gelesen wird erst, wenn jemand die
/// Lizenzseite öffnet.
void registerBundledLicences() {
  LicenseRegistry.addLicense(_ranksLicence);
}

Stream<LicenseEntry> _ranksLicence() async* {
  final String text = await rootBundle.loadString(ranksLicenceAsset);
  yield LicenseEntryWithLineBreaks(const <String>[ranksLicencePackage], text);
}
