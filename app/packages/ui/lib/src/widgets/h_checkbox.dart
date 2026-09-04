import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../theme/h_theme.dart';
import '../theme/shadcn_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_control.dart';

/// Ein Kästchen, das an oder aus ist, mit dem Satz daneben, der sagt, was das
/// Anhaken kostet.
///
/// Der [hint] ist keine Zierde: wo eine Einstellung Sicherheit kostet, sagt
/// der Text das in einem Satz (`backlog/CONVENTIONS.md` 4.13). Das Kästchen
/// ist ein Fokusstopp, mindestens [HSize.hitMin] hoch, und zeigt den Fokus als
/// Ring außerhalb (`docs/UX.md` 5.1 und 6).
///
/// Das Kästchen selbst ist `Checkbox` aus `shadcn_flutter`, mitsamt ihrem
/// gezeichneten Haken und dessen Anlauf; Größe, Ecke und Farben kommen aus dem
/// `CheckboxTheme`, das `HTheme` aus den Token füllt. Verhalten und Fokus
/// bleiben bei [HControl], weil die Komponente der Bibliothek keinen
/// `FocusNode` von außen annimmt — ein Bildschirm könnte die Reihenfolge
/// seiner Fokusstopps dann nicht mehr selbst bestimmen. Deshalb steht die
/// Komponente hier hinter `ExcludeFocus` und `IgnorePointer`: sie zeichnet,
/// sie entscheidet nicht.
///
/// Ein Kästchen mit `enabled: false` sieht auch so aus: Beschriftung, Hinweis
/// und die Fläche des Kästchens stehen in `fg2`, der Stufe, die `docs/UX.md` 6
/// für wirklich deaktivierte Controls freihält, und die Fläche trägt nicht den
/// Akzent — der gehört dem, was man anfassen kann (3.3). Der Haken selbst
/// bleibt `onAccent`: er kommt aus der Komponente der Bibliothek und ist dort
/// fest an `primaryForeground` gebunden. Auf `fg2` misst er dunkel 3,90:1 —
/// über der Flächengrenze, unter der Textgrenze, und ein Haken ist eine
/// Grafik.
class HCheckbox extends StatelessWidget {
  /// Creates a checkbox.
  const HCheckbox({
    required this.label,
    required this.value,
    required this.onChanged,
    this.hint,
    this.enabled = true,
    this.focusNode,
    super.key,
  });

  /// Die Beschriftung neben dem Kästchen, vom Aufrufer bereits übersetzt.
  final String label;

  /// Ob es angehakt ist.
  final bool value;

  /// Wird mit dem neuen Wert gerufen.
  final ValueChanged<bool> onChanged;

  /// Was das Anhaken bedeutet, wenn das nicht offensichtlich ist.
  final String? hint;

  /// Falsch für etwas, das niemand ändern darf.
  final bool enabled;

  /// Ein von außen gehaltener Fokusknoten.
  final FocusNode? focusNode;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? hint = this.hint;
    // Deaktiviert heißt sichtbar deaktiviert: `fg2` ist genau dafür da.
    final Color labelColor = enabled ? tokens.colors.fg0 : tokens.colors.fg2;
    final Color hintColor = enabled ? tokens.colors.fg1 : tokens.colors.fg2;
    return Semantics(
      checked: value,
      enabled: enabled,
      label: label,
      excludeSemantics: true,
      child: HControl(
        enabled: enabled,
        onPressed: enabled ? () => onChanged(!value) : null,
        focusNode: focusNode,
        radius: tokens.radii.control,
        fill: (HTokens tokens, Set<WidgetState> states) =>
            const Color(0x00000000),
        style: (HTokens tokens, Color fill) => HShadcnButtonStyle.plain(tokens),
        builder: (BuildContext context, Set<WidgetState> states, Color fill) =>
            ConstrainedBox(
              constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Padding(
                    padding: EdgeInsets.only(top: tokens.spacing.x2),
                    child: ExcludeFocus(
                      child: IgnorePointer(
                        child: shad.Checkbox(
                          state: value
                              ? shad.CheckboxState.checked
                              : shad.CheckboxState.unchecked,
                          onChanged: null,
                          // **Immer `true`, auch am toten Kästchen.** Der
                          // Wert entscheidet in der Komponente nichts als die
                          // Rahmenfarbe: ohne ihn rechnet sie
                          // `widget.enabled ?? widget.onChanged != null`, und
                          // weil hier niemand entscheidet — `IgnorePointer`
                          // und `ExcludeFocus` liegen darum —, käme immer
                          // `false` heraus. Dann malte sie den Zweig
                          // `!enabled ? colorScheme.muted` und damit **jedes**
                          // nicht angehakte Kästchen in `bg2`, also in der
                          // Farbe eines toten. Deaktiviert heißt hier `fg2`,
                          // und das steht unten in [borderColor].
                          enabled: true,
                          // Die Farben stehen hier und nicht nur im
                          // `CheckboxTheme`, weil ein abgeschaltetes Kästchen
                          // `fg2` trägt und nicht den Akzent: der gehört dem,
                          // was man anfassen kann (`docs/UX.md` 3.3).
                          activeColor: enabled
                              ? tokens.colors.accentFill
                              : tokens.colors.fg2,
                          borderColor: enabled
                              ? tokens.colors.lineStrong
                              : tokens.colors.fg2,
                          backgroundColor: const Color(0x00000000),
                          size: HSize.tick,
                          borderRadius: HRadius.badgeRadius,
                        ),
                      ),
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Padding(
                          padding: EdgeInsets.only(top: tokens.spacing.x1),
                          child: Text(
                            label,
                            style: tokens.typography.ui13.tinted(labelColor),
                          ),
                        ),
                        if (hint != null)
                          Text(
                            hint,
                            style: tokens.typography.ui12.tinted(hintColor),
                          ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
      ),
    );
  }
}
