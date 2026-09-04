/// A titled section that folds. Specified for `packages/ui` (HUM-020
/// Schritt 2, `HCollapsible`); it lives here until that package is touched
/// again (handoff). No user string inside: the title comes in localised.
///
/// Die Kopfzeile ist ein Fokusstopp und faltet auf `Enter` und `Leertaste`,
/// nicht nur unter dem Zeiger: jeder Zeigerweg hat eine Taste
/// (`docs/UX.md` 5.1). Der Ring läuft als `HFocusRing.inline` auf ihrer
/// Kante, wie bei jeder anderen Zeile, die von Rand zu Rand läuft.
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// A section with a clickable header and a body that folds away.
class HCollapsible extends StatefulWidget {
  /// Creates a section titled [title] around [child].
  const HCollapsible({
    required this.title,
    required this.child,
    this.initiallyOpen = true,
    this.trailing,
    this.semanticsLabel,
    super.key,
  });

  /// The header text, localised.
  final String title;

  /// The body.
  final Widget child;

  /// Whether the body shows at first.
  final bool initiallyOpen;

  /// Something at the right end of the header, for example a count.
  final Widget? trailing;

  /// Screen-reader label of the header; [title] when null.
  final String? semanticsLabel;

  @override
  State<HCollapsible> createState() => _HCollapsibleState();
}

class _HCollapsibleState extends State<HCollapsible>
    with SingleTickerProviderStateMixin {
  late bool _open = widget.initiallyOpen;
  bool _focused = false;
  // Ohne `AnimationBehavior.preserve`: das Falten erklärt eine Änderung, es
  // sichert keine. Wer Animationen abgeschaltet hat, will die Sektion sofort
  // offen sehen, und die Kürzung der Plattform ist genau das (2.10).
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
    value: _open ? 1 : 0,
  );

  // Ein Feld, kein Ausdruck in `build`: `CurvedAnimation` hängt in seinem
  // Konstruktor einen Statuslistener an den Controller, den niemand wieder
  // abnimmt. In `build` wüchse die Liste der Listener mit jedem Rebuild, und
  // ein Klappabschnitt baut bei jedem Fokuswechsel neu (`docs/UX.md` 7).
  late final CurvedAnimation _fold = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
    reverseCurve: HMotion.exit,
  );

  @override
  void dispose() {
    _fold.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _toggle() {
    setState(() => _open = !_open);
    if (_open) {
      _controller.forward();
    } else {
      _controller.reverse();
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Semantics(
          button: true,
          expanded: _open,
          label: widget.semanticsLabel ?? widget.title,
          excludeSemantics: true,
          child: FocusableActionDetector(
            mouseCursor: SystemMouseCursors.click,
            onFocusChange: (bool value) => setState(() => _focused = value),
            actions: <Type, Action<Intent>>{
              ActivateIntent: CallbackAction<ActivateIntent>(
                onInvoke: (ActivateIntent intent) {
                  _toggle();
                  return null;
                },
              ),
            },
            child: HFocusRing.inline(
              visible: _focused,
              radius: tokens.radii.control,
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: _toggle,
                // Eine Mindesthöhe, keine Höhe: bei doppelter Textskalierung
                // misst die Zeile in `ui12` 32 px gegen einen 28-px-Kasten,
                // und eine feste Höhe schnitte sie still ab
                // (`docs/UX.md` 6).
                child: ConstrainedBox(
                  constraints: const BoxConstraints(minHeight: HSize.hitMin),
                  child: Row(
                    children: <Widget>[
                      RotationTransition(
                        turns: Tween<double>(
                          begin: 0,
                          end: 0.25,
                        ).animate(_controller),
                        child: HGlyphIcon(
                          HGlyph.chevronRight,
                          size: 14,
                          color: tokens.colors.fg2,
                        ),
                      ),
                      SizedBox(width: tokens.spacing.x1),
                      Expanded(
                        child: Text(
                          widget.title,
                          style: tokens.typography.ui12.semibold.tinted(
                            tokens.colors.fg1,
                          ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      if (widget.trailing != null) widget.trailing!,
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
        ClipRect(
          child: SizeTransition(
            sizeFactor: _fold,
            alignment: Alignment.topCenter,
            child: Padding(
              padding: EdgeInsets.only(
                left: tokens.spacing.x5,
                bottom: tokens.spacing.x2,
              ),
              child: widget.child,
            ),
          ),
        ),
      ],
    );
  }
}
