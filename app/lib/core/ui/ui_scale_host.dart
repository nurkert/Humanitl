/// Der Wirt der Skalierung (HUM-356): ändert die logische Pixeldichte für
/// alles unter ihm.
///
/// **Warum kein `Transform`.** Ein vergrößertes Bild des Baums bei gleichem
/// Layout schnitte bei 200 % die rechte und die untere Hälfte ab, oder es
/// müsste schrumpfen, bevor es wächst. Der Wirt legt seinen Inhalt stattdessen
/// in `Fenster / s` logischen Pixeln an ([RenderUiScale]) und meldet das
/// darunter über die `MediaQuery`: Größe und Ränder geteilt durch `s`, die
/// Pixeldichte mal `s`. Jede Regel des Layouts -- die Breite der Panes, die
/// Anker des schwebenden Terminals, die Zeilen des Terminals -- rechnet in
/// diesem kleineren Fenster und bricht dort neu um; erst beim Malen wächst
/// alles um `s`. Text wird dabei als Text gezeichnet, nicht als Bild
/// vergrößert, und bleibt scharf.
///
/// **Zeiger und Overlays.** [RenderUiScale] meldet seine Transformation beim
/// Treffertest und in `applyPaintTransform`. Ein Zeigerereignis kommt damit
/// in jedem Widget in dessen eigenen Koordinaten an (`localPosition`,
/// `delta`, die Geschwindigkeit eines Wurfs), `globalToLocal` und
/// `localToGlobal` rechnen richtig, und die Eingabemethode bekommt die wahre
/// Lage eines Textfelds. Der Navigator mit seinem Overlay steht unter dem
/// Wirt (`app.dart`, `WidgetsApp.builder`); Menüs, Beschriftungen beim
/// Überfahren, Blätter und Dialoge wachsen deshalb mit. Was ein Overlay an
/// eine globale Zeigerposition legt, rechnet sie vorher in seine Koordinaten
/// um (`HContextMenu`).
///
/// **Die Grenze.** Über 100 % gilt höchstens, was das Fenster trägt
/// ([uiScaleLimitFor]); ändert sich die Größe des Fensters, rechnet der Wirt
/// im selben Bild neu, weil er seine Größe aus dem Layout nimmt.
library;

import 'dart:async';

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../l10n/l10n.dart';
import 'announce.dart';
import 'ui.dart';
import 'ui_scale.dart';

/// Wie lange der Hinweis mit dem neuen Wert stehen bleibt.
///
/// Dieselbe Zeit wie die Bestätigung einer Kopie: eine Rückmeldung auf
/// einen eigenen Handgriff, lang genug für einen Blick. Jeder Druck beginnt
/// die Zeit neu.
const Duration uiScaleNoticeDuration = HMotion.copyFeedback;

/// Skaliert [child] nach der Wahl in [uiScaleProvider], begrenzt durch die
/// Größe, die der Wirt selbst bekommt.
///
/// Unter ein `HTheme` und unter die Lokalisierung setzen, über den
/// Navigator: im `builder` von `WidgetsApp`.
class UiScaleHost extends ConsumerStatefulWidget {
  /// Skaliert [child].
  const UiScaleHost({required this.child, super.key});

  /// Alles, was skaliert wird, mit dem Navigator und seinem Overlay.
  final Widget child;

  @override
  ConsumerState<UiScaleHost> createState() => _UiScaleHostState();
}

class _UiScaleHostState extends ConsumerState<UiScaleHost> {
  /// Der Text des Hinweises, solange er steht.
  String? _notice;
  Timer? _hide;

  /// Die Werte des letzten Layouts: Wahl, Grenze und was gilt.
  int _preferred = uiScaleDefault;
  int _limit = uiScaleMax;
  int _effective = uiScaleDefault;

  @override
  void dispose() {
    _hide?.cancel();
    super.dispose();
  }

