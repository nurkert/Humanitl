/// The settings as one component: the head with the search, the column of
/// categories on the left and the rows or hits on the right (HUM-535, before
/// that the content of the overlay, HUM-404).
///
/// # Hosts
///
/// Two hosts draw the panel, each only a frame around it: `SettingsSheet`
/// in a session, the overlay from below, and `SettingsTab`, a tab of the
/// start window (ADR-0024). A host decides the frame, whether the panel can
/// be closed and what `Esc` does once the search is empty; everything else
/// belongs to the panel: the content, the search, the jump of a
/// `ChangeSetting` fix and the chosen category. A row that changes here
/// changes in both hosts without a line in either.
///
/// The host hands its frame in as a [SettingsFrameBuilder] instead of
/// wrapping the panel from outside: the keys of the panel (`Ctrl+F`, `Esc`)
/// stand above the frame, as they stood above the overlay, so they still
/// work when the keyboard rests on the focus scope of a sheet
/// (`HLayerScope`) and not on a control of the panel.
///
/// # Keys
///
/// `Tab` goes search, categories, page, close button; the arrow keys move
/// between the categories. `Ctrl+F` puts the cursor into the search, and so
/// does typing anywhere outside a text field. `Esc` empties the search
/// first; after that it calls [SettingsPanel.onClose], and without one it
/// lets the key pass to whoever stands around the host.
///
/// # Writing
///
/// A switch or a choice is written at once through `SetConfig`, and the row
/// says "Saved" for a moment; a refusal of the daemon stands at the row
/// instead. Endpoint, model and key stand in the detail view of a server
/// (`ServerDetail`, HUM-405).
/// The panel decides nothing about what a value means (ADR-018).
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/config_providers.dart';
import '../../core/ipc/connection.dart';
import '../../core/shortcuts/shortcut_catalog.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'registry/settings_registry.dart';
import 'registry/settings_search.dart';
import 'settings_scope.dart';
import 'widgets/setting_row.dart';
import 'widgets/settings_category_page.dart';
import 'widgets/settings_header.dart';
import 'widgets/settings_nav.dart';
import 'widgets/settings_search_results.dart';

/// Draws the frame of a host around [panel].
///
/// [close] closes through the panel, so a form that holds a change asks
/// first (HUM-414); a host that closes on a click outside the panel calls
/// it. A host that cannot be closed ignores it.
typedef SettingsFrameBuilder = Widget Function(
  BuildContext context,
  Widget panel,
  VoidCallback close,
);

/// Puts the cursor into the search field of the panel.
class _FocusSearchIntent extends Intent {
  const _FocusSearchIntent();
}

/// `Esc` in the panel: the search first, then the host.
///
/// An intent of its own and not `DismissIntent`: a host without
/// [SettingsPanel.onClose] lets `Esc` pass, and a `DismissIntent` binding
/// around the host must then find its own action and not the disabled one
/// of the panel.
class _EscapeIntent extends Intent {
  const _EscapeIntent();
}

/// The action behind [_EscapeIntent]; enabled only while it has something
/// to do, so that an unused `Esc` passes on.
class _EscapeAction extends Action<_EscapeIntent> {
  _EscapeAction(this._state);

  final _SettingsPanelState _state;

  @override
  bool isEnabled(_EscapeIntent intent) =>
      _state._query.text.isNotEmpty || _state.widget.onClose != null;

  @override
  Object? invoke(_EscapeIntent intent) {
    _state._escape();
    return null;
  }
}

/// The settings: head with search, categories, rows or hits.
class SettingsPanel extends ConsumerStatefulWidget {
  /// Creates the panel inside the frame [frame] of its host.
  const SettingsPanel({
    required this.frame,
    this.onClose,
    this.closing = false,
    this.autofocus = true,
    this.focusKey,
    this.focusSerial = 0,
    this.daemonRestart,
    this.onOpenSession,
    super.key,
  });

  /// The frame of the host around the panel.
  final SettingsFrameBuilder frame;

