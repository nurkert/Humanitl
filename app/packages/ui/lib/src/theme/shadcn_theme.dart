import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../tokens/colors.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// Die Brücke von [HTokens] auf das Theme von `shadcn_flutter`.
///
/// **Die Richtung ist eine Entscheidung, keine Bequemlichkeit.** Die
/// Bibliothek bringt ein eigenes `ThemeData` mit `ColorScheme`, `Typography`,
/// `Density` und einem Radius-Faktor mit; wir bringen [HTokens] mit. Zwei
/// Paletten nebeneinander sind keine Lösung, also speist genau eine die
/// andere. Es ist [HTokens], das speist:
///
/// * Das `ColorScheme` der Bibliothek hat rund zwanzig allgemeine Plätze —
///   `background`, `card`, `primary`, `muted`, `destructive`, `border`,
///   `ring` und fünf Diagrammfarben. Die acht Zustandsfarben aus
///   `docs/UX.md` 3.3, die je nach Rolle als Fläche oder als Text gelesen
///   werden, haben dort keinen Platz. Wer [HTokens] aus dem `ColorScheme`
///   ableitete, verlöre entweder die Zustandsleiter oder führte eine zweite
///   Tabelle daneben — genau das, was diese Datei verhindert.
/// * Umgekehrt ist die Abbildung vollständig: jeder Platz des `ColorScheme`
///   bekommt eine Farbe unserer Leiter, siehe [colorScheme]. Damit malt jede
///   Komponente der Bibliothek in unserer Palette, ohne dass ein Widget eine
///   Farbe von Hand setzt.
/// * Die Hexwerte selbst stehen in `BACKLOG.md` 5 und sind in
///   `tokens_test.dart` eingefroren; die Kontrast-Ableitung
///   ([HColorDerivation]) rechnet auf ihnen. Eine fremde Palette an dieser
///   Stelle machte beide Prüfungen bedeutungslos.
///
/// Die Bündel werden je [HTokens] einmal gebaut und gemerkt. Das ist keine
/// Optimierung um ihrer selbst willen: `ButtonTheme` und Verwandte vergleichen
/// sich über Funktionsidentität, und ein bei jedem Aufbau neu erzeugtes Bündel
/// meldete jedem Button des Baumes eine Themenänderung.
abstract final class HShadcnTheme {
  /// Ein [Expando] und keine Karte: [HTokens] hat kein `==`, der Schlüssel ist
  /// also die Identität, und eine Karte hielte jedes je gesehene Token-Objekt
  /// für die Lebensdauer des Prozesses fest. Heute sind es die zwei
  /// Singletons; wer sich eigene Token baut — ein Test, ein späterer
  /// Theme-Editor —, füllte damit eine Karte, die niemand räumt. Das Expando
  /// hängt das Bündel an das Token und lässt beide zusammen gehen.
  static final Expando<HShadcnBundle> _cache = Expando<HShadcnBundle>(
    'HShadcnTheme',
  );

  /// Das Themenbündel zu [tokens], einmal gebaut und danach gemerkt.
  static HShadcnBundle bundle(HTokens tokens) =>
      _cache[tokens] ??= HShadcnBundle._of(tokens);

  /// Das `ThemeData` der Bibliothek zu [tokens].
  static shad.ThemeData of(HTokens tokens) => bundle(tokens).theme;

  /// Das `ColorScheme` der Bibliothek zu [tokens].
  ///
  /// Die Abbildung Platz für Platz, damit sie nachprüfbar ist:
  ///
  /// | shadcn | Humanitl | warum |
  /// |---|---|---|
  /// | `background` | `bg0` | die Anwendungsfläche |
  /// | `foreground` | `fg0` | Primärtext |
  /// | `card` | `bg1` | Panelfläche |
  /// | `popover` | `bg2` | die gehobene Fläche, auf der Overlays stehen |
  /// | `primary` | `accentFill` | die Füllung des einen Controls je Kontext |
  /// | `primaryForeground` | `onAccent` | das Wort darauf, 4,5:1 |
  /// | `secondary` | `bg2` | die zweite Handlung ist eine Fläche, kein Ton |
  /// | `muted` | `bg2` | Hover-Fläche eines stillen Controls |
  /// | `mutedForeground` | `fg2` | die Stufe für wirklich Deaktiviertes |
  /// | `accent` | `bg3` | in shadcn eine Hover-Fläche, **nicht** unser Akzent |
  /// | `destructive` | `state.blocked` | Rot heißt blockiert (`docs/UX.md` 3.3) |
  /// | `border`, `input` | `line` | die Haarlinie |
  /// | `ring` | `accent` | der Fokusring, und nur er trägt den Akzent |
  /// | `chart1..5` | fünf Zustandsfarben | die einzigen freien Plätze |
  ///
  /// `accent` ist die Stelle, an der ein flüchtiger Blick danebengreift: in
  /// shadcn ist das die Fläche, auf der ein Menüeintrag unter dem Zeiger
  /// steht, nicht die Markenfarbe. Unser Akzent gehört deshalb auf `ring` und
  /// `primary`, nicht auf `accent`.
  static shad.ColorScheme colorScheme(HTokens tokens) =>
      bundle(tokens).theme.colorScheme;

