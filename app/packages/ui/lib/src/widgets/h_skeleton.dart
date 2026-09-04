import 'dart:async';

import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_hairline.dart';

/// Das Skelett der Zeilen, die gleich kommen.
///
/// Haarlinien in der Höhe einer der drei Zieldichten, in `fg2`, ohne Bewegung.
/// Es sagt, wie viel gleich kommt und wo es stehen wird; ein Spinner sagt nur,
/// dass etwas läuft, und es gibt in diesem Programm keinen
/// (`docs/UX.md` 2.11).
///
/// Beim Eintreffen wird nichts verschoben: das Skelett wird an derselben
/// Stelle durch die Zeile ersetzt, die es beschrieben hat, in einem Frame.
/// Deshalb steht hier dieselbe Höhe wie in der Liste dahinter —
/// [HSize.row] (36), [HSize.rowHistory] (28) oder [HSize.rowBody] (24).
class HSkeleton extends StatelessWidget {
  /// Zeichnet [rows] Zeilen der Höhe [rowHeight].
  const HSkeleton({
    required this.rows,
    this.rowHeight = HSize.rowHistory,
    super.key,
  });

  /// Wie viele Zeilen erwartet werden.
  final int rows;

  /// Die Höhe einer Zeile, eine der drei Dichten aus `docs/UX.md` 3.2.
  final double rowHeight;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return ExcludeSemantics(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          for (int i = 0; i < rows; i++)
            SizedBox(
              height: rowHeight,
              child: Padding(
                padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: FractionallySizedBox(
                    // Die Linien sind nicht alle gleich lang: Zeilen sind es
                    // auch nicht, und ein Block gleicher Balken liest sich als
                    // Tabelle, die schon da ist.
                    widthFactor: i.isEven ? 0.62 : 0.44,
                    child: HHairline(color: tokens.colors.fg2),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// Warten mit den beiden Schwellen aus `docs/UX.md` 2.11.
///
/// Unter [HMotion.waitVisible] (150 ms) passiert nichts: eine Anzeige, die
/// kürzer sichtbar ist als eine Reaktionszeit, wird als Flackern gelesen.
/// Danach steht [skeleton] an der Stelle, an der die Antwort stehen wird, und
/// bleibt mindestens [HMotion.waitMinVisible] (400 ms) — sonst erzeugt eine
/// Antwort kurz nach der Schwelle genau das Flackern, das die Schwelle
/// verhindern soll.
///
/// Die beiden Schwellen stehen hier und nicht in jedem Screen, damit sie nicht
/// in jedem Screen neu erfunden werden (`docs/UX.md` 9, Punkt 19).
class HWait extends StatefulWidget {
  /// Zeigt [child], oder [skeleton], sobald das Warten lange genug dauert.
  const HWait({
    required this.loading,
    required this.skeleton,
    required this.child,
    super.key,
  });

  /// Ob die Antwort noch unterwegs ist.
  final bool loading;

  /// Was an der Stelle der Antwort steht, solange sie fehlt.
  final Widget skeleton;

  /// Die Antwort.
  final Widget child;

  @override
  State<HWait> createState() => _HWaitState();
}

class _HWaitState extends State<HWait> {
  Timer? _appear;
  Timer? _linger;
  bool _showSkeleton = false;

  @override
  void initState() {
    super.initState();
    if (widget.loading) {
      _startWaiting();
    }
  }

  @override
  void didUpdateWidget(HWait oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.loading == oldWidget.loading) {
      return;
    }
    if (widget.loading) {
      _startWaiting();
    } else {
      _stopWaiting();
    }
  }

  void _startWaiting() {
    _appear?.cancel();
    _appear = Timer(HMotion.waitVisible, () {
      if (!mounted || !widget.loading) {
        return;
      }
      setState(() => _showSkeleton = true);
      _linger?.cancel();
      _linger = Timer(HMotion.waitMinVisible, () {
        if (mounted && !widget.loading) {
          setState(() => _showSkeleton = false);
        }
      });
    });
  }

  void _stopWaiting() {
    _appear?.cancel();
    _appear = null;
    if (!_showSkeleton) {
      return;
    }
    if (_linger?.isActive ?? false) {
      // Das Skelett hat seine Mindeststandzeit noch nicht abgesessen; der
      // Timer nimmt es weg.
      return;
    }
    setState(() => _showSkeleton = false);
  }

  @override
  void dispose() {
    _appear?.cancel();
    _linger?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      _showSkeleton ? widget.skeleton : widget.child;
}
