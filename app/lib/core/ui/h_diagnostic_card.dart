/// A card that shows one diagnostic: code, severity, title, cause, detail,
/// fix and documentation. Specified for `packages/ui` (HUM-019 Schritt 6);
/// it lives here until that package is touched again (handoff).
///
/// Like every `H*` widget it holds no user-visible string: every label comes
/// in already localised.
///
/// **Wo diese Karte nicht stehen darf.** Sie misst sich seit HUM-150 über
/// einen `LayoutBuilder`, und der beantwortet weder eine intrinsische Frage
/// noch eine Trockenmessung: Unter `IntrinsicHeight`, `IntrinsicWidth`,
/// `Table` oder einem anderen Elternteil, das eine dieser Fragen stellt,
/// wirft er in Debug „LayoutBuilder does not support returning intrinsic
/// dimensions." und liefert in Release still 0.0
/// (`flutter/src/widgets/layout_builder.dart`). Heute steht keine der
/// achtzehn Karten so; beide Muster kommen in diesem Programm aber vor
/// (`features/intercept/widgets/release_valve.dart`,
/// `features/sandbox/widgets/sandbox_table.dart`).
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// The card.
class HDiagnosticCard extends StatelessWidget {
  /// Creates a card.
  const HDiagnosticCard({
    required this.code,
    required this.severityLabel,
    required this.color,
    required this.title,
    required this.why,
    this.detail,
    this.fix,
    this.docsUrl,
    this.width = 560,
    super.key,
  });

  /// The registered code, for example `DAEMON_001`.
  final String code;

  /// The severity, already localised.
  final String severityLabel;

  /// The severity hue; never the blocked red.
  final Color color;

  /// The fixed part of the message.
  final String title;

  /// The cause, in the person's language.
  final String why;

  /// The technical detail, shown in monospace when present.
  final String? detail;

  /// The fix control, when there is one.
  final Widget? fix;

  /// Link to the documentation anchor, shown as text.
  final String? docsUrl;