  /// Die Typografie der Bibliothek, aus [HType] gefüllt.
  ///
  /// Die Bibliothek benennt ihre Stufen `xSmall` bis `x9Large`; unsere Skala
  /// hat sechs UI-Größen und vier Monospace-Größen (`BACKLOG.md` 5). Die
  /// Zuordnung geht über die Bedeutung, nicht über den Namen: `base` ist die
  /// Dichte der Anwendung (13/20), `large` die Überschrift einer Karte
  /// (16/24), `xLarge` die eine Display-Größe (20/28). Alles, was oberhalb
  /// davon steht, hat in einem Kontrollraum keinen Auftritt und bekommt die
  /// nächstkleinere Stufe, damit kein Text in einer Größe erscheint, die die
  /// Skala nicht kennt.
  static shad.Typography typography(HTokens tokens) =>
      bundle(tokens).theme.typography;
}

/// Das fertige Themenbündel eines [HTokens]: `ThemeData` plus die
/// Komponententhemen, die `HTheme` darüber veröffentlicht.
///
/// Ein eigener Typ, weil die Komponententhemen der Bibliothek keine Felder von
/// `ThemeData` sind, sondern eigene `ComponentTheme`-Widgets. Sie hier zu
/// bündeln hält sie an derselben Quelle wie die Farben.
@immutable
class HShadcnBundle {
  const HShadcnBundle._({
    required this.theme,
    required this.focusOutline,
    required this.textField,
    required this.checkbox,
    required this.divider,
    required this.card,
    required this.outlinedContainer,
    required this.badge,
    required this.primaryButton,
    required this.secondaryButton,
    required this.ghostButton,
    required this.destructiveButton,
    required this.textButton,
    required this.mutedButton,
  });

