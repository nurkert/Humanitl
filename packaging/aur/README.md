# AUR-Paket humanitl-bin

Dieses Verzeichnis erzeugt das AUR-Paket `humanitl-bin` (HUM-339) für Arch
Linux, Manjaro, EndeavourOS und andere Arch-Abkömmlinge. Es installiert die
Programme und das Flutter-Bundle aus dem Release-Archiv
`humanitl-<version>-linux-x86_64.tar.gz`, also dieselben Dateien wie das .deb,
in derselben Anordnung:

| Pfad | Inhalt |
|---|---|
| `/usr/lib/humanitl/` | Flutter-Bundle (`humanitl`, `lib/`, `data/`) |
| `/usr/lib/humanitl/bin/` | `humanitld`, `humanitl`, `humanitl-shim` |
| `/usr/bin/humanitl`, `/usr/bin/humanitl-app` | Symlinks wie im .deb |
| `/usr/lib/systemd/user/humanitld.{service,socket}` | Nutzer-Units des Pakets |
| `/usr/share/applications/dev.humanitl.humanitl.desktop` | Menüeintrag |
| `/usr/share/icons/hicolor/…/apps/humanitl.{svg,png}` | Symbole |
| `/usr/share/humanitl/{catalog,profiles/sandbox}` | Katalog und Profil `default` |
| `/usr/share/licenses/humanitl-bin/{LICENSE,copyright}` | statt `/usr/share/doc/humanitl` |

Gepflegt wird hier nur die Vorlage. Das AUR selbst ist ein eigenes
Git-Repository, in das der Maintainer von Hand hochlädt; kein Skript und kein
Workflow pusht dorthin.

## Dateien

- `PKGBUILD.in`: Vorlage des PKGBUILD mit Platzhaltern für Version,
  Mindestversion von bubblewrap und Prüfsummen.
- `humanitl-bin.install.in`: Vorlage des Install-Skripts. `render.sh` setzt an
  die Zeile `#HUMANITL_USERS#` denselben Teil, den `packaging/deb/maintscripts.sh`
  in `postinst` und `postrm` des .deb setzt (`packaging/deb/maintscript-users.sh`):
  Nach Installation und Update laden die laufenden systemd-Nutzerinstanzen neu,
  und ein Daemon ohne Agenten-Sitzung startet mit dem neuen Programm; nach dem
  Entfernen wird nur neu geladen.
- `render.sh <0.0.N> <archiv-oder-sha256> [<out-dir>]`: schreibt das fertige
  AUR-Repository nach `<out-dir>` (ohne Angabe `./aur-humanitl-bin-<version>`):
  `PKGBUILD`, `.SRCINFO`, `humanitl-bin.install` und die lokalen Quellen.
  `copyright` ist die Datei des .deb; nur der Satz mit dem Debian-Pfad
  `/usr/share/common-licenses/GPL-3` zeigt hier auf `LICENSE` im selben
  Verzeichnis.
- `check-aur.sh`: Prüfung im Container `archlinux:base-devel`, siehe unten.

## Warum lokale Quellen

Das Release-Archiv enthält weder die Nutzer-Units noch Desktop-Datei, Symbole
oder `copyright`. Die bringt das AUR-Repository als lokale Quellen mit, jede
mit eigener SHA-256 im PKGBUILD: `humanitld.service` und `humanitld.socket`
(schon so erzeugt wie im .deb, mit `ExecStart=/usr/lib/humanitl/bin/humanitld`
und ohne die Marke von `humanitl daemon install`), `dev.humanitl.humanitl.desktop`,
`humanitl.svg`, `humanitl-{64,128,256}.png` und `copyright`.

`render.sh` nimmt diese Dateien aus dem Arbeitsbaum. Deshalb muss der
Arbeitsbaum beim Rendern auf dem Tag `v<version>` stehen; sonst warnt das
Skript. Sollte das Archiv die Dateien künftig selbst enthalten, fallen die
lokalen Quellen weg und `package()` nimmt sie aus dem Archiv.

## Ein Release ins AUR bringen

Einmalig: Konto auf <https://aur.archlinux.org> mit SSH-Schlüssel, dann

```sh
git clone ssh://aur@aur.archlinux.org/humanitl-bin.git ~/aur/humanitl-bin
```

Ein leeres Repository ist für einen neuen Paketnamen normal; der erste Push
legt das Paket an.

Für jedes Release `0.0.N`, nachdem der Release-Lauf grün war und das Release
auf GitHub steht:

1. Den Arbeitsbaum auf den Tag stellen und die Prüfsumme des Archivs aus
   `SHA256SUMS` des Releases nehmen (oder das Archiv selbst übergeben):

   ```sh
   git checkout v0.0.N
   sha=$(curl -fsSL https://github.com/nurkert/Humanitl/releases/download/v0.0.N/SHA256SUMS \
     | awk '$2 == "humanitl-0.0.N-linux-x86_64.tar.gz" { print $1 }')
   packaging/aur/render.sh 0.0.N "$sha" ~/aur/humanitl-bin
   ```

   Dieselben Dateien legt der Release-Lauf unter `dist-meta/aur/` in sein
   Workflow-Artefakt `humanitl-release` (14 Tage aufbewahrt); der Job
   `check-aur` hat genau sie gebaut und geprüft. Sie sind bewusst kein
   Release-Asset: Ein Nutzer braucht sie nicht, und PKGBUILD und `.SRCINFO`
   allein wären ohne die lokalen Quellen unvollständig. Wer sie aus dem
   Artefakt nimmt, statt neu zu rendern, bekommt byte-gleiche Dateien.