  /// Zeigt nach einer verlangten Änderung kurz, was jetzt gilt, und sagt es
  /// auch einem Screenreader.
  ///
  /// Gerufen nach dem Bild, in dem die neue Wahl ankommt: Erst dann hat das
  /// Layout die Skalierung gerechnet, die gilt.
  ///
  /// [refused] meldet einen Schritt nach oben, den das Fenster abwies, auch
  /// wenn die Wahl genau an der Grenze steht; dann sagt der Hinweis es wie
  /// bei einer Wahl über der Grenze.
  void _requested(bool refused) {
    if (!mounted) {
      return;
    }
    final AppLocalizations l10n = context.l10n;
    final String percent = l10n.shellScalePercent(_effective);
    final bool limited =
        refused || (_preferred > _effective && _effective == _limit);
    final String text = limited
        ? l10n.shellScaleNoticeLimited(percent)
        : percent;
    _hide?.cancel();
    _hide = Timer(uiScaleNoticeDuration, () {
      if (mounted) {
        setState(() => _notice = null);
      }
    });
    setState(() => _notice = text);
    announcePolitely(context, text);
  }

  @override
  Widget build(BuildContext context) {
    final int preferred = ref.watch(uiScaleProvider);
    ref.listen<UiScaleRequest>(uiScaleRequestsProvider, (
      UiScaleRequest? _,
      UiScaleRequest request,
    ) {
      WidgetsBinding.instance
        ..addPostFrameCallback((Duration _) => _requested(request.refused))
        ..ensureVisualUpdate();
    });
    final MediaQueryData outer = MediaQuery.of(context);
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final Size window =
            constraints.hasBoundedWidth && constraints.hasBoundedHeight
            ? constraints.biggest
            : outer.size;
        final int limit = uiScaleLimitFor(window);
        final int effective = effectiveUiScale(preferred, window);
        final double scale = effective / 100;
        _preferred = preferred;
        _limit = limit;
        _effective = effective;
        return UiScaleScope(
          preferred: preferred,
          limit: limit,
          effective: effective,
          child: UiScaleBox(
            scale: scale,
            child: MediaQuery(
              data: scaledMediaQuery(outer, window, scale),
              child: Stack(
                fit: StackFit.expand,
                children: <Widget>[
                  widget.child,
                  if (_notice case final String text)
                    _UiScaleNotice(text: text),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

/// [outer] für einen Inhalt, der sich in [window] / [scale] logischen
/// Pixeln anlegt: Größe und alle Ränder geteilt durch [scale], die
/// Pixeldichte mal [scale]. Bei 1 unverändert.
MediaQueryData scaledMediaQuery(
  MediaQueryData outer,
  Size window,
  double scale,
) {
  if (scale == 1) {
    return outer;
  }
  return outer.copyWith(
    size: window / scale,
    devicePixelRatio: outer.devicePixelRatio * scale,
    padding: outer.padding / scale,
    viewPadding: outer.viewPadding / scale,
    viewInsets: outer.viewInsets / scale,
    systemGestureInsets: outer.systemGestureInsets / scale,
  );
}

/// Der Hinweis mit dem neuen Wert: oben in der Mitte, unter der Kopfleiste,
/// ohne Bewegung, ohne Fokus und ohne Treffer.
///
/// Er erscheint und verschwindet ohne Übergang: Er antwortet auf einen
/// Tastendruck, und eine Antwort, die erst einblendet, hinkt dem Druck nach
/// (`docs/UX.md` 2.9). Er nimmt keinen Zeiger an, damit er nichts verdeckt,
/// das gerade jemand anklicken will.
class _UiScaleNotice extends StatelessWidget {
  const _UiScaleNotice({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Align(
      alignment: Alignment.topCenter,
      child: Padding(
        padding: EdgeInsets.only(
          top: tokens.sizes.headerBar + tokens.spacing.x3,
        ),
        child: IgnorePointer(
          child: ExcludeSemantics(
            child: DecoratedBox(
              key: const Key('ui-scale-notice'),
              decoration: BoxDecoration(
                color: tokens.colors.bg2,
                borderRadius: BorderRadius.circular(tokens.radii.card),
                border: Border.all(color: tokens.colors.lineStrong),
              ),
              child: Padding(
                padding: EdgeInsets.symmetric(
                  horizontal: tokens.spacing.x3,
                  vertical: tokens.spacing.x2,
                ),
                child: Text(
                  text,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg0),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Legt [child] in seiner Größe geteilt durch [scale] an und malt es um
/// [scale] vergrößert ([RenderUiScale]).
class UiScaleBox extends SingleChildRenderObjectWidget {
  /// Skaliert [child] um [scale].
  const UiScaleBox({required this.scale, super.child, super.key});

  /// Der Faktor, größer 0.
  final double scale;

  @override
  RenderUiScale createRenderObject(BuildContext context) =>
      RenderUiScale(scale: scale);

  @override
  void updateRenderObject(BuildContext context, RenderUiScale renderObject) {
    renderObject.scale = scale;
  }
}

/// Das Render-Objekt hinter [UiScaleBox].
///
/// Layout, Malen, Treffertest und `applyPaintTransform` gehen durch dieselbe
/// eine Matrix, sonst träfe ein Klick bei 150 % ein anderes Element als das
/// unter dem Zeiger. Bei 1 ist es ein reiner Durchgang ohne eigene Schicht,
/// und die Bilder sind Pixel für Pixel dieselben wie ohne Wirt.
class RenderUiScale extends RenderProxyBox {
  /// Skaliert um [scale].
  RenderUiScale({required double scale, RenderBox? child})
    : assert(scale > 0),
      _scale = scale,
      super(child);

  /// Der Faktor.
  double get scale => _scale;
  double _scale;
  set scale(double value) {
    assert(value > 0);
    if (value == _scale) {
      return;
    }
    _scale = value;
    markNeedsLayout();
  }

  Matrix4 get _transform => Matrix4.diagonal3Values(_scale, _scale, 1);

  BoxConstraints _inner(BoxConstraints constraints) => BoxConstraints(
    minWidth: constraints.minWidth / _scale,
    maxWidth: constraints.maxWidth / _scale,
    minHeight: constraints.minHeight / _scale,
    maxHeight: constraints.maxHeight / _scale,
  );

  @override
  void performLayout() {
    final RenderBox? child = this.child;
    if (child == null) {
      size = constraints.smallest;
      return;
    }
    child.layout(_inner(constraints), parentUsesSize: true);
    size = constraints.constrain(child.size * _scale);
  }

  @override
  Size computeDryLayout(BoxConstraints constraints) {
    final RenderBox? child = this.child;
    if (child == null) {
      return constraints.smallest;
    }
    return constraints.constrain(
      child.getDryLayout(_inner(constraints)) * _scale,
    );
  }

  @override
  double computeMinIntrinsicWidth(double height) =>
      (child?.getMinIntrinsicWidth(height / _scale) ?? 0) * _scale;

  @override
  double computeMaxIntrinsicWidth(double height) =>
      (child?.getMaxIntrinsicWidth(height / _scale) ?? 0) * _scale;

  @override
  double computeMinIntrinsicHeight(double width) =>
      (child?.getMinIntrinsicHeight(width / _scale) ?? 0) * _scale;

  @override
  double computeMaxIntrinsicHeight(double width) =>
      (child?.getMaxIntrinsicHeight(width / _scale) ?? 0) * _scale;

  @override
  void paint(PaintingContext context, Offset offset) {
    final RenderBox? child = this.child;
    if (child == null) {
      layer = null;
      return;
    }
    if (_scale == 1) {
      layer = null;
      context.paintChild(child, offset);
      return;
    }
    layer = context.pushTransform(
      needsCompositing,
      offset,
      _transform,
      (PaintingContext context, Offset offset) =>
          context.paintChild(child, offset),
      oldLayer: layer is TransformLayer ? layer! as TransformLayer : null,
    );
  }

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) {
    final RenderBox? child = this.child;
    if (child == null) {
      return false;
    }
    return result.addWithPaintTransform(
      transform: _transform,
      position: position,
      hitTest: (BoxHitTestResult result, Offset position) =>
          child.hitTest(result, position: position),
    );
  }

  @override
  void applyPaintTransform(RenderBox child, Matrix4 transform) {
    transform.multiply(_transform);
  }

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DoubleProperty('scale', _scale));
  }
}