  factory HShadcnBundle._of(HTokens tokens) {
    final HSurfaceColors c = tokens.colors;
    const shad.Density density = shad.Density(
      baseContainerPadding: HSpace.x3,
      baseGap: HSpace.x2,
      baseContentPadding: HSpace.x3,
    );
    // Der Radius, den jedes Control der Bibliothek nimmt, ist `radiusMd`, und
    // der rechnet sich als `radius * 12 * (baseContentPadding / 16)`. Der
    // Faktor hier ist der Quotient, der ihn auf [HRadius.control] bringt —
    // ausgerechnet und nicht geraten, weil unsere Dichte enger ist als die
    // Vorgabe der Bibliothek und den Radius sonst stillschweigend mitzöge.
    final double densityRadiusScale =
        density.baseContentPadding /
        shad.Density.defaultDensity.baseContentPadding;
    final shad.ThemeData theme = shad.ThemeData(
      colorScheme: _colorScheme(tokens),
      typography: _typography(tokens),
      radius: HRadius.control / (12 * densityRadiusScale),
      scaling: 1,
      density: density,
      iconTheme: const shad.IconThemeProperties(
        xSmall: IconThemeData(size: HSize.glyph - 4),
        small: IconThemeData(size: HSize.glyph - 2),
        medium: IconThemeData(size: HSize.glyph),
        large: IconThemeData(size: HSize.glyph + 4),
      ),
      // Aus: die Rückmeldung auf einen Druck ist in diesem System eine
      // Füllung über [HMotion.press] und kein Schrumpfen der Fläche
      // (`docs/UX.md` 2.2). Der Schalter der Bibliothek schaltet beides
      // zugleich — Haptik und eine 95-%-Skalierung —, und eine Fläche, die
      // unter dem Finger kleiner wird, bewegt sich unter dem lesenden Auge
      // (2.8). Den sichtbaren Druckzustand liefert stattdessen
      // [HShadcnButtonStyle]: er hat für jede Variante eine eigene Füllung.
      enableFeedback: false,
    );
    return HShadcnBundle._(
      theme: theme,
      focusOutline: shad.FocusOutlineTheme(
        // Zwei Pixel Akzent, und der Ring liegt zwei Pixel außerhalb der
        // eigenen Kante — dieselbe Geometrie wie [HFocusRing], damit der Ring
        // der Bibliothek (Eingabefeld, Auswahl) und unserer nicht
        // auseinanderlaufen (`docs/UX.md` 6). Der Vorgabewert der Bibliothek
        // ist drei Pixel Akzent bei halber Deckkraft; halbe Deckkraft ist auf
        // einer Füllung kein Ring mehr.
        align: HFocusRingMetrics.width + HFocusRingMetrics.gap,
        border: Border.all(color: c.accent, width: HFocusRingMetrics.width),
      ),
      textField: shad.TextFieldTheme(
        borderRadius: HRadius.controlRadius,
        border: Border.all(color: c.line),
        filled: true,
        padding: const EdgeInsets.symmetric(
          horizontal: HSpace.x2,
          vertical: HSpace.x1,
        ),
      ),
      checkbox: shad.CheckboxTheme(
        size: HSize.tick,
        gap: HSpace.x2,
        borderRadius: HRadius.badgeRadius,
        activeColor: c.accentFill,
        borderColor: c.lineStrong,
        backgroundColor: const Color(0x00000000),
      ),
      divider: shad.DividerTheme(
        color: c.line,
        thickness: HSize.hairline,
        padding: EdgeInsets.zero,
        indent: 0,
        endIndent: 0,
      ),
      card: shad.CardTheme(
        padding: const EdgeInsets.all(HSpace.panelPadding),
        filled: true,
        fillColor: c.bg1,
        borderColor: c.line,
        borderWidth: HSize.hairline,
        borderRadius: HRadius.cardRadius,
      ),
      outlinedContainer: shad.OutlinedContainerTheme(
        backgroundColor: c.bg1,
        borderColor: c.line,
        borderWidth: HSize.hairline,
        borderRadius: HRadius.cardRadius,
      ),
      badge: shad.BadgeTheme(
        primaryStyle: HShadcnButtonStyle.badge(tokens, c.accent),
        secondaryStyle: HShadcnButtonStyle.badge(tokens, c.fg1),
        outlineStyle: HShadcnButtonStyle.badge(tokens, c.fg1),
        destructiveStyle: HShadcnButtonStyle.badge(
          tokens,
          tokens.state.blocked,
        ),
      ),
      primaryButton: shad.PrimaryButtonTheme(
        decoration: _decorationOf(tokens, HShadcnButtonRole.primary),
        textStyle: _textStyleOf(tokens, HShadcnButtonRole.primary),
        iconTheme: _iconThemeOf(tokens, HShadcnButtonRole.primary),
      ),
      secondaryButton: shad.SecondaryButtonTheme(
        decoration: _decorationOf(tokens, HShadcnButtonRole.secondary),
        textStyle: _textStyleOf(tokens, HShadcnButtonRole.secondary),
        iconTheme: _iconThemeOf(tokens, HShadcnButtonRole.secondary),
      ),
      ghostButton: shad.GhostButtonTheme(
        decoration: _decorationOf(tokens, HShadcnButtonRole.ghost),
        textStyle: _textStyleOf(tokens, HShadcnButtonRole.ghost),
        iconTheme: _iconThemeOf(tokens, HShadcnButtonRole.ghost),
      ),
      destructiveButton: shad.DestructiveButtonTheme(
        decoration: _decorationOf(tokens, HShadcnButtonRole.danger),
        textStyle: _textStyleOf(tokens, HShadcnButtonRole.danger),
        iconTheme: _iconThemeOf(tokens, HShadcnButtonRole.danger),
      ),
      // Die beiden Rollen, die ihr Wort im Ruhezustand in `mutedForeground`
      // schreiben. Das ist bei uns `fg2`, die Stufe, die `docs/UX.md` 6 für
      // wirklich deaktivierte Controls freihält: auf `bg2` misst sie 3,39:1,
      // und ein lebendes Control trägt sein Wort bei 4,5:1. Sie stehen hier,
      // weil der erste gewickelte Kontextmenü- oder Datum-Zeit-Wähler genau
      // solche Knöpfe mitbringt. Die übrigen sechs Rollen der Bibliothek
      // schreiben `foreground`, `accentForeground` oder `cardForeground` —
      // alle drei sind `fg0` — und `mutedForeground` nur am deaktivierten
      // Control, wo er hingehört.
      textButton: shad.TextButtonTheme(textStyle: _liveTextStyle(tokens)),
      mutedButton: shad.MutedButtonTheme(textStyle: _liveTextStyle(tokens)),
    );
  }

