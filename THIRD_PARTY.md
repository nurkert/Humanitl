# Third-party works in Humanitl

Humanitl is licensed under GPL-3.0-only (`LICENSE`). This file names the
third-party works that the repository itself contains and that require
attribution. Dependencies that come from their registries at build time (Rust
crates, Dart packages, the Flutter engine) are listed with their licences in
the package metadata and in the application bundle under
`data/flutter_assets/NOTICES.Z`; for the `.deb`, `packaging/deb/copyright`
sums everything up.

## App icon: the fist

- **File:** `packaging/deb/humanitl.svg`, rendered from it
  `packaging/deb/icons/humanitl-{64,128,256}.png` (HUM-259).
- **Source:** "hand-fist" from Font Awesome Free 6.6.0, © Fonticons, Inc.,
  <https://fontawesome.com/icons/hand-fist>.
- **Licence of the source:** Creative Commons Attribution 4.0 International
  (CC BY 4.0), <https://creativecommons.org/licenses/by/4.0/>; an overview of
  the Font Awesome Free licences: <https://fontawesome.com/license/free>.
- **Changes:** index and middle finger are reshaped. The fist is scaled down to
  34 per cent,
  filled in `#14161C` and placed on a yellow rounded tile with a vertical
  gradient (`#EBC05A` at the top, `#D49E34` at the bottom). Tile and
  composition are drawn for Humanitl.
- **Licence of the icon:** the icon as a whole is licensed under CC BY 4.0,
  with the attribution above. The origin is also given as a comment in the SVG
  file, so that it travels along where this file does not come along (the
  AppImage contains only the SVG).
- **In the package:** `/usr/share/doc/humanitl/copyright` names the installed
  icons under `/usr/share/icons/hicolor/` with the same origin.
- **On the website:** `website/public/favicon.svg` is a copy of the SVG,
  `website/public/apple-touch-icon.png` is rendered from it, and
  `website/src/components/Logo.astro` draws the same paths (HUM-333); each
  carries the attribution above.

## Website: platform logos

- **File:** `website/src/lib/platform-icons.ts` (the Apple and Linux logos on
  the download page, HUM-333).
- **Source:** Simple Icons 16.34.0, <https://simpleicons.org>, under CC0 1.0
  (no attribution required; named here for the record). The logos are
  trademarks of their owners and name the platform only.

## Described elsewhere

- Excerpt from the Majestic Million (CC BY 3.0): `catalog/RANKS-LICENSE`.
- Icons of the domain catalogue (drawn for Humanitl, GPL-3.0-only):
  `catalog/icons/LICENSES.md`.
