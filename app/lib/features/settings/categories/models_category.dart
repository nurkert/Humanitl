/// The category Models: the saved servers as a list with the detail view of
/// the chosen one beside it, and below, as a section of its own, who uses
/// which server (HUM-405, list and detail since HUM-414).
///
/// On a wide page the list stands left and the detail view of the chosen
/// row right; one row is always chosen, the default when nobody chose
/// another, so a second server added later causes no jump of the layout. On
/// a narrow page the list takes the whole width, and a click puts the detail
/// view in its place, with "All servers" back. Without any server the page
/// says what that means and offers to add one. Nothing here writes on its
/// own; every change is one RPC of `LlmServers` or `LlmKey` (ADR-018).
///
/// A change that is not saved yet is not dropped by a click on another row:
/// the detail view then asks, in its own footer, whether to save, discard or
/// keep editing.
///
/// A jump of a `ChangeSetting` fix or of the search to the endpoint, the
/// model or the key chooses the default server, or a new one while none is
/// saved, and then lands on the row (HUM-275).
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/shortcuts/intents.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../registry/setting_entry.dart';
import '../settings_scope.dart';
import '../widgets/servers/server_detail.dart';
import '../widgets/servers/server_empty.dart';
import '../widgets/servers/server_list.dart';
import '../widgets/servers/server_usage.dart';
import '../widgets/servers/sticky_pane.dart';
import '../widgets/setting_row.dart';

/// Draws the whole category.
Widget buildModelsCategory(SettingsPageData data) => const ModelsCategory();

/// The settings that stand in the detail view of a server.
const Set<String> serverFormIds = <String>{
  FormKeys.llmEndpoint,
  FormKeys.llmModels,
  FormKeys.llmApiKeyRef,
};

/// The width of the list beside the detail view.
const double serverListWidth = 232;

/// Below this page width the list and the detail view take turns instead of
/// standing side by side: the detail view would get less than a form needs.
const double serverListSideBySide = 600;

/// The wish to go back to the list from the detail view on a narrow page;
/// a value of the pending choice beside the names and [serverAddRow].
const String _backToList = '<list>';

/// The wish to leave the page (close the overlay, another category); a
/// value of the pending choice, with [_ModelsCategoryState._proceed].
const String _away = '<away>';

/// The list, the detail view and the table of assignments.
class ModelsCategory extends ConsumerStatefulWidget {
  /// Creates the category.
  const ModelsCategory({super.key});

  @override
  ConsumerState<ModelsCategory> createState() => _ModelsCategoryState();
}

/// `Ctrl+N` on the page: a new server.
class _AddServerIntent extends Intent {
  const _AddServerIntent();
}

class _ModelsCategoryState extends ConsumerState<ModelsCategory> {
  /// The chosen row: a name, [serverAddRow], or null for the default.
  String? _selected;

  /// On a narrow page: whether the detail view stands in place of the list.
  bool _pushed = false;

  /// The row somebody chose while the form held unsaved changes.
  String? _pending;

  /// Whether the detail view holds anything "Save" would write. Told by the
  /// detail view itself; no rebuild hangs on it.
  bool _dirty = false;

  final GlobalKey<ServerListState> _list = GlobalKey<ServerListState>();
  final FocusNode _detailEntry = FocusNode(debugLabel: 'server-detail-entry');

  /// The jump that chose a server and still has to land.
  String? _landing;

  /// The row the detail view shows, as last built; a click on it only
  /// pushes the detail on a narrow page.
  String? _visible;

  /// The row the current detail view was built for. When the page shows
  /// another row, [_serial] counts up and the form starts fresh; a rename
  /// keeps the same form (`onSaved` moves this along).
  String? _lastShown;

  /// Counts the detail views; the key of the current one.
  int _serial = 0;

  /// The chosen server as the daemon last listed it, for the draft that
  /// stays when another client removes the server (HUM-414, review).
  SavedServer? _snapshot;

  /// The frame of list and detail, for the list that stays in view.
  final GlobalKey _frame = GlobalKey(debugLabel: 'server-frame');

  /// The height of the list beside the detail view, as last laid out; the
  /// row keeps this much room for it, since the list itself stands on top
  /// so it can stay in view.
  double _listHeight = 0;

  /// What runs once a pending [_away] was answered with save or discard.
  VoidCallback? _proceed;

  /// The jump that waits for the answer to the pending choice.
  String? _revealAfter;

  SettingsController? _scope;