  static shad.ColorScheme _colorScheme(HTokens tokens) {
    final HSurfaceColors c = tokens.colors;
    final HStateColors s = tokens.state;
    return shad.ColorScheme(
      brightness: tokens.brightness,
      background: c.bg0,
      foreground: c.fg0,
      card: c.bg1,
      cardForeground: c.fg0,
      popover: c.bg2,
      popoverForeground: c.fg0,
      primary: c.accentFill,
      primaryForeground: c.onAccent,
      secondary: c.bg2,
      secondaryForeground: c.fg0,
      muted: c.bg2,
      mutedForeground: c.fg2,
      accent: c.bg3,
      accentForeground: c.fg0,
      destructive: s.blocked,
      // Das Feld ist in der Bibliothek als veraltet markiert und hat einen
      // Vorgabewert — durchsichtig. Es steht hier trotzdem, weil es Leser hat:
      // `icon.dart` malt ein Icon in Warnfarbe damit, und ein durchsichtiges
      // Icon ist kein Icon. Die Textvariante von `blocked` ist genau die
      // Farbe, in der ein Wort oder ein Zeichen dieser Bedeutung steht
      // (`docs/UX.md` 6).
      // ignore: deprecated_member_use
      destructiveForeground: tokens.stateTextColor(HFlowState.blocked),
      border: c.line,
      input: c.line,
      ring: c.accent,
      chart1: s.held,
      chart2: s.allowed,
      chart3: s.blocked,
      chart4: s.passthroughLlm,
      chart5: s.timedOut,
    );
  }

  static shad.Typography _typography(HTokens tokens) {
    final HTypography t = tokens.typography;
    return shad.Typography(
      sans: HType.uiBase,
      mono: HType.monoBase,
      xSmall: t.ui11,
      small: t.ui12,
      base: t.ui13,
      large: t.ui16,
      xLarge: t.ui20,
      x2Large: t.ui20,
      x3Large: t.ui20,
      x4Large: t.ui20,
      x5Large: t.ui20,
      x6Large: t.ui20,
      x7Large: t.ui20,
      x8Large: t.ui20,
      x9Large: t.ui20,
      thin: const TextStyle(fontWeight: HType.regular),
      light: const TextStyle(fontWeight: HType.regular),
      extraLight: const TextStyle(fontWeight: HType.regular),
      normal: const TextStyle(fontWeight: HType.regular),
      medium: const TextStyle(fontWeight: HType.medium),
      // Die Skala kennt kein 700 und kein 800. `bold` und schwerer fallen
      // deshalb auf 600 zurück, statt eine Schriftstärke einzuführen, die
      // `tokens_test.dart` als unerlaubt kennt.
      semiBold: const TextStyle(fontWeight: HType.semibold),
      bold: const TextStyle(fontWeight: HType.semibold),
      extraBold: const TextStyle(fontWeight: HType.semibold),
      black: const TextStyle(fontWeight: HType.semibold),
      italic: const TextStyle(fontStyle: FontStyle.italic),
      h1: t.ui20.semibold,
      h2: t.ui20.semibold,
      h3: t.ui16.semibold,
      h4: t.ui16.semibold,
      p: t.ui13,
      blockQuote: t.ui13,
      inlineCode: t.mono12.medium,
      lead: t.ui16,
      textLarge: t.ui16.semibold,
      textSmall: t.ui12.medium,
      textMuted: t.ui12,
    );
  }

  static shad.ButtonStatePropertyDelegate<Decoration> _decorationOf(
    HTokens tokens,
    HShadcnButtonRole role,
  ) {
    return (BuildContext context, Set<WidgetState> states, Decoration value) =>
        HShadcnButtonStyle.decoration(tokens, role, states);
  }

  static shad.ButtonStatePropertyDelegate<TextStyle> _textStyleOf(
    HTokens tokens,
    HShadcnButtonRole role,
  ) {
    return (BuildContext context, Set<WidgetState> states, TextStyle value) =>
        HShadcnButtonStyle.textStyle(tokens, role, states);
  }

  /// Hebt ein Wort von der Deaktiviert-Stufe auf die Sekundärstufe.
  ///
  /// Nur am lebenden Control: am deaktivierten bleibt `fg2` richtig.
  static shad.ButtonStatePropertyDelegate<TextStyle> _liveTextStyle(
    HTokens tokens,
  ) {
    return (BuildContext context, Set<WidgetState> states, TextStyle value) =>
        states.contains(WidgetState.disabled)
        ? value.copyWith(color: tokens.colors.fg2)
        : value.copyWith(color: tokens.colors.fg1);
  }

  static shad.ButtonStatePropertyDelegate<IconThemeData> _iconThemeOf(
    HTokens tokens,
    HShadcnButtonRole role,
  ) {
    return (
      BuildContext context,
      Set<WidgetState> states,
      IconThemeData value,
    ) => IconThemeData(
      size: HSize.glyph,
      color: HShadcnButtonStyle.textStyle(tokens, role, states).color,
    );
  }

