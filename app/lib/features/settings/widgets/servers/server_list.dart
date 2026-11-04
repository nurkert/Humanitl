/// The saved model servers as a calm list to choose from (HUM-414, before
/// that cards of HUM-405).
///
/// A row says only which server it is: its name, its host and port, the
/// word "Default" on the one default, and a small dot once "Check" ran in
/// this app session. Everything that changes a server stands in its detail
/// view beside the list; a row holds no button, switch or menu. A click
/// chooses a row, and the chosen row carries the look of the chosen
/// category of the overlay (`bg3` and the 2 px accent mark).
///
/// The list is one stop of the `Tab` order, like the column of categories:
/// `Up` and `Down` move the choice, `Home` and `End` jump, `Enter` goes into
/// the detail view. The row "Add server" stands last.
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/shortcuts/shortcut_catalog.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/server_checks.dart';

/// The value of the row "Add server" among the names of [ServerList].
const String serverAddRow = '+';

/// The servers of [servers] in list order: the default first, it serves
/// every session without its own choice and is the one most often looked
/// for; the others by name.
List<SavedServer> serversInListOrder(SavedServers servers) => <SavedServer>[
  ...servers.servers.where((SavedServer server) => server.active),
  ...servers.servers.where((SavedServer server) => !server.active),
];

/// The list of [servers] and, at its end, the row that adds one.
class ServerList extends StatefulWidget {
  /// Creates the list; [selected] is the chosen name, [serverAddRow] while a
  /// new server is added, or null while none is chosen.
  const ServerList({
    required this.servers,
    required this.selected,
    required this.onSelect,
    required this.onEnter,
    super.key,
  });

  /// The saved servers.
  final SavedServers servers;

  /// The chosen row.
  final String? selected;

  /// Invoked with the name of the chosen row, or [serverAddRow].
  final ValueChanged<String> onSelect;

  /// `Enter` on a row: the row is chosen, and the focus goes into the
  /// detail view.
  final ValueChanged<String> onEnter;

  @override
  State<ServerList> createState() => ServerListState();
}

/// The state of [ServerList]; [focusSelected] brings the focus back from the
/// detail view (`Escape`).
class ServerListState extends State<ServerList> {
  final Map<String, FocusNode> _nodes = <String, FocusNode>{};

  /// The row the arrow keys stand on before the choice settles; the detail
  /// view follows once the keys rest for [settleAfter], so a held key does
  /// not rebuild it, and ask the daemon about a key, for every row it
  /// passes.
  String? _cursor;
  Timer? _settle;

  /// How long the arrow keys rest before the choice follows them.
  static const Duration settleAfter = Duration(milliseconds: 150);

  FocusNode _node(String name) =>
      _nodes.putIfAbsent(name, () => FocusNode(debugLabel: 'server-$name'));

  List<String> get _order => <String>[
    for (final SavedServer server in serversInListOrder(widget.servers))
      server.name,
    serverAddRow,
  ];

  /// Puts the focus on the chosen row, or on the first one.
  void focusSelected() {
    final List<String> order = _order;
    final String target = order.contains(widget.selected)
        ? widget.selected!
        : order.first;
    _node(target).requestFocus();
  }

  @override
  void didUpdateWidget(ServerList oldWidget) {
    super.didUpdateWidget(oldWidget);
    final Set<String> alive = _order.toSet();
    _nodes.removeWhere((String name, FocusNode node) {
      if (alive.contains(name)) {
        return false;
      }
      node.dispose();
      return true;
    });
  }

  @override
  void dispose() {
    _settle?.cancel();
    for (final FocusNode node in _nodes.values) {
      node.dispose();
    }
    super.dispose();
  }

  String get _current {
    for (final MapEntry<String, FocusNode> entry in _nodes.entries) {
      if (entry.value.hasFocus) {
        return entry.key;
      }
    }
    return _cursor ?? widget.selected ?? _order.first;
  }

  KeyEventResult _onKey(FocusNode _, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final List<String> order = _order;
    final int at = order.indexOf(_current).clamp(0, order.length - 1);
    // Die Tasten stehen im Katalog (HUM-420).
    final int? next = switch (event) {
      _ when AppShortcut.listNext.matches(event) => (at + 1).clamp(
        0,
        order.length - 1,
      ),
      _ when AppShortcut.listPrevious.matches(event) => (at - 1).clamp(
        0,
        order.length - 1,
      ),
      _ when AppShortcut.listFirst.matches(event) => 0,
      _ when AppShortcut.listLast.matches(event) => order.length - 1,
      _ => null,
    };
    if (next == null) {
      return KeyEventResult.ignored;
    }
    final String name = order[next];
    setState(() => _cursor = name);
    _node(name).requestFocus();
    _settle?.cancel();
    _settle = Timer(settleAfter, () {
      if (mounted) {
        setState(() => _cursor = null);
        widget.onSelect(name);
      }
    });
    return KeyEventResult.handled;
  }