  /// Asks the host to close, after a form that holds a change agreed
  /// (HUM-414); null for a host that does not close. With it the head shows
  /// a close button and `Esc` on an empty search closes; without it neither.
  final VoidCallback? onClose;

  /// Whether the host is taking the panel away: no jump and no search
  /// starts any more.
  final bool closing;

  /// Whether the search takes the keyboard when the panel is built. A sheet
  /// holds the keyboard and passes true; a tab beside other tabs must not
  /// pull the keyboard to itself and passes false.
  final bool autofocus;

  /// The setting the panel opens at, or null (HUM-275): a diagnostic's
  /// `ChangeSetting` fix names its key. The panel opens its category,
  /// scrolls to the row, focuses its control and lights it briefly.
  final String? focusKey;

  /// Counts the requests for [focusKey]. A second click on the same fix
  /// lands on the row again, even if the focus moved away in between.
  final int focusSerial;

  /// The control that restarts the daemon, or null (HUM-404).
  ///
  /// The restart and its question during a session belong to the shell
  /// (`features/shell/daemon_restart.dart`), and no feature imports another
  /// (ARCHITECTURE 5); the shell hands its control in, and the category
  /// Daemon draws it in a row of its own.
  final Widget? daemonRestart;

  /// Shows the terminal of the running session (HUM-521); null for a host
  /// without a shell.
  final VoidCallback? onOpenSession;

  /// The width of the column of categories, and the narrower one below
  /// [narrowBelow] of panel width.
  static const double navWidth = 200;

  /// See [navWidth].
  static const double navWidthNarrow = 160;

  /// The panel width under which the column narrows.
  static const double narrowBelow = 760;

  /// The widest a page gets; on a wide window the rows do not stretch until
  /// the control has drifted away from its name.
  static const double pageMaxWidth = 760;

  @override
  ConsumerState<SettingsPanel> createState() => _SettingsPanelState();
}

class _SettingsPanelState extends ConsumerState<SettingsPanel> {
  // Ein Feld, kein Ausdruck in `build`: Eine Map, die jeder Build neu baut,
  // ist für jeden Nachfahren, der von ihr abhängt, ein neues Objekt
  // (`docs/UX.md` 7).
  late final Map<ShortcutActivator, Intent> _shortcuts =
      AppShortcut.bind<Intent>(const <AppShortcut, Intent>{
        AppShortcut.settingsClose: _EscapeIntent(),
        AppShortcut.settingsSearch: _FocusSearchIntent(),
      });

  late final Map<Type, Action<Intent>> _actions = <Type, Action<Intent>>{
    _EscapeIntent: _EscapeAction(this),
    _FocusSearchIntent: CallbackAction<_FocusSearchIntent>(
      onInvoke: (_FocusSearchIntent _) {
        _searchNode.requestFocus();
        _query.selection = TextSelection(
          baseOffset: 0,
          extentOffset: _query.text.length,
        );
        return null;
      },
    ),
  };

  late final SettingsController _settings = SettingsController(
    (String key, String value) =>
        ref.read(configProvider.notifier).write(key, value),
  );

  final TextEditingController _query = TextEditingController();
  final FocusNode _searchNode = FocusNode(debugLabel: 'settings-search');
  final ScrollController _scroll = ScrollController();

  /// The id of the category of the page; null until the catalog answered,
  /// then its first one.
  String? _category;

  /// The view of the catalog of the last build.
  SettingsView? _view;

  /// A setting a jump asked for before the catalog answered (HUM-534); the
  /// build that has the catalog turns it into [_category] and [_pending].
  String? _wanted;

  /// The setting a jump wants to land on until its row stands (HUM-275).
  ///
  /// A fix can open the panel before `GetConfig` or `GetInfo` answered;
  /// then the row comes later. The target waits for it, and every build
  /// tries again, because the panel rebuilds as soon as either answers.
  /// It goes once the row was revealed, or when the daemon really does not
  /// offer the setting.
  String? _pending;