  /// Das `ThemeData` der Bibliothek.
  final shad.ThemeData theme;

  /// Der Fokusring, wie die Bibliothek ihn zeichnet.
  final shad.FocusOutlineTheme focusOutline;

  /// Das Eingabefeld.
  final shad.TextFieldTheme textField;

  /// Das Kästchen.
  final shad.CheckboxTheme checkbox;

  /// Die Haarlinie.
  final shad.DividerTheme divider;

  /// Die Karte.
  final shad.CardTheme card;

  /// Die umrandete Fläche, auf der Blatt und Modal stehen.
  final shad.OutlinedContainerTheme outlinedContainer;

  /// Die vier Badge-Stile der Bibliothek.
  final shad.BadgeTheme badge;

  /// Der Primärbutton.
  final shad.PrimaryButtonTheme primaryButton;

  /// Der Sekundärbutton.
  final shad.SecondaryButtonTheme secondaryButton;

  /// Der stille Button.
  final shad.GhostButtonTheme ghostButton;

  /// Der Button, der blockiert.
  final shad.DestructiveButtonTheme destructiveButton;

  /// Der Textbutton der Bibliothek.
  final shad.TextButtonTheme textButton;

  /// Der stille Button der Bibliothek.
  final shad.MutedButtonTheme mutedButton;
}

/// Die vier Rollen, die ein Button in diesem System spielen kann, aus der
/// Sicht der Stil-Ableitung.
///
/// Dieselben vier wie `HButtonVariant`; getrennt geführt, weil die Ableitung
/// unter den Widgets liegt und ein Widget nicht von seinem eigenen Stil
/// abhängen soll.
enum HShadcnButtonRole {
  /// Die eine Handlung, mit der Akzentfüllung.
  primary,

  /// Die zweite, gleichrangige Handlung.
  secondary,

  /// Die stille Handlung ohne eigene Fläche.
  ghost,

  /// Die Handlung, die blockiert, im Ton eines blockierten Flows.
  danger,
}

/// Die Stil-Ableitung, die aus [HTokens] und einer [HShadcnButtonRole] alles
/// macht, was die Bibliothek von einem Buttonstil verlangt.
///
/// Sie steht an genau einer Stelle und wird von zwei Seiten gelesen: von den
/// `ButtonTheme`-Einträgen des Bündels, damit **jeder** Button der Bibliothek
/// in unserer Palette malt, und von `HButton` selbst.
///
/// Drei Dinge, die die Bibliothek nicht mitbringt, stehen hier:
///
/// 1. **Ein sichtbarer gedrückter Zustand.** Die Vorgaben der Bibliothek
///    kennen `hovered` und `disabled`, aber keinen Druck; die einzige
///    Rückmeldung hängt an `enableFeedback`, und der ist auf dem Desktop aus.
///    Ein Control, das auf einen Klick nichts tut, fühlt sich kaputt an, also
///    hat hier jede Rolle eine eigene Druckfüllung.
/// 2. **Eine Untergrenze für Lesbarkeit.** Der Destructive-Button der
///    Bibliothek malt im hellen Thema festes Weiß auf ein Rot mit halber
///    Deckkraft — 1,97:1, mit einem Kommentar im Quelltext, der das einräumt.
///    Hier trägt er die Textvariante der Zustandsfarbe und erreicht auf jeder
///    seiner drei Füllungen 4,5:1 (`docs/UX.md` 6).
/// 3. **Deaktiviert heißt sichtbar deaktiviert.** Die Bibliothek füllt einen
///    deaktivierten Primärbutton mit `mutedForeground`, also mit einer
///    Textfarbe. Hier bleibt die Fläche stehen und das Wort geht auf `fg2`.
abstract final class HShadcnButtonStyle {
  /// Der Stil einer Rolle, fertig für `Button(style: ...)`.
  static shad.AbstractButtonStyle of(
    HTokens tokens,
    HShadcnButtonRole role, {
    EdgeInsetsGeometry padding = const EdgeInsets.symmetric(
      horizontal: HSpace.x3,
      vertical: HSpace.x1,
    ),
    Color? fill,
  }) {
    return shad.ButtonVariance(
      decoration: (BuildContext context, Set<WidgetState> states) =>
          decoration(tokens, role, states, fill: fill),
      textStyle: (BuildContext context, Set<WidgetState> states) =>
          textStyle(tokens, role, states),
      iconTheme: (BuildContext context, Set<WidgetState> states) =>
          IconThemeData(
            size: HSize.glyph,
            color: textStyle(tokens, role, states).color,
          ),
      padding: (BuildContext context, Set<WidgetState> states) => padding,
      mouseCursor: (BuildContext context, Set<WidgetState> states) =>
          states.contains(WidgetState.disabled)
          ? MouseCursor.defer
          : SystemMouseCursors.click,
      margin: (BuildContext context, Set<WidgetState> states) =>
          EdgeInsets.zero,
    );
  }