  /// A click chooses the row and puts the focus on it, as in the column of
  /// categories, so the arrow keys go on from there.
  void _tap(String name) {
    _settle?.cancel();
    _cursor = null;
    widget.onSelect(name);
    _node(name).requestFocus();
  }

  void _enter(String name) {
    _settle?.cancel();
    _cursor = null;
    widget.onEnter(name);
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final List<SavedServer> ordered = serversInListOrder(widget.servers);
    final List<String> order = _order;
    final String? selected = _cursor ?? widget.selected;
    // Ein Halt im `Tab`: die gewählte Zeile, ohne Wahl die erste.
    final String tabStop = order.contains(selected) ? selected! : order.first;
    for (final String name in order) {
      _node(name).skipTraversal = name != tabStop;
    }
    final int count = ordered.length;
    return Semantics(
      container: true,
      label: l10n.settingsServerListLabel,
      child: Focus(
        canRequestFocus: false,
        skipTraversal: true,
        onKeyEvent: _onKey,
        child: Column(
          key: const Key('settings-server-list'),
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            for (int i = 0; i < count; i++)
              _ServerRow(
                key: ValueKey<String>('settings-server-${ordered[i].name}'),
                server: ordered[i],
                position: l10n.settingsServerPosition(i + 1, count),
                selected: selected == ordered[i].name,
                focusNode: _node(ordered[i].name),
                onTap: () => _tap(ordered[i].name),
                onEnter: () => _enter(ordered[i].name),
              ),
            _AddRow(
              selected: selected == serverAddRow,
              focusNode: _node(serverAddRow),
              onTap: () => _tap(serverAddRow),
              onEnter: () => _enter(serverAddRow),
            ),
          ],
        ),
      ),
    );
  }
}

/// The surface every row of the list shares: transparent, `bg2` under the
/// pointer, `bg3` with the accent mark when chosen; click, `Enter` and
/// `Space` choose it.
class _RowSurface extends StatefulWidget {
  const _RowSurface({
    required this.selected,
    required this.focusNode,
    required this.onTap,
    required this.onEnter,
    required this.semanticsLabel,
    required this.builder,
  });

  final bool selected;
  final FocusNode focusNode;
  final VoidCallback onTap;
  final VoidCallback onEnter;
  final String semanticsLabel;
  final Widget Function(BuildContext context, bool hovered) builder;

  @override
  State<_RowSurface> createState() => _RowSurfaceState();
}

