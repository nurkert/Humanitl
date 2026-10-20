# Fremde Werke in Humanitl

Humanitl steht unter GPL-3.0-only (`LICENSE`). Diese Datei nennt die Werke
Dritter, die das Repository selbst enthält und die eine Namensnennung
verlangen. Abhängigkeiten, die beim Bau aus ihren Registries kommen (Rust-Crates,
Dart-Pakete, die Flutter-Engine), stehen mit ihren Lizenzen in den
Paket-Metadaten und im Anwendungsbündel unter `data/flutter_assets/NOTICES.Z`;
für das `.deb` fasst `packaging/deb/copyright` alles zusammen.

## App-Icon: die Faust

- **Datei:** `packaging/deb/humanitl.svg`, daraus gerechnet
  `packaging/deb/icons/humanitl-{64,128,256}.png` (HUM-259).
- **Vorlage:** „hand-fist“ aus Font Awesome Free 6.6.0, © Fonticons, Inc.,
  <https://fontawesome.com/icons/hand-fist>.
- **Lizenz der Vorlage:** Creative Commons Attribution 4.0 International
  (CC BY 4.0), <https://creativecommons.org/licenses/by/4.0/>; die Lizenzen von
  Font Awesome Free im Überblick: <https://fontawesome.com/license/free>.
- **Änderungen:** Zeige- und Mittelfinger sind umgeformt. Die Faust ist auf
  34 Prozent verkleinert,
  in `#14161C` gefüllt und auf eine gelbe abgerundete Kachel mit senkrechtem
  Verlauf (`#EBC05A` oben, `#D49E34` unten) gesetzt. Kachel und Komposition sind
  für Humanitl gezeichnet.
- **Lizenz des Icons:** Das Icon als Ganzes steht unter CC BY 4.0, mit der
  Namensnennung oben. Die Herkunft steht zusätzlich als Kommentar in der
  SVG-Datei, damit sie auch dort mitreist, wo diese Datei nicht mitkommt (im
  AppImage liegt nur das SVG).
- **Im Paket:** `/usr/share/doc/humanitl/copyright` nennt die installierten
  Symbole unter `/usr/share/icons/hicolor/` mit derselben Herkunft.

## An anderer Stelle beschrieben

- Auszug aus der Majestic Million (CC BY 3.0): `catalog/RANKS-LICENSE`.
- Symbole des Domain-Katalogs (für Humanitl gezeichnet, GPL-3.0-only):
  `catalog/icons/LICENSES.md`.
