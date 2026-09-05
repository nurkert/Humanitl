import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';

/// Ein Eintrag eines Kontextmenüs.
@immutable
class HMenuItem {
  /// Ein Eintrag mit Beschriftung und Handlung.
  const HMenuItem({
    required this.label,
    required this.onSelected,
    this.enabled = true,
    this.shortcut,
  });

  /// Was dort steht, übersetzt.
  final String label;

  /// Was der Klick auslöst. Läuft, nachdem das Menü zu ist.
  final VoidCallback onSelected;

  /// Ob der Eintrag gerade etwas tun kann.
  ///
  /// Ein deaktivierter Eintrag bleibt stehen und verschwindet nicht: Ein Menü,
  /// dessen Einträge kommen und gehen, verlernt seine Reihenfolge, und wer
  /// „Kopieren" sucht, sucht es dann jedes Mal neu (`docs/UX.md` 5.2).
  final bool enabled;

  /// Das Tastenkürzel, das dasselbe tut; nur Anzeige.
  final String? shortcut;
}

/// Ein Griff, mit dem ein anderes Widget dasselbe Menü öffnet.
///
/// Manche Flächen fangen den Rechtsklick selbst ab, weil sie ihn auch für
/// etwas anderes brauchen -- der Terminal-Emulator zum Beispiel kennt die
/// Zelle unter dem Zeiger und gibt sie mit. Sie halten deshalb einen Griff
/// und öffnen das Menü selbst, statt dass zwei Gestenerkenner um denselben
/// Klick streiten.
class HContextMenuController {
  void Function(Offset)? _open;
  VoidCallback? _close;

  /// Öffnet das Menü an dieser Stelle des Fensters.
  void open(Offset globalPosition) => _open?.call(globalPosition);

  /// Schließt es, falls es offen ist.
  void close() => _close?.call();
}

/// Das Menü, das ein Rechtsklick öffnet.
///
/// Zwei Zusagen, und beide stehen in `docs/UX.md`:
///
/// * **Es erscheint ohne Bewegung.** Wer rechts klickt, hat schon gezielt; ein
///   Menü, das hereinfliegt, kostet Aufmerksamkeit für eine Frage, die niemand
///   gestellt hat (2.2, Zeile „Command Palette").
/// * **Ein Klick zeigt eine Reaktion.** Jeder Eintrag füllt sich unter dem
///   Zeiger, und der Eintrag ist mindestens [HSize.hitMin] hoch — die zwei
///   Punkte der Barrierefreiheit, die bis 1.0.0 gelten, weil sie in Wahrheit
///   Benutzbarkeit sind (6).
///
/// Der erste Nutzer ist das Terminal des Sandbox-Bildschirms (HUM-042):
/// Kopieren aus der Auswahl und Einfügen sind Handlungen des Menschen und
/// deshalb erlaubt, während die Zwischenablage für den Agenten geschlossen
/// bleibt (`docs/SECURITY.md` 3.3). Der zweite ist die Warteschlange
/// (HUM-030).
class HContextMenu extends StatefulWidget {
  /// Legt das Menü über [child].
  const HContextMenu({
    required this.itemsBuilder,
    required this.child,
    this.controller,
    this.semanticsLabel,
    super.key,
  });

  /// Der Griff, mit dem [child] das Menü selbst öffnet.
  ///
  /// Ohne ihn öffnet ein Rechtsklick auf [child] es; mit ihm **nur** der
  /// Aufruf, denn dann hat [child] eigene Gründe für seinen Rechtsklick.
  final HContextMenuController? controller;

  /// Die Einträge, gebaut beim Öffnen.
  ///
  /// Eine Funktion und keine Liste: Ob „Kopieren" etwas zu kopieren hat,
  /// entscheidet sich in dem Augenblick, in dem das Menü aufgeht, und nicht
  /// als der Bildschirm gebaut wurde.
  final List<HMenuItem> Function() itemsBuilder;

  /// Worüber das Menü liegt.
  final Widget child;

  /// Was ein Screenreader über das Menü sagt.
  final String? semanticsLabel;

  @override
  State<HContextMenu> createState() => _HContextMenuState();
}

class _HContextMenuState extends State<HContextMenu> {
  OverlayEntry? _entry;

  @override
  void initState() {
    super.initState();
    _bind(widget.controller);
  }

