/// Die Fläche, auf der jede Rumpf-Ansicht steht.
///
/// Eine Datei, weil alle vier dieselben zwei Eigenschaften brauchen und eine
/// Kopie davon sofort auseinanderliefe: eine bekannte Zeilenhöhe, damit
/// Scrollen bei zehntausend Zeilen konstant kostet statt linear
/// (`docs/UX.md` 7), und waagerechtes Scrollen statt Umbruch, damit kein
/// Versatz verrutscht (3.2).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// Die Breite eines Monospace-Zeichens in [style], gemessen statt geschätzt.
///
/// `HSize.monoAdvance` ist ausdrücklich Illustration und keine Norm
/// (`docs/UX.md` 3.2): der Vorschub hängt an der installierten Schrift, und
/// eine geschätzte Breite ist in einer Ansicht, die waagerecht scrollt, der
/// Unterschied zwischen „alles erreichbar" und „der Rest ist weg". Gemessen
/// wird einmal je Schriftgröße und Skalierung, in `build` und nie in einem
/// Layout-Callback (`docs/UX.md` 7).
double monoAdvance(BuildContext context, TextStyle style) {
  final TextScaler scaler = MediaQuery.textScalerOf(context);
  final double size = style.fontSize ?? 12;
  final (double, double) key = (size, scaler.scale(size));
  final double? known = _advances[key];
  if (known != null) {
    return known;
  }
  final TextPainter painter = TextPainter(
    text: TextSpan(text: '0' * 16, style: style),
    textDirection: TextDirection.ltr,
    textScaler: scaler,
  )..layout();
  final double advance = painter.width / 16;
  painter.dispose();
  _advances[key] = advance;
  return advance;
}

final Map<(double, double), double> _advances = <(double, double), double>{};

/// Die Höhe einer Zeile in einer Rumpf-Ansicht, mit der Textskalierung.
///
/// Die dichteste der drei Dichten aus `docs/UX.md` 3.2 und wie die anderen
/// beiden eine Mindesthöhe: eine feste Höhe schluckte den Überlauf still
/// (Abschnitt 6).
double rowExtent(BuildContext context) =>
    MediaQuery.textScalerOf(context).scale(HSize.rowBody);

/// Eine scrollende Liste fester Zeilenhöhe hinter einem waagerechten Schieber.
class BodySurface extends StatefulWidget {
  /// Creates the surface.
  const BodySurface({
    required this.contentWidth,
    required this.itemCount,
    required this.itemBuilder,
    this.focusRow,
    this.focusOffset,
    super.key,
  });

  /// Wie breit der Inhalt ist; darunter füllt die Fläche.
  final double contentWidth;

  /// Wie viele Zeilen.
  final int itemCount;

  /// Baut Zeile [index].
  final Widget Function(BuildContext context, int index) itemBuilder;

  /// Die Zeile, zu der gesprungen werden soll, sobald sie sich ändert.
  ///
  /// Das ist der Weg vom Chip zur Fundstelle: der Chip nennt einen Fund, die
  /// Ansicht rechnet ihn in eine Zeile um, und diese Fläche springt dorthin.
  /// Ein Sprung, kein Gleiten -- die Bewegung erklärte hier nichts, und Text,
  /// den jemand liest, bewegt sich nie (`docs/UX.md` 2.9).
  final int? focusRow;

  /// Wie weit rechts die Fundstelle in dieser Zeile steht, in Pixeln.
  ///
  /// Ohne sie bliebe ein Fund weit rechts nach dem Sprung unsichtbar, und
  /// nichts sagte es: die Zeile stimmte, die Spalte nicht.
  final double? focusOffset;

  @override
  State<BodySurface> createState() => _BodySurfaceState();
}

class _BodySurfaceState extends State<BodySurface> {
  final ScrollController _horizontal = ScrollController();
  final ScrollController _vertical = ScrollController();

  @override
  void initState() {
    super.initState();
    if (widget.focusRow != null) {
      _scheduleJump(widget.focusRow!);
    }
  }

  @override
  void didUpdateWidget(BodySurface oldWidget) {
    super.didUpdateWidget(oldWidget);
    final int? row = widget.focusRow;
    if (row != null && row != oldWidget.focusRow) {
      _scheduleJump(row);
    }
  }

  /// Springt nach dem nächsten Frame; vorher kennt die Liste ihre Grenzen
  /// nicht.
  void _scheduleJump(int row) {
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (!mounted) {
        return;
      }
      if (_vertical.hasClients) {
        final double target = row * rowExtent(context);
        _vertical.jumpTo(
          target.clamp(0, _vertical.position.maxScrollExtent).toDouble(),
        );
      }
      final double? column = widget.focusOffset;
      if (column != null && _horizontal.hasClients) {
        // Ein Drittel Fenster Vorlauf, damit die Stelle nicht am linken Rand
        // klebt und der Zusammenhang links davon lesbar bleibt.
        final double viewport = _horizontal.position.viewportDimension;
        _horizontal.jumpTo(
          (column - viewport / 3)
              .clamp(0, _horizontal.position.maxScrollExtent)
              .toDouble(),
        );
      }
    });
  }

  @override
  void dispose() {
    _horizontal.dispose();
    _vertical.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Die Zeilenhöhe wächst mit der Textskalierung; eine feste Höhe schluckte
    // den Überlauf still (`docs/UX.md` 6).
    final double extent = rowExtent(context);
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double width = widget.contentWidth < constraints.maxWidth
            ? constraints.maxWidth
            : widget.contentWidth;
        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          controller: _horizontal,
          child: SizedBox(
            width: width,
            height: constraints.maxHeight,
            child: ListView.builder(
              controller: _vertical,
              itemExtent: extent,
              itemCount: widget.itemCount,
              itemBuilder: widget.itemBuilder,
            ),
          ),
        );
      },
    );
  }
}