  /// Whether a try for [_pending] is already queued.
  bool _tryQueued = false;

  @override
  void initState() {
    super.initState();
    _query.addListener(_queryChanged);
    if (!_jumpTo(widget.focusKey) && widget.autofocus) {
      _afterLayer(_searchNode.requestFocus);
    }
  }

  @override
  void didUpdateWidget(SettingsPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.closing && !oldWidget.closing) {
      // Auf dem Weg hinaus nimmt kein Sprungziel mehr die Tastatur.
      _pending = null;
      _settings.want(null);
    }
    if (widget.focusKey != oldWidget.focusKey ||
        widget.focusSerial != oldWidget.focusSerial) {
      setState(() => _jumpTo(widget.focusKey));
    }
  }

  @override
  void dispose() {
    _query
      ..removeListener(_queryChanged)
      ..dispose();
    _searchNode.dispose();
    _scroll.dispose();
    _settings.dispose();
    super.dispose();
  }

  void _queryChanged() => setState(() {
    // Wer sucht, ist weiter als ein Sprungziel, das noch wartet.
    if (_query.text.isNotEmpty) {
      _pending = null;
      _wanted = null;
      _settings.want(null);
    }
  });

  /// Opens the category of [key] and lets [_pending] wait for its row.
  /// Before the catalog answered the key waits in [_wanted]; a key the
  /// catalog does not know is dropped once it answered.
  bool _jumpTo(String? key) {
    if (key == null) {
      return false;
    }
    final SettingEntry? entry = _view?.entry(key);
    if (entry == null) {
      if (_view?.entries.isNotEmpty ?? false) {
        return false;
      }
      _query.clear();
      _wanted = key;
      return true;
    }
    _query.clear();
    _land(entry);
    return true;
  }

  /// Opens the category of [entry] and lets [_pending] wait for its row.
  void _land(SettingEntry entry) {
    _wanted = null;
    _category = entry.category;
    _pending = entry.id;
    _settings.want(entry.id);
    _queueTry();
  }

  void _queueTry() {
    if (_tryQueued) {
      return;
    }
    _tryQueued = true;
    _afterLayer(() {
      _tryQueued = false;
      _tryReveal();
    });
  }

  /// Reveals [_pending] if its row stands. A daemon that answered and does
  /// not offer the setting drops it: there is nothing to wait for.
  void _tryReveal() {
    final String? id = _pending;
    if (id == null) {
      return;
    }
    if (widget.closing) {
      _pending = null;
      return;
    }
    // Ein Katalog, der den Eintrag nicht nennt, bietet ihn nicht an: Es gibt
    // nichts, worauf zu warten wäre (HUM-534).
    final SettingsView? view = _view;
    if (view != null && view.entries.isNotEmpty && view.entry(id) == null) {
      _pending = null;
      return;
    }
    // Eine Seite, die die Zeile erst nach einem eigenen Schritt zeichnet,
    // hat den Sprung schon selbst ausgeführt (HUM-405).
    if (_settings.wanted != id || _settings.reveal(id)) {
      _pending = null;
    }
  }

  /// Runs [action] once a layer around the panel has taken the keyboard:
  /// after the frame that builds it, and behind the request of the layer
  /// itself, which is queued in the same frame (`HLayerScope`). Without a
  /// layer the same moment does no harm.
  void _afterLayer(VoidCallback action) {
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      scheduleMicrotask(() {
        if (mounted) {
          action();
        }
      });
    });
  }

  /// Closes through the host; a form that holds a change asks first
  /// (HUM-414). Nothing without [SettingsPanel.onClose].
  void _close() {
    final VoidCallback? onClose = widget.onClose;
    if (onClose != null) {
      _settings.leave(onClose);
    }
  }

  void _choose(String category) {
    if (_query.text.isEmpty && category == _category) {
      return;
    }
    _settings.leave(() => _show(category));
  }

  void _show(String category) {
    if (!mounted) {
      return;
    }
    setState(() {
      _query.clear();
      _category = category;
      _pending = null;
      _settings.want(null);
    });
    if (_scroll.hasClients) {
      _scroll.jumpTo(0);
    }
  }

  /// A hit of the search: its category, then the row.
  void _chooseHit(String id) =>
      _settings.leave(() => mounted ? setState(() => _jumpTo(id)) : null);

  void _clearSearch() {
    _query.clear();
    _searchNode.requestFocus();
  }

  /// `Esc`: the search first, then the host.
  void _escape() {
    if (_query.text.isNotEmpty) {
      _clearSearch();
    } else {
      _close();
    }
  }

  /// Typing outside a text field starts a search with that character.
  KeyEventResult _typeToSearch(FocusNode _, KeyEvent event) {
    // Auf dem Weg hinaus beginnt keine Suche mehr.
    if (event is! KeyDownEvent || widget.closing || _searchNode.hasFocus) {
      return KeyEventResult.ignored;
    }
    final String? character = event.character;
    if (character == null ||
        character.trim().isEmpty ||
        character.codeUnitAt(0) < 0x20 ||
        character.codeUnitAt(0) == 0x7f) {
      return KeyEventResult.ignored;
    }
    final HardwareKeyboard keys = HardwareKeyboard.instance;
    if (keys.isControlPressed || keys.isAltPressed || keys.isMetaPressed) {
      return KeyEventResult.ignored;
    }
    final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
    if (focused != null &&
        (focused.widget is EditableText ||
            focused.findAncestorWidgetOfExactType<EditableText>() != null)) {
      return KeyEventResult.ignored;
    }
    final String text = _query.text + character;
    _query.value = TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
    _searchNode.requestFocus();
    return KeyEventResult.handled;
  }

  /// `Enter` in the search takes the first hit.
  void _submitSearch() {
    final SettingsView? view = _view;
    if (view == null) {
      return;
    }
    final List<SettingsSearchGroup> groups = searchSettings(_query.text, view);
    if (groups.isNotEmpty) {
      _chooseHit(groups.first.entries.first.id);
    }
  }

  @override
  Widget build(BuildContext context) {
    // Die Antworten, auf die ein Sprungziel wartet: Kommt eine, baut das
    // Panel neu, und der nächste Versuch findet die Zeile.
    ref
      ..watch(configProvider)
      ..watch(daemonInfoProvider);
    final SettingsView view = _view = SettingsView.of(context, ref);
    if (_wanted case final String wanted when view.entries.isNotEmpty) {
      // Ein Ziel, das der Katalog nicht nennt, bietet der Daemon nicht an:
      // Die Suche bekommt die Tastatur, wie beim Öffnen ohne Ziel. Im Bau
      // wird die Suche nicht geleert; das tat schon der Sprung.
      if (view.entry(wanted) case final SettingEntry entry) {
        _land(entry);
      } else {
        _wanted = null;
        if (widget.autofocus) {
          _afterLayer(_searchNode.requestFocus);
        }
      }
    }
    if (view.categories.isNotEmpty &&
        (_category == null || view.category(_category!) == null)) {
      _category = view.categories.first.id;
    }
    if (_pending != null) {
      _queueTry();
    }
    return Shortcuts(
      shortcuts: _shortcuts,
      child: Actions(
        actions: _actions,
        child: HTheme.host(
          context,
          Builder(
            builder: (BuildContext context) => widget.frame(
              context,
              Focus(
                canRequestFocus: false,
                skipTraversal: true,
                onKeyEvent: _typeToSearch,
                child: LayoutBuilder(
                  builder: (BuildContext context, BoxConstraints box) =>
                      _content(context, width: box.maxWidth),
                ),
              ),
              _close,
            ),
          ),
        ),
      ),
    );
  }

  Widget _content(BuildContext context, {required double width}) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final double nav = width < SettingsPanel.narrowBelow
        ? SettingsPanel.navWidthNarrow
        : SettingsPanel.navWidth;
    final SettingsView view = _view ?? SettingsView.of(context, ref);
    final AsyncValue<ConfigValues> config = ref.watch(configProvider);
    final LlmKeyStatus status =
        ref.watch(llmKeyProvider).value ?? LlmKeyStatus.unset;
    final ConfigValues? values = config.value;
    final String query = _query.text;
    final bool searching = query.trim().isNotEmpty;
    // Die linke Kante der Seite: Spalte, Linie und Rand der Seite. Dazu der
    // Einzug der Zeile, und auf dieser Kante beginnt auch das Suchfeld.
    final double textEdge = nav + HSize.hairline + tokens.spacing.x4;

    final AsyncValue<SettingsCatalog> catalog = ref.watch(
      settingsCatalogProvider(l10n.localeName),
    );
    final Widget page = values == null || catalog.value == null
        ? Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: SettingRowGrid.inset,
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  config.hasError || catalog.hasError
                      ? l10n.settingsLoadFailed
                      : l10n.settingsLoading,
                  key: const Key('settings-loading'),
                  style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                ),
                // Ohne Wiederholung bliebe ein einmal gescheiterter Abruf bis
                // zur nächsten Änderung stehen (HUM-534).
                if (config.hasError || catalog.hasError) ...<Widget>[
                  SizedBox(height: tokens.spacing.x3),
                  HButton(
                    key: const Key('settings-load-retry'),
                    onPressed: () {
                      ref.invalidate(settingsCatalogProvider);
                      unawaited(ref.read(configProvider.notifier).refresh());
                    },
                    child: Text(l10n.settingsLoadRetry),
                  ),
                ],
              ],
            ),
          )
        : searching
        ? SettingsSearchResults(
            query: query,
            groups: searchSettings(query, view),
            onChoose: _chooseHit,
            onClear: _clearSearch,
          )
        : SettingsCategoryPage(
            category: _category ?? '',
            data: SettingsPageData(
              config: values,
              status: status,
              daemonRestart: widget.daemonRestart,
              onOpenSession: widget.onOpenSession,
            ),
          );

    return SettingsScope(
      key: const Key('settings-panel'),
      controller: _settings,
      child: FocusTraversalGroup(
        policy: OrderedTraversalPolicy(),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            SettingsHeader(
              textEdge: textEdge,
              pageMaxWidth: SettingsPanel.pageMaxWidth,
              searchController: _query,
              searchFocus: _searchNode,
              onSubmitSearch: _submitSearch,
              onClose: widget.onClose == null ? null : _close,
            ),
            const HHairline(),
            Expanded(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  SizedBox(
                    width: nav,
                    child: FocusTraversalOrder(
                      order: const NumericFocusOrder(2),
                      child: FocusTraversalGroup(
                        child: Padding(
                          padding: EdgeInsets.only(top: tokens.spacing.x2),
                          child: SettingsNav(
                            categories: view.categories,
                            selected: searching ? null : _category,
                            onSelect: _choose,
                          ),
                        ),
                      ),
                    ),
                  ),
                  const HHairline(vertical: true),
                  Expanded(
                    child: FocusTraversalOrder(
                      order: const NumericFocusOrder(3),
                      child: FocusTraversalGroup(
                        child: SingleChildScrollView(
                          key: ValueKey<String>(
                            searching
                                ? 'settings-scroll-search'
                                : 'settings-scroll-$_category',
                          ),
                          controller: _scroll,
                          padding: EdgeInsets.fromLTRB(
                            tokens.spacing.x4,
                            tokens.spacing.x4,
                            tokens.spacing.x4,
                            tokens.spacing.x8,
                          ),
                          child: Align(
                            alignment: AlignmentDirectional.topStart,
                            child: ConstrainedBox(
                              constraints: const BoxConstraints(
                                maxWidth: SettingsPanel.pageMaxWidth,
                              ),
                              child: page,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