  @override
  void didUpdateWidget(HContextMenu oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.controller, widget.controller)) {
      _unbind(oldWidget.controller);
      _bind(widget.controller);
    }
  }

  @override
  void dispose() {
    _unbind(widget.controller);
    _close();
    super.dispose();
  }

  void _bind(HContextMenuController? controller) {
    controller
      ?.._open = _open
      .._close = _close;
  }

  void _unbind(HContextMenuController? controller) {
    if (controller != null && identical(controller._open, _open)) {
      controller
        .._open = null
        .._close = null;
    }
  }

  void _open(Offset globalPosition) {
    final List<HMenuItem> items = widget.itemsBuilder();
    if (items.isEmpty) {
      return;
    }
    final OverlayState? overlay = Overlay.maybeOf(context);
    if (overlay == null) {
      return;
    }
    _close();
    final HTokens tokens = HTheme.of(context);
    final OverlayEntry entry = OverlayEntry(
      builder: (BuildContext context) => _Menu(
        at: globalPosition,
        items: items,
        tokens: tokens,
        semanticsLabel: widget.semanticsLabel,
        onDismiss: _close,
      ),
    );
    _entry = entry;
    overlay.insert(entry);
  }

  void _close() {
    _entry?.remove();
    _entry = null;
  }

  @override
  Widget build(BuildContext context) {
    // Wer einen Griff hält, hat eigene Gründe für seinen Rechtsklick; ein
    // zweiter Gestenerkenner darüber nähme ihm den Klick oder bekäme ihn nie.
    if (widget.controller != null) {
      return widget.child;
    }
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onSecondaryTapUp: (TapUpDetails details) => _open(details.globalPosition),
      onLongPressStart: (LongPressStartDetails details) =>
          _open(details.globalPosition),
      child: widget.child,
    );
  }
}

/// Die Fläche, die den Rest des Fensters abfängt, und das Menü darauf.
class _Menu extends StatelessWidget {
  const _Menu({
    required this.at,
    required this.items,
    required this.tokens,
    required this.onDismiss,
    this.semanticsLabel,
  });

  final Offset at;
  final List<HMenuItem> items;
  final HTokens tokens;
  final VoidCallback onDismiss;
  final String? semanticsLabel;

  /// Wie breit ein Eintrag höchstens wird.
  static const double _maxWidth = 260;

  @override
  Widget build(BuildContext context) {
    final Size window = MediaQuery.sizeOf(context);
    final double height = items.length * HSize.hitMin + HSpace.x1 * 2;
    final double left = at.dx.clamp(
      0.0,
      (window.width - _maxWidth).clamp(0.0, double.infinity),
    );
    final double top = at.dy.clamp(
      0.0,
      (window.height - height).clamp(0.0, double.infinity),
    );
    return Stack(
      children: <Widget>[
        // Der Rest des Fensters schließt das Menü. Kein Verdunkler: Ein
        // Kontextmenü ist keine Entscheidung, vor der ein Bildschirm
        // zurücktreten müsste (`docs/UX.md` 2.2).
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.translucent,
            onTap: onDismiss,
            onSecondaryTap: onDismiss,
            child: const SizedBox.expand(),
          ),
        ),
        Positioned(
          left: left,
          top: top,
          child: Shortcuts(
            shortcuts: const <ShortcutActivator, Intent>{
              SingleActivator(LogicalKeyboardKey.escape): DismissIntent(),
            },
            child: Actions(
              actions: <Type, Action<Intent>>{
                DismissIntent: CallbackAction<DismissIntent>(
                  onInvoke: (DismissIntent intent) {
                    onDismiss();
                    return null;
                  },
                ),
              },
              child: FocusScope(
                autofocus: true,
                child: Semantics(
                  label: semanticsLabel,
                  container: true,
                  child: Container(
                    width: _maxWidth,
                    padding: const EdgeInsets.symmetric(vertical: HSpace.x1),
                    decoration: BoxDecoration(
                      color: tokens.colors.bg2,
                      border: Border.all(color: tokens.colors.line),
                      borderRadius: BorderRadius.circular(HRadius.card),
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        for (final HMenuItem item in items)
                          _MenuRow(
                            item: item,
                            tokens: tokens,
                            onDismiss: onDismiss,
                          ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// Ein Eintrag, der sich unter dem Zeiger füllt.
class _MenuRow extends StatefulWidget {
  const _MenuRow({
    required this.item,
    required this.tokens,
    required this.onDismiss,
  });

  final HMenuItem item;
  final HTokens tokens;
  final VoidCallback onDismiss;

  @override
  State<_MenuRow> createState() => _MenuRowState();
}

class _MenuRowState extends State<_MenuRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = widget.tokens;
    final bool enabled = widget.item.enabled;
    final Color text = enabled ? tokens.colors.fg0 : tokens.colors.fg2;
    return MouseRegion(
      cursor: enabled ? SystemMouseCursors.click : SystemMouseCursors.basic,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: enabled
            ? () {
                widget.onDismiss();
                widget.item.onSelected();
              }
            : null,
        child: Container(
          constraints: const BoxConstraints(minHeight: HSize.hitMin),
          padding: const EdgeInsets.symmetric(horizontal: HSpace.x2),
          color: _hovered && enabled ? tokens.colors.bg3 : null,
          alignment: Alignment.centerLeft,
          child: Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  widget.item.label,
                  style: tokens.typography.ui13.copyWith(color: text),
                ),
              ),
              if (widget.item.shortcut case final String shortcut)
                Text(
                  shortcut,
                  style: tokens.typography.mono11.copyWith(
                    color: tokens.colors.fg2,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