  /// [_guard] as one object, so the scope can tell it apart.
  late final UnsavedGuard _guardFn = _guard;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final SettingsController? scope = SettingsScope.maybeOf(context);
    if (scope != _scope) {
      _scope?.releaseUnsaved(_guardFn);
      _scope = scope?..guardUnsaved(_guardFn);
    }
  }

  /// The guard of the overlay: closing it or choosing another category
  /// while the form holds a change asks the same question as another row.
  bool _guard(VoidCallback proceed) {
    if (!_dirty || !mounted) {
      return false;
    }
    setState(() {
      _pending = _away;
      _proceed = proceed;
    });
    return true;
  }

  @override
  void dispose() {
    _scope?.releaseUnsaved(_guardFn);
    _detailEntry.dispose();
    super.dispose();
  }

  /// The row the page shows: the chosen one while it exists, otherwise the
  /// default, otherwise the first; null without any server.
  String? _effective(SavedServers list) {
    final String? chosen = _selected;
    if (chosen == serverAddRow ||
        (chosen != null && list.named(chosen) != null)) {
      return chosen;
    }
    final List<SavedServer> ordered = serversInListOrder(list);
    return ordered.isEmpty ? null : ordered.first.name;
  }

  void _choose(String target, SavedServers list) {
    if (target == _visible && target != _backToList) {
      setState(() => _pushed = true);
      return;
    }
    if (_dirty) {
      setState(() => _pending = target);
      return;
    }
    _go(target);
  }

  void _go(String target) {
    final VoidCallback? proceed = _proceed;
    final String? reveal = _revealAfter;
    setState(() {
      _pending = null;
      _dirty = false;
      _proceed = null;
      _revealAfter = null;
      if (target == _away) {
        // Die Seite bleibt, bis der Aufrufer geht.
      } else if (target == _backToList) {
        _pushed = false;
      } else {
        _selected = target;
        _pushed = true;
      }
    });
    if (target == _away) {
      proceed?.call();
    }
    if (reveal != null) {
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        if (mounted) {
          SettingsScope.maybeOf(context)?.reveal(reveal);
          _landing = null;
        }
      });
    }
  }

  /// "Keep editing": the pending choice is dropped, and so is a jump that
  /// waited on it.
  void _stay() {
    if (_revealAfter != null) {
      SettingsScope.maybeOf(context)?.want(null);
    }
    setState(() {
      _pending = null;
      _proceed = null;
      _revealAfter = null;
      _landing = null;
    });
  }

  void _enter(String target, SavedServers list) {
    _choose(target, list);
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted && _pending == null) {
        _detailEntry.requestFocus();
      }
    });
  }

  /// The detail view closed itself: after "Cancel" of a new server or
  /// after a removal. The page goes back to the default.
  void _closed() => setState(() {
    _selected = null;
    _pending = null;
    _dirty = false;
    _pushed = false;
  });

  /// Chooses the server a jump to [id] needs, once the list is known.
  void _follow(String id, SavedServers list) {
    if (_landing == id) {
      return;
    }
    _landing = id;
    final SavedServer? fallback = list.activeServer;
    final String target = fallback == null ? serverAddRow : fallback.name;
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (!mounted) {
        return;
      }
      // Hält das Formular eines anderen Servers einen Entwurf, fragt es
      // erst; der Sprung landet nach der Antwort.
      if (_dirty && target != _visible) {
        setState(() {
          _pending = target;
          _revealAfter = id;
        });
        return;
      }
      setState(() {
        _selected = target;
        _pushed = true;
        _pending = null;
      });
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        if (!mounted) {
          return;
        }
        SettingsScope.maybeOf(context)?.reveal(id);
        _landing = null;
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AsyncValue<SavedServers> servers = ref.watch(llmServersProvider);
    final SavedServers list = servers.value ?? SavedServers.none;
    final SettingsController? scope = SettingsScope.maybeOf(context);
    final String? wanted = scope?.wanted;
    final bool known = servers.hasValue || servers.hasError;
    if (wanted != null && serverFormIds.contains(wanted) && known) {
      _follow(wanted, list);
    }
    Widget anchored(String id, Widget child) => scope == null
        ? child
        : KeyedSubtree(
            key: scope.rowKey(id),
            child: Focus(focusNode: scope.anchor(id), child: child),
          );

    // Hält das Formular einen Entwurf für einen Server, den ein anderer
    // Client entfernt hat, bleibt er stehen; die Ansicht sagt es und bietet
    // Verwerfen und Als-neu-Speichern an.
    final String? chosen = _selected;
    final SavedServer? ghost =
        chosen != null &&
            chosen != serverAddRow &&
            servers.hasValue &&
            list.named(chosen) == null &&
            _dirty &&
            _snapshot?.name == chosen
        ? _snapshot
        : null;
    final String? shown = ghost?.name ?? _effective(list);
    if (shown != _lastShown) {
      _serial++;
      _lastShown = shown;
    }
    _visible = shown;
    if (ghost == null && shown != null && shown != serverAddRow) {
      _snapshot = list.named(shown);
    }
    return Shortcuts(
      shortcuts: withCommandOnMac(const <ShortcutActivator, Intent>{
        SingleActivator(LogicalKeyboardKey.keyN, control: true):
            _AddServerIntent(),
      }),
      child: Actions(
        actions: <Type, Action<Intent>>{
          _AddServerIntent: CallbackAction<_AddServerIntent>(
            onInvoke: (_AddServerIntent _) {
              _choose(serverAddRow, list);
              return null;
            },
          ),
        },
        child: Padding(
          padding: EdgeInsets.only(bottom: tokens.spacing.x4),
          child: Semantics(
            container: true,
            label: l10n.settingsServersLabel,
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: SettingRowGrid.inset,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  anchored(
                    SettingIds.llmServers,
                    Column(
                      key: const ValueKey<String>(
                        'setting-row-${SettingIds.llmServers}',
                      ),
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        ServerSectionHead(
                          title: l10n.settingsServersSection,
                          help: l10n.settingsServersIntro,
                        ),
                        SizedBox(height: tokens.spacing.x3),
                        if (shown == null)
                          ServerEmptyState(
                            onAdd: () => _choose(serverAddRow, list),
                          )
                        else
                          LayoutBuilder(
                            builder:
                                (BuildContext context, BoxConstraints box) =>
                                    _servers(
                                      context,
                                      list,
                                      shown,
                                      ghost: ghost,
                                      sideBySide:
                                          box.maxWidth >= serverListSideBySide,
                                    ),
                          ),
                      ],
                    ),
                  ),
                  if (list.servers.isNotEmpty) ...<Widget>[
                    SizedBox(height: tokens.spacing.x8),
                    anchored(
                      SettingIds.llmServerUsage,
                      KeyedSubtree(
                        key: const ValueKey<String>(
                          'setting-row-${SettingIds.llmServerUsage}',
                        ),
                        child: ServerUsage(servers: list),
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// The list and the detail view of [shown], side by side or in turns.
  Widget _servers(
    BuildContext context,
    SavedServers list,
    String shown, {
    required bool sideBySide,
    SavedServer? ghost,
  }) {
    final HTokens tokens = HTheme.of(context);
    final Widget listView = FocusTraversalGroup(
      child: Padding(
        padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
        child: ServerList(
          key: _list,
          servers: list,
          selected: shown,
          onSelect: (String name) => _choose(name, list),
          onEnter: (String name) => _enter(name, list),
        ),
      ),
    );
    final Widget detail = FocusTraversalGroup(
      child: Focus(
        canRequestFocus: false,
        skipTraversal: true,
        onKeyEvent: (FocusNode _, KeyEvent event) {
          if (event is! KeyDownEvent ||
              event.logicalKey != LogicalKeyboardKey.escape) {
            return KeyEventResult.ignored;
          }
          if (sideBySide) {
            _list.currentState?.focusSelected();
          } else {
            _choose(_backToList, list);
          }
          return KeyEventResult.handled;
        },
        child: Padding(
          padding: EdgeInsets.symmetric(vertical: tokens.spacing.x3),
          child: _detail(list, shown, sideBySide: sideBySide, ghost: ghost),
        ),
      ),
    );
    final BoxDecoration frame = BoxDecoration(
      border: Border.all(color: tokens.colors.line),
      borderRadius: BorderRadius.circular(tokens.radii.card),
    );
    if (!sideBySide) {
      return DecoratedBox(
        decoration: frame,
        child: ClipRRect(
          borderRadius: BorderRadius.circular(tokens.radii.card),
          child: _pushed ? detail : listView,
        ),
      );
    }
    return DecoratedBox(
      decoration: frame,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(tokens.radii.card),
        child: Stack(
          key: _frame,
          children: <Widget>[
            // Die Trennlinie reicht über die ganze Höhe, auch wenn die Liste
            // länger ist als die Detailansicht.
            Positioned(
              left: serverListWidth,
              top: 0,
              bottom: 0,
              width: HSize.hairline,
              child: ColoredBox(color: tokens.colors.line),
            ),
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                SizedBox(width: serverListWidth, height: _listHeight),
                const SizedBox(width: HSize.hairline),
                Expanded(child: detail),
              ],
            ),
            StickyPane(
              frame: _frame,
              width: serverListWidth,
              child: SizeReport(
                onHeight: (double height) {
                  if (mounted && height != _listHeight) {
                    setState(() => _listHeight = height);
                  }
                },
                child: listView,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _detail(
    SavedServers list,
    String shown, {
    required bool sideBySide,
    SavedServer? ghost,
  }) {
    final SavedServer? server = shown == serverAddRow
        ? null
        : ghost ?? list.named(shown);
    return ServerDetail(
      key: ValueKey<String>('detail-$_serial'),
      server: server,
      removedElsewhere: ghost != null,
      list: list,
      entryFocus: _detailEntry,
      leaving: _pending != null,
      onLeave: () {
        final String? target = _pending;
        if (target != null) {
          _go(target);
        }
      },
      onStay: _stay,
      onDirtyChanged: (bool dirty) => _dirty = dirty,
      onBack: sideBySide ? null : () => _choose(_backToList, list),
      onClose: _closed,
      onSaved: (String name) => setState(() {
        // Ein neuer Server und ein Entwurf, der als neuer Server ging,
        // bekommen eine frische Ansicht; eine Umbenennung behält die ihre.
        final bool fresh = server == null || ghost != null;
        _selected = name;
        _dirty = false;
        if (!fresh) {
          _lastShown = name;
        }
      }),
    );
  }
}