  /// Der Stil eines Badge: dieselbe Ableitung, nur enger und kleiner gesetzt.
  ///
  /// Fläche und Beschriftung werden getrennt geführt. [color] ist der Ton, aus
  /// dem die Tönung gemacht wird; [background] übergeht die Fläche, wo ein
  /// Badge neutral stehen soll (`docs/UX.md` 3.3, Regel 4), und [textColor]
  /// die Beschriftung, wo sie nicht die Textvariante von [color] ist. Eine
  /// Zustands- oder Methodenfarbe ist auf 3:1 geklemmt und trägt damit keinen
  /// Text; ihre Textvariante erreicht 4,5:1 auch auf der eigenen Tönung
  /// (`docs/UX.md` 6).
  static shad.AbstractButtonStyle badge(
    HTokens tokens,
    Color color, {
    Color? background,
    Color? textColor,
    bool mono = false,
  }) {
    final Color label = textColor ?? tokens.stateTextOf(color);
    final TextStyle base = mono
        ? tokens.typography.mono11
        : tokens.typography.ui11;
    return shad.ButtonVariance(
      decoration: (BuildContext context, Set<WidgetState> states) =>
          BoxDecoration(
            color: background ?? _step(color, states, rest: HColors.tintAlpha),
            borderRadius: HRadius.badgeRadius,
          ),
      textStyle: (BuildContext context, Set<WidgetState> states) =>
          base.medium.tinted(
            states.contains(WidgetState.disabled) ? tokens.colors.fg2 : label,
          ),
      iconTheme: (BuildContext context, Set<WidgetState> states) =>
          IconThemeData(size: HSize.glyph - 4, color: label),
      padding: (BuildContext context, Set<WidgetState> states) =>
          const EdgeInsets.symmetric(horizontal: HSpace.x2),
      mouseCursor: (BuildContext context, Set<WidgetState> states) =>
          SystemMouseCursors.click,
      margin: (BuildContext context, Set<WidgetState> states) =>
          EdgeInsets.zero,
    );
  }

  /// Ein Stil ohne eigene Fläche, für ein Control, dessen Marke die Fläche
  /// selbst ist — das Kästchen etwa.
  static shad.AbstractButtonStyle plain(HTokens tokens) {
    return shad.ButtonVariance(
      decoration: (BuildContext context, Set<WidgetState> states) =>
          const BoxDecoration(color: Color(0x00000000)),
      textStyle: (BuildContext context, Set<WidgetState> states) =>
          tokens.typography.ui13.tinted(
            states.contains(WidgetState.disabled)
                ? tokens.colors.fg2
                : tokens.colors.fg0,
          ),
      iconTheme: (BuildContext context, Set<WidgetState> states) =>
          IconThemeData(size: HSize.glyph, color: tokens.colors.fg1),
      padding: (BuildContext context, Set<WidgetState> states) =>
          EdgeInsets.zero,
      mouseCursor: (BuildContext context, Set<WidgetState> states) =>
          states.contains(WidgetState.disabled)
          ? MouseCursor.defer
          : SystemMouseCursors.click,
      margin: (BuildContext context, Set<WidgetState> states) =>
          EdgeInsets.zero,
    );
  }

  /// Der Stil eines Segments in `HSegmented` oder `HChoiceChips`.
  ///
  /// Das gewählte Segment trägt die höchste Fläche und den Primärtext, nie die
  /// Akzentfüllung: der Akzent gehört der einen Handlung des Bildschirms, und
  /// ein Formular mit vier gefüllten Segmenten hätte fünf (`docs/UX.md` 3.1).
  /// Rahmen und Ecke kommen von der Fläche, in der die Segmente stehen, nicht
  /// vom Segment selbst.
  static shad.AbstractButtonStyle segment(
    HTokens tokens, {
    required bool selected,
    Color? fill,
  }) {
    return shad.ButtonVariance(
      decoration: (BuildContext context, Set<WidgetState> states) =>
          BoxDecoration(
            color: fill ?? segmentFill(tokens, states, selected: selected),
          ),
      textStyle: (BuildContext context, Set<WidgetState> states) => tokens
          .typography
          .ui12
          .medium
          .tinted(segmentTextColor(tokens, states, selected: selected)),
      iconTheme: (BuildContext context, Set<WidgetState> states) =>
          IconThemeData(size: HSize.glyph - 4, color: tokens.colors.fg1),
      padding: (BuildContext context, Set<WidgetState> states) =>
          const EdgeInsets.symmetric(horizontal: HSpace.x2),
      mouseCursor: (BuildContext context, Set<WidgetState> states) =>
          states.contains(WidgetState.disabled)
          ? MouseCursor.defer
          : SystemMouseCursors.click,
      margin: (BuildContext context, Set<WidgetState> states) =>
          EdgeInsets.zero,
    );
  }