2. Wer Arch zur Hand hat, baut einmal zur Probe: `cd ~/aur/humanitl-bin &&
   makepkg -si`, danach `makepkg --printsrcinfo | diff - .SRCINFO`. Sonst
   genügt `check-aur.sh`, siehe unten.
3. Hochladen:

   ```sh
   cd ~/aur/humanitl-bin
   git add PKGBUILD .SRCINFO humanitl-bin.install humanitld.service humanitld.socket \
     dev.humanitl.humanitl.desktop humanitl.svg humanitl-64.png humanitl-128.png \
     humanitl-256.png copyright
   git commit -m "Update to 0.0.N"
   git push origin master
   ```

   Das AUR nimmt nur den Zweig `master` an und weist einen Push ohne
   `.SRCINFO` ab. `pkgrel` bleibt `1`; ändert sich nur das Paket und nicht die
   Software, erhöht man ihn im erzeugten PKGBUILD und in `.SRCINFO` von Hand.

## Prüfung

`check-aur.sh <render-dir> <tarball> <0.0.N> [<reference-deb>]` läuft als
root in `archlinux:base-devel@sha256:624bdc7fc6615155d67bc34cf7146c0c9fd0e3c92943977da63a630e4ca49c30`
und prüft der Reihe nach:

1. `.SRCINFO` gleicht `makepkg --printsrcinfo` Zeichen für Zeichen;
2. `makepkg` baut aus dem lokalen Archiv (unter dem Dateinamen der Quelle
   lädt makepkg nichts herunter, prüft aber die SHA-256), ohne `-debug`-Paket,
   und `.INSTALL` im Paket ist das gerenderte Install-Skript;
3. `namcap` auf PKGBUILD und Paket meldet keinen Fehler (`E:`);
4. `pacman -U` löst `depends` aus dem Archiv auf, `pacman -Qkk` ist sauber,
   nichts liegt außerhalb von `/usr`, Rechte und Symlinks wie im .deb; mit
   `<reference-deb>` liefert das Paket genau die Pfade und Rechte des .deb,
   bis auf `/usr/share/doc` und die lintian-Overrides (Debian) und
   `/usr/share/licenses` (Arch);
5. dieselben Prüfungen nach der Installation wie `packaging/deb/check-install.sh`:
   ldd, Versionen, Starttest der Anwendung unter Xvfb, Desktop-Datei, Units,
   Symbole, `humanitl daemon install --print` erkennt die Units des Pakets;
6. nach `pacman -Rns` bleibt nichts mit „humanitl“ im Namen zurück.

Der Container braucht Netz zu den Arch-Spiegeln. Unter qemu-Emulation (etwa
colima auf Apple Silicon) scheitert die seccomp-Sandbox von pacman 7 mit
`error restricting syscalls via seccomp`; dort hilft vor dem Aufruf
`sed -i '/^\[options\]/a DisableSandbox' /etc/pacman.conf`. Auf einem
x86_64-Runner ist das nicht nötig.

## Abhängigkeiten

`depends` stammt aus den NEEDED-Einträgen der Programme und des Bundles,
übersetzt in Arch-Pakete: `glibc>=2.39 libgcc libstdc++ zlib gtk3 glib2 pango harfbuzz
cairo gdk-pixbuf2 at-spi2-core libepoxy fontconfig`. Dazu `libglvnd`, weil die
Flutter-Engine `libEGL.so.1` und `libGLESv2.so.2` mit dlopen lädt (wie
`libegl1, libgles2` im .deb), und `bubblewrap>=<MIN_BWRAP_VERSION>`, das der
Daemon startet. `at-spi2-core` liefert auf heutigem Arch `libatk-1.0.so`.

glibc 2.39 ist die Untergrenze, weil die Programme auf Ubuntu 24.04 gebaut
werden. Arch und seine Abkömmlinge mit aktuellem Stand liegen weit darüber.

`hicolor-icon-theme` steht in `depends`, weil es die Symbolverzeichnisse
besitzt; ohne den Eintrag meldet namcap einen Fehler.

## Erwartete Hinweise von namcap

`check-aur.sh` fällt nur bei `E:` durch. Diese Warnungen bleiben und sind
begründet (Stand 0.0.12, namcap 3.6.0):

| Warnung | Warum sie bleibt |
|---|---|
| `Reference to x86_64 should be changed to $CARCH` (2×) | Der Name des Release-Archivs enthält `x86_64` fest; es gibt nur diese Architektur. |
| `lacks FULL RELRO` (Anwendung, Plugins, `libapp.so`, Shim), `humanitl-shim lacks PIE` | So baut sie Flutter beziehungsweise der statisch gelinkte Shim; dieselben Dateien wie im .deb. Ein Binärpaket baut nicht neu. |
| `Referenced library 'libflutter_linux_gtk.so' is an uninstalled dependency` | Private Bibliothek des Bundles unter `/usr/lib/humanitl/lib`, gefunden über den RUNPATH der Anwendung. |
| `Dependency included, but may not be needed ('libglvnd')` | dlopen von `libEGL.so.1` und `libGLESv2.so.2`; namcap sieht nur NEEDED. |
| `Dependency included, but may not be needed ('bubblewrap')` | Der Daemon startet `bwrap` als Programm. |
| `Unused shared library …` | Überflüssige NEEDED-Einträge aus den Builds von Rust und Flutter, harmlos. |
| `Missing Contributor tag` (I:) | Ein Maintainer, keine Mitwirkenden. |

`options=('!strip' '!debug')` ist Pflicht: `strip` würde den Dart-AOT-Snapshot
`libapp.so` beschädigen, und `debug` spaltete ein leeres `-debug`-Paket ab.