  /// Width of the card.
  final double width;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? detail = this.detail;
    final Widget? fix = this.fix;
    final String? docsUrl = this.docsUrl;
    return Semantics(
      container: true,
      label: '$code $title',
      child: SizedBox(
        width: width,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: tokens.colors.bg2,
            borderRadius: BorderRadius.circular(tokens.radii.card),
            border: Border.all(color: tokens.colors.lineStrong),
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(tokens.radii.card),
            // Wo der Platz nicht reicht, wird gescrollt und nicht
            // abgeschnitten (HUM-150).
            //
            // Vorher stand hier `IntrinsicHeight` über einer `Row` mit
            // `CrossAxisAlignment.stretch`, damit die Schiene die Höhe des
            // Textes bekommt. `IntrinsicHeight` schätzt die Höhe nicht falsch
            // — gemessen stimmt sie bei 280 und 560 Pixeln Breite und bei
            // Textskalierung 1 und 2 auf das Pixel —, aber es reicht sie durch
            // `BoxConstraints.tighten` und klemmt sie damit in die Schranke
            // des Elternteils. Wo die kleiner war als der Satz des Daemons,
            // bekam die Zeile eine zu kleine feste Höhe, die Spalte lief über,
            // und dieses `ClipRRect` schnitt den Rest stumm ab: kein
            // Auslassungszeichen, kein Weg an den Rest des Satzes.
            // Wo das Elternteil eine Schranke setzt, scrollt der Inhalt
            // stattdessen in einer `SingleChildScrollView`, die ihm eine
            // unbegrenzte Höhe reicht.
            //
            // **Nur dort.** Ohne Schranke — in der Liste des Streifens, in
            // einer Spalte, an den meisten der achtzehn Stellen, an denen
            // diese Karte gebaut wird — entsteht gar keine Ansicht, die
            // scrollen könnte, und das ist kein Geiz: Auf Linux
            // hängt `ScrollBehavior.buildScrollbar` an **jedes** `Scrollable`
            // einen eigenen `RawScrollbar` (`WidgetsApp` ohne
            // `scrollBehavior`, `app.dart`), und `ScrollAction` sucht sich zu
            // „Bild ab" das **innerste** `Scrollable` vom Fokus aus, meldet
            // die Taste als verbraucht und bewegt nichts, wenn dieses nichts
            // zu scrollen hat. Eine leere Ansicht in der Karte ergäbe also
            // einen zweiten Balken über dem des Streifens und eine tote
            // Bild-Taste (`docs/UX.md` 5.3). Im Test fällt beides nicht auf:
            // `flutter test` läuft als `TargetPlatform.android`, und dort baut
            // die Vorgabe keine Balken.
            //
            // Eine dieser achtzehn Stellen hat eine Schranke und baut die
            // Ansicht deshalb wirklich: die Fehlkarte im History-Detail
            // (`features/history/history_detail.dart`, `_Failure`), die in
            // einem `Flexible` der unteren Hälfte des geteilten Panes steht.
            // Dort entsteht auf Linux auch der Balken der Plattform über der
            // Karte. Er stört dort heute nichts — diese Karte trägt keinen
            // Vorschlag, ihr Doku-Verweis ist reiner Text, und es gibt keinen
            // Fokus-Halt in ihr, an dem `ScrollAction` hängen bliebe. Belegt
            // ist das aus dem Bau der Widgets, nicht aus einer Messung: Kein
            // Test fährt diesen Zweig.
            child: LayoutBuilder(
              builder: (BuildContext context, BoxConstraints constraints) {
                // Die Schiene liegt im Stapel und nicht in einer Zeile, damit
                // sie die volle Höhe des Satzes trägt, ohne dass jemand die
                // Höhe vorher schätzen muss; dasselbe Mittel wie in `HRow`.
                final Widget body = Stack(
                  children: <Widget>[
                    Padding(
                      padding: EdgeInsetsDirectional.fromSTEB(
                        HSize.stateRail + tokens.spacing.x4,
                        tokens.spacing.x4,
                        tokens.spacing.x4,
                        tokens.spacing.x4,
                      ),
                      child: SizedBox(
                        // Volle Breite wie vorher das `Expanded`: Ein Kind des
                        // Stapels bekäme sonst lose Schranken, und die Karte
                        // schrumpfte auf ihren längsten Absatz.
                        width: double.infinity,
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            // `Wrap` und nicht `Row`: bei doppelter
                            // Textskalierung passen Code und Schweregrad nicht
                            // mehr nebeneinander, und eine Zeile schnitte sie
                            // ab, statt umzubrechen (`docs/UX.md` 6).
                            Wrap(
                              spacing: tokens.spacing.x2,
                              runSpacing: tokens.spacing.x1,
                              children: <Widget>[
                                HBadge(text: code, color: color, mono: true),
                                HBadge(text: severityLabel, color: color),
                              ],
                            ),
                            SizedBox(height: tokens.spacing.x2),
                            Text(
                              title,
                              style: tokens.typography.ui16.semibold.tinted(
                                tokens.colors.fg0,
                              ),
                            ),
                            SizedBox(height: tokens.spacing.x2),
                            Text(
                              why,
                              style: tokens.typography.ui13.tinted(
                                tokens.colors.fg1,
                              ),
                            ),
                            if (detail != null &&
                                detail.isNotEmpty) ...<Widget>[
                              SizedBox(height: tokens.spacing.x3),
                              DecoratedBox(
                                decoration: BoxDecoration(
                                  color: tokens.colors.bg1,
                                  borderRadius: BorderRadius.circular(
                                    tokens.radii.control,
                                  ),
                                  border: Border.all(color: tokens.colors.line),
                                ),
                                child: Padding(
                                  padding: EdgeInsets.symmetric(
                                    horizontal: tokens.spacing.x3,
                                    vertical: tokens.spacing.x2,
                                  ),
                                  child: Text(
                                    detail,
                                    style: tokens.typography.mono12.tinted(
                                      tokens.colors.fg1,
                                    ),
                                  ),
                                ),
                              ),
                            ],
                            if (fix != null) ...<Widget>[
                              SizedBox(height: tokens.spacing.x3),
                              fix,
                            ],
                            if (docsUrl != null &&
                                docsUrl.isNotEmpty) ...<Widget>[
                              SizedBox(height: tokens.spacing.x3),
                              Text(
                                docsUrl,
                                // Der Akzent ist eine Fläche; ein Wort darauf
                                // nimmt seine Textvariante (`docs/UX.md` 6).
                                style: tokens.typography.mono12.tinted(
                                  tokens.colors.accentText,
                                ),
                              ),
                            ],
                          ],
                        ),
                      ),
                    ),
                    PositionedDirectional(
                      top: 0,
                      bottom: 0,
                      start: 0,
                      width: HSize.stateRail,
                      child: ColoredBox(color: color),
                    ),
                  ],
                );
                return constraints.hasBoundedHeight
                    ? SingleChildScrollView(child: body)
                    : body;
              },
            ),
          ),
        ),
      ),
    );
  }
}