  /// Die Farbe, in der [segment] sein Wort in [states] schreibt.
  ///
  /// Getrennt geführt, damit ein Test sie ohne `BuildContext` nachrechnen
  /// kann: die Füllungen dieses Systems werden gegen ihre Wörter geprüft, und
  /// eine Prüfung, die einen Baum aufbauen muss, prüft am Ende den Baum.
  static Color segmentTextColor(
    HTokens tokens,
    Set<WidgetState> states, {
    required bool selected,
  }) {
    // Deaktiviert heißt sichtbar deaktiviert: `fg2` ist die Stufe, die
    // `docs/UX.md` 6 dafür freihält.
    if (states.contains(WidgetState.disabled)) {
      return tokens.colors.fg2;
    }
    return selected ? tokens.colors.fg0 : tokens.colors.fg1;
  }

  /// Die Füllung, die [segment] in [states] nähme.
  ///
  /// Drei Stufen je Zustand, damit der Druck auch am gewählten Segment zu
  /// sehen ist: die Bibliothek kennt für ihn keine Fläche, und ein Control,
  /// das auf einen Klick nichts tut, fühlt sich kaputt an.
  static Color segmentFill(
    HTokens tokens,
    Set<WidgetState> states, {
    required bool selected,
  }) {
    if (states.contains(WidgetState.disabled)) {
      return selected ? tokens.colors.bg3 : const Color(0x00000000);
    }
    if (states.contains(WidgetState.pressed)) {
      return selected ? tokens.colors.lineStrong : tokens.colors.bg3;
    }
    if (states.contains(WidgetState.hovered)) {
      return selected ? tokens.colors.bg3 : tokens.colors.bg2;
    }
    return selected ? tokens.colors.bg3 : const Color(0x00000000);
  }

  /// Die Füllung, die [badge] in [states] nähme.
  static Color badgeFill(
    Color color,
    Set<WidgetState> states, {
    Color? background,
  }) => background ?? _step(color, states, rest: HColors.tintAlpha);

  /// Die Füllung und der Rahmen einer Rolle in [states].
  ///
  /// [fill] übergeht die Füllung dieses Frames. `HButton` reicht dort den
  /// Zwischenwert von `HAnimatedFill` hinein: die Tastenfüllung behält so ihre
  /// 120 ms aus [HMotion.press], auch wenn die Plattform `disableAnimations`
  /// meldet — die Animationsprimitive der Bibliothek baut ihren Controller
  /// ohne `animationBehavior` und kollabierte dort auf fünf Prozent
  /// (`docs/UX.md` 2.10).
  static Decoration decoration(
    HTokens tokens,
    HShadcnButtonRole role,
    Set<WidgetState> states, {
    Color? fill,
  }) {
    final HSurfaceColors c = tokens.colors;
    switch (role) {
      case HShadcnButtonRole.primary:
        final Color base = c.accentFill;
        // Deaktiviert steht der Primärbutton auf der Sekundärfläche und nicht
        // auf der Akzentfüllung: die Füllung ist die Zusage, dass hier die
        // eine Handlung des Bildschirms sitzt, und ein Control, das nichts
        // tut, darf sie nicht geben (`docs/UX.md` 3.1 und 3.3).
        final Color background =
            fill ??
            (states.contains(WidgetState.disabled)
                ? c.bg2
                : states.contains(WidgetState.pressed)
                ? HColorDerivation.darken(base, 0.06)
                : states.contains(WidgetState.hovered)
                ? HColorDerivation.darken(
                    base,
                    tokens.brightness == Brightness.dark ? -0.04 : 0.04,
                  )
                : base);
        return BoxDecoration(
          color: background,
          borderRadius: HRadius.controlRadius,
          border: Border.all(
            color: states.contains(WidgetState.disabled) ? c.line : background,
          ),
        );
      case HShadcnButtonRole.secondary:
        // Drei Flächen, nicht zwei: die neutrale Leiter endet bei `bg3`, und
        // der Druck braucht eine Stufe darüber, sonst sind Hover und Druck
        // dieselbe Farbe und ein Klick sieht aus wie ein Überfahren. Die
        // einzige neutrale Farbe über `bg3` ist [HSurfaceColors.lineStrong];
        // sie tritt in beiden Themen deutlich von `bg3` weg, und `fg0` steht
        // darauf weit über 4,5:1 (`docs/UX.md` 2.2 und 6).
        final Color background =
            fill ??
            (states.contains(WidgetState.disabled)
                ? c.bg2
                : states.contains(WidgetState.pressed)
                ? c.lineStrong
                : states.contains(WidgetState.hovered)
                ? c.bg3
                : c.bg2);
        return BoxDecoration(
          color: background,
          borderRadius: HRadius.controlRadius,
          border: Border.all(color: c.line),
        );
      case HShadcnButtonRole.ghost:
        final Color background =
            fill ??
            (states.contains(WidgetState.disabled)
                ? const Color(0x00000000)
                : states.contains(WidgetState.pressed)
                ? c.bg3
                : states.contains(WidgetState.hovered)
                ? c.bg2
                : const Color(0x00000000));
        return BoxDecoration(
          color: background,
          borderRadius: HRadius.controlRadius,
          border: Border.all(color: const Color(0x00000000)),
        );
      case HShadcnButtonRole.danger:
        final Color blocked = tokens.state.blocked;
        // Deaktiviert verliert auch dieser Button seinen Ton. Er behielte
        // sonst die Tönung der Zustandsfarbe, und `fg2` darauf misst über
        // `bg3` 2,68:1 — unter jeder Grenze, die dieses System kennt. Ein
        // totes Control sieht aus wie ein totes, gleich welches es war.
        final bool disabled = states.contains(WidgetState.disabled);
        return BoxDecoration(
          color:
              fill ??
              (disabled
                  ? c.bg2
                  : _step(blocked, states, rest: HColors.tintAlpha)),
          borderRadius: HRadius.controlRadius,
          border: Border.all(
            color: disabled ? c.line : HColorDerivation.fade(blocked, 0.4),
          ),
        );
    }
  }