class _RowSurfaceState extends State<_RowSurface> {
  // Ein Feld, kein Ausdruck in `build`: Eine Map, die jeder Build neu baut,
  // ist für jeden Nachfahren, der von ihr abhängt, ein neues Objekt
  // (`docs/UX.md` 7).
  late final Map<ShortcutActivator, Intent> _shortcuts =
      AppShortcut.bind<Intent>(const <AppShortcut, Intent>{
        AppShortcut.serverOpen: ActivateIntent(),
        AppShortcut.serverChoose: ButtonActivateIntent(),
      });

  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool selected = widget.selected;
    final Color fill = selected
        ? tokens.colors.bg3
        : _hovered
        ? tokens.colors.bg2
        : const Color(0x00000000);
    return Semantics(
      button: true,
      selected: selected,
      label: widget.semanticsLabel,
      excludeSemantics: true,
      child: FocusableActionDetector(
        focusNode: widget.focusNode,
        mouseCursor: SystemMouseCursors.click,
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        shortcuts: _shortcuts,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              widget.onEnter();
              return null;
            },
          ),
          ButtonActivateIntent: CallbackAction<ButtonActivateIntent>(
            onInvoke: (ButtonActivateIntent _) {
              widget.onTap();
              return null;
            },
          ),
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: HFocusRing.inline(
            visible: _focused,
            child: AnimatedContainer(
              duration: HMotion.press,
              color: fill,
              constraints: const BoxConstraints(minHeight: serverRowHeight),
              child: Stack(
                children: <Widget>[
                  if (selected)
                    Positioned(
                      left: 0,
                      top: 0,
                      bottom: 0,
                      width: HSize.selectionRail,
                      child: ColoredBox(color: tokens.colors.accent),
                    ),
                  Padding(
                    padding: EdgeInsets.symmetric(
                      horizontal: tokens.spacing.x3,
                      vertical: tokens.spacing.x2,
                    ),
                    child: widget.builder(context, _hovered),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The least height of a row: two lines and their padding, the same for the
/// row that adds, so the list keeps one rhythm.
const double serverRowHeight = 52;

/// One saved server.
class _ServerRow extends ConsumerWidget {
  const _ServerRow({
    required this.server,
    required this.position,
    required this.selected,
    required this.focusNode,
    required this.onTap,
    required this.onEnter,
    super.key,
  });

  final SavedServer server;
  final String position;
  final bool selected;
  final FocusNode focusNode;
  final VoidCallback onTap;
  final VoidCallback onEnter;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String name = server.name;
    ref.watch(serverChecksProvider);
    final ServerCheck? check = ref
        .read(serverChecksProvider.notifier)
        .of(name, server.endpoint);
    final String host = server.endpoint.isEmpty
        ? l10n.settingsServerNoAddress
        : serverHost(server.endpoint);
    final String? checkText = switch (check) {
      null => null,
      ServerCheck(ok: true) => l10n.settingsServerCheckedModels(check.models),
      _ => l10n.settingsServerCheckFailed,
    };
    return _RowSurface(
      selected: selected,
      focusNode: focusNode,
      onTap: onTap,
      onEnter: onEnter,
      semanticsLabel: <String>[
        name,
        host,
        if (server.active) l10n.settingsServerDefault,
        ?checkText,
        position,
      ].join(', '),
      builder: (BuildContext context, bool hovered) => Row(
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Text(
                  name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: selected
                      ? tokens.typography.ui13.medium.tinted(tokens.colors.fg0)
                      : tokens.typography.ui13.tinted(tokens.colors.fg0),
                ),
                Text(
                  host,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tokens.typography.mono11.tinted(tokens.colors.fg1),
                ),
              ],
            ),
          ),
          if (server.active) ...<Widget>[
            SizedBox(width: tokens.spacing.x2),
            HGlyphIcon(
              HGlyph.star,
              key: ValueKey<String>('settings-server-star-$name'),
              size: 11,
              color: tokens.colors.accent,
            ),
            SizedBox(width: tokens.spacing.x1),
            Text(
              l10n.settingsServerDefault,
              key: ValueKey<String>('settings-server-default-$name'),
              style: tokens.typography.ui11.medium.tinted(
                tokens.colors.accentText,
              ),
            ),
          ],
          if (check != null) ...<Widget>[
            SizedBox(width: tokens.spacing.x2),
            _CheckDot(
              key: ValueKey<String>('settings-server-checked-$name'),
              check: check,
              label: checkText!,
            ),
          ],
        ],
      ),
    );
  }
}

/// The mark of the last check: a tick with models, a dash without, a
/// triangle when it failed. The shape tells what the colour repeats, and the
/// words stand in the row's label.
class _CheckDot extends StatelessWidget {
  const _CheckDot({required this.check, required this.label, super.key});

  final ServerCheck check;
  final String label;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final (HGlyph glyph, Color color) = !check.ok
        ? (HGlyph.triangleAlert, tokens.stateText.error)
        : check.models > 0
        ? (HGlyph.check, tokens.stateText.allowed)
        : (HGlyph.minus, tokens.colors.fg2);
    return HGlyphIcon(glyph, size: 12, color: color);
  }
}

/// The row at the end that adds a server.
class _AddRow extends StatelessWidget {
  const _AddRow({
    required this.selected,
    required this.focusNode,
    required this.onTap,
    required this.onEnter,
  });

  final bool selected;
  final FocusNode focusNode;
  final VoidCallback onTap;
  final VoidCallback onEnter;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return KeyedSubtree(
      key: const Key('settings-server-add'),
      child: _RowSurface(
        selected: selected,
        focusNode: focusNode,
        onTap: onTap,
        onEnter: onEnter,
        semanticsLabel: l10n.settingsServerAdd,
        builder: (BuildContext context, bool hovered) => Row(
          children: <Widget>[
            HGlyphIcon(
              HGlyph.plus,
              size: 12,
              color: selected || hovered
                  ? tokens.colors.fg0
                  : tokens.colors.fg1,
            ),
            SizedBox(width: tokens.spacing.x2),
            Expanded(
              child: Text(
                l10n.settingsServerAdd,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style:
                    (selected
                            ? tokens.typography.ui13.medium
                            : tokens.typography.ui13)
                        .tinted(
                          selected || hovered
                              ? tokens.colors.fg0
                              : tokens.colors.fg1,
                        ),
              ),
            ),
            HKeyHint(l10n.settingsServerAddShortcut, color: tokens.colors.fg2),
          ],
        ),
      ),
    );
  }
}