  /// Die Füllung, die [decoration] in [states] ohne Übergang nähme.
  ///
  /// `HButton` fragt danach und reicht sie an `HAnimatedFill`; die Ableitung
  /// bleibt damit an einer Stelle.
  static Color fillOf(
    HTokens tokens,
    HShadcnButtonRole role,
    Set<WidgetState> states,
  ) {
    final Decoration painted = decoration(tokens, role, states);
    return painted is BoxDecoration
        ? (painted.color ?? const Color(0x00000000))
        : const Color(0x00000000);
  }

  /// Die Schrift einer Rolle in [states].
  static TextStyle textStyle(
    HTokens tokens,
    HShadcnButtonRole role,
    Set<WidgetState> states,
  ) {
    final HSurfaceColors c = tokens.colors;
    if (states.contains(WidgetState.disabled)) {
      return tokens.typography.ui13.medium.tinted(c.fg2);
    }
    final Color color = switch (role) {
      HShadcnButtonRole.primary => c.onAccent,
      HShadcnButtonRole.secondary => c.fg0,
      HShadcnButtonRole.ghost => c.fg1,
      // Die Textvariante der Zustandsfarbe, nicht die Fläche: auf der eigenen
      // Tönung misst `blocked` sonst rund 2,5:1 (`docs/UX.md` 6).
      HShadcnButtonRole.danger => tokens.stateTextColor(HFlowState.blocked),
    };
    return tokens.typography.ui13.medium.tinted(color);
  }

  /// Eine Tönung, die mit Hover und Druck eine Stufe steigt.
  static Color _step(
    Color color,
    Set<WidgetState> states, {
    required double rest,
  }) {
    if (states.contains(WidgetState.pressed)) {
      return color.withValues(alpha: HColors.fillPressedAlpha);
    }
    if (states.contains(WidgetState.hovered)) {
      return color.withValues(alpha: HColors.fillHoverAlpha);
    }
    return HColorDerivation.tint(color, rest);
  }
}

/// Die Maße des Fokusrings, an einer Stelle.
///
/// Sie stehen hier und nicht in `HFocusRing`, weil zwei Ringe dieselben Maße
/// brauchen: unserer, der seinen Platz reserviert, und der der Bibliothek, der
/// über die eigene Kante hinaus zeichnet. Ohne eine gemeinsame Quelle liefen
/// sie beim ersten Nachziehen auseinander (`docs/UX.md` 6).
abstract final class HFocusRingMetrics {
  /// Die Breite des Rings. Zwei Pixel, wie `docs/UX.md` 6 sie nennt.
  static const double width = 2;

  /// Der Abstand zwischen dem Control und dem Ring.
  static const double gap = 2;
}
