/// The settings, as an overlay from below with categories and a search
/// (HUM-404; before that a sheet from the right, HUM-069).
///
/// # Shape
///
/// A panel over three quarters of the window height, the full width but a
/// margin, the application dimmed behind it. On top: the title, the search
/// field and the close button. Below: the categories in a column on the
/// left ([SettingsNav]) and the rows of the chosen one on the right
/// ([SettingsCategoryPage]); while the search holds text, the hits replace
/// the rows ([SettingsSearchResults]). What stands where comes from one
/// register (`registry/settings_registry.dart`).
///
/// # Keys
///
/// `Tab` goes search, categories, page, close button; the arrow keys move
/// between the categories. `Ctrl+F` puts the cursor into the search, and so
/// does typing anywhere outside a text field. `Esc` empties the search
/// first and closes the overlay after that, as does a click on the dimmed
/// application and the close button. The overlay holds the keyboard while it
/// is open (`HLayerScope`) and gives it back when it closes.
///
/// # Motion
///
/// It slides in from below over [HMotion.arrive] on the `enter` curve and
/// out on `exit`; under reduced motion it only fades.
///
/// # Writing
///
/// A switch or a choice is written at once through `SetConfig`, and the row
/// says "Saved" for a moment; a refusal of the daemon stands at the row
/// instead. Endpoint, model and key stand in the detail view of a server
/// (`ServerDetail`, HUM-405).
/// The overlay decides nothing about what a value means (ADR-018).
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/config_providers.dart';
import '../../core/ipc/connection.dart';
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

/// Puts the cursor into the search field of the overlay.
class _FocusSearchIntent extends Intent {
  const _FocusSearchIntent();
}

/// The settings overlay.
class SettingsOverlay extends ConsumerStatefulWidget {
  /// Creates the overlay; [onClose] asks to close it, [onClosed] reports
  /// that it has slid out and may be taken away.
  const SettingsOverlay({
    required this.onClose,
    this.onClosed,
    this.closing = false,
    this.focusKey,
    this.focusSerial = 0,
    this.daemonRestart,
    super.key,
  });

  /// Asks the owner to close the overlay; the owner then sets [closing].
  final VoidCallback onClose;

  /// Invoked once the overlay has slid out after [closing] turned true.
  final VoidCallback? onClosed;

  /// Whether the overlay is on its way out.
  final bool closing;

  /// The setting the overlay opens at, or null (HUM-275): a diagnostic's
  /// `ChangeSetting` fix names its key. The overlay opens its category,
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

  /// The share of the window height the panel covers.
  static const double heightFraction = 0.75;

  /// The margin left and right of the panel.
  static const double sideMargin = HSpace.x4;

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
  ConsumerState<SettingsOverlay> createState() => _SettingsOverlayState();
}

class _SettingsOverlayState extends ConsumerState<SettingsOverlay>
    with SingleTickerProviderStateMixin {
  late final AnimationController _motion = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  )..addStatusListener(_motionChanged);

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _motion,
    curve: HMotion.enter,
    reverseCurve: HMotion.exit,
  );

  late final SettingsController _settings = SettingsController(
    (String key, String value) =>
        ref.read(configProvider.notifier).write(key, value),
  );

  final TextEditingController _query = TextEditingController();
  final FocusNode _searchNode = FocusNode(debugLabel: 'settings-search');
  final ScrollController _scroll = ScrollController();

  SettingsCategory _category = SettingsCategory.values.first;

  /// The setting a jump wants to land on until its row stands (HUM-275).
  ///
  /// A fix can open the overlay before `GetConfig` or `GetInfo` answered;
  /// then the row comes later. The target waits for it, and every build
  /// tries again, because the overlay rebuilds as soon as either answers.
  /// It goes once the row was revealed, or when the daemon really does not
  /// offer the setting.
  String? _pending;

  /// Whether a try for [_pending] is already queued.
  bool _tryQueued = false;

  @override
  void initState() {
    super.initState();
    _query.addListener(_queryChanged);
    if (widget.closing) {
      _motion.value = 0;
    } else {
      unawaited(_motion.forward());
    }
    if (!_jumpTo(widget.focusKey)) {
      _afterLayer(_searchNode.requestFocus);
    }
  }

  @override
  void didUpdateWidget(SettingsOverlay oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.closing != oldWidget.closing) {
      if (widget.closing) {
        // Auf dem Weg hinaus nimmt kein Sprungziel mehr die Tastatur.
        _pending = null;
        _settings.want(null);
        unawaited(_motion.reverse());
      } else {
        unawaited(_motion.forward());
      }
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
    _curve.dispose();
    _motion.dispose();
    _settings.dispose();
    super.dispose();
  }

  void _motionChanged(AnimationStatus status) {
    if (status != AnimationStatus.dismissed || !widget.closing) {
      return;
    }
    // Nach dem Bild und nicht im Rückruf: Ohne Weg steht die Bewegung schon
    // im Bau des Bildes am Ende, und der Besitzer darf dann nicht bauen.
    WidgetsBinding.instance
      ..addPostFrameCallback((Duration _) {
        if (mounted && widget.closing && _motion.isDismissed) {
          widget.onClosed?.call();
        }
      })
      ..ensureVisualUpdate();
  }

  void _queryChanged() => setState(() {
    // Wer sucht, ist weiter als ein Sprungziel, das noch wartet.
    if (_query.text.isNotEmpty) {
      _pending = null;
      _settings.want(null);
    }
  });

  /// Opens the category of [key] and lets [_pending] wait for its row.
  /// False for a key the register does not know.
  bool _jumpTo(String? key) {
    final SettingEntry? entry = key == null ? null : settingEntryOrNull(key);
    if (entry == null) {
      return false;
    }
    _query.clear();
    _category = entry.category;
    _pending = entry.id;
    _settings.want(entry.id);
    _queueTry();
    return true;
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
    final DaemonInfo? info = ref.read(daemonInfoProvider).value;
    if (info != null && !settingEntry(id).shownFor(info)) {
      _pending = null;
      return;
    }
    // Eine Seite, die die Zeile erst nach einem eigenen Schritt zeichnet,
    // hat den Sprung schon selbst ausgeführt (HUM-405).
    if (_settings.wanted != id || _settings.reveal(id)) {
      _pending = null;
    }
  }

  /// Runs [action] once the layer around the overlay has taken the
  /// keyboard: after the frame that builds it, and behind the request of the
  /// layer itself, which is queued in the same frame (`HLayerScope`).
  void _afterLayer(VoidCallback action) {
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      scheduleMicrotask(() {
        if (mounted) {
          action();
        }
      });
    });
  }

  void _choose(SettingsCategory category) {
    if (_query.text.isEmpty && category == _category) {
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
  void _chooseHit(String id) => setState(() => _jumpTo(id));

  void _clearSearch() {
    _query.clear();
    _searchNode.requestFocus();
  }

  /// `Esc`: the search first, then the overlay.
  void _dismiss() {
    if (_query.text.isNotEmpty) {
      _clearSearch();
    } else {
      widget.onClose();
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
  void _submitSearch(AppLocalizations l10n) {
    final List<SettingsSearchGroup> groups = searchSettings(
      _query.text,
      l10n,
      ref.read(daemonInfoProvider).value,
    );
    if (groups.isNotEmpty) {
      _chooseHit(groups.first.entries.first.id);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool reduced = HReducedMotion.of(context);
    // Die Antworten, auf die ein Sprungziel wartet: Kommt eine, baut das
    // Overlay neu, und der nächste Versuch findet die Zeile.
    ref
      ..watch(configProvider)
      ..watch(daemonInfoProvider);
    if (_pending != null) {
      _queueTry();
    }
    return Shortcuts(
      shortcuts: const <ShortcutActivator, Intent>{
        SingleActivator(LogicalKeyboardKey.escape): DismissIntent(),
        SingleActivator(LogicalKeyboardKey.keyF, control: true):
            _FocusSearchIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          DismissIntent: CallbackAction<DismissIntent>(
            onInvoke: (DismissIntent _) {
              _dismiss();
              return null;
            },
          ),
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
        },
        child: HTheme.host(
          context,
          HLayerScope(
            child: Focus(
              canRequestFocus: false,
              skipTraversal: true,
              onKeyEvent: _typeToSearch,
              child: IgnorePointer(
                ignoring: widget.closing,
                child: Stack(
                  fit: StackFit.expand,
                  children: <Widget>[
                    FadeTransition(
                      opacity: _curve,
                      child: Semantics(
                        button: true,
                        label: l10n.settingsClose,
                        child: GestureDetector(
                          key: const Key('settings-dim'),
                          behavior: HitTestBehavior.opaque,
                          onTap: widget.onClose,
                          child: ColoredBox(
                            color: HColorDerivation.fade(
                              tokens.colors.bg0,
                              0.72,
                            ),
                          ),
                        ),
                      ),
                    ),
                    LayoutBuilder(
                      builder: (BuildContext context, BoxConstraints box) {
                        final Widget panel = _panel(
                          context,
                          tokens,
                          l10n,
                          width: box.maxWidth - 2 * SettingsOverlay.sideMargin,
                        );
                        return Align(
                          alignment: Alignment.bottomCenter,
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: SettingsOverlay.sideMargin,
                            ),
                            child: SizedBox(
                              height:
                                  box.maxHeight *
                                  SettingsOverlay.heightFraction,
                              child: reduced
                                  ? FadeTransition(
                                      opacity: _curve,
                                      child: panel,
                                    )
                                  : SlideTransition(
                                      position: Tween<Offset>(
                                        begin: const Offset(0, 1),
                                        end: Offset.zero,
                                      ).animate(_curve),
                                      child: panel,
                                    ),
                            ),
                          ),
                        );
                      },
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _panel(
    BuildContext context,
    HTokens tokens,
    AppLocalizations l10n, {
    required double width,
  }) {
    final double nav = width < SettingsOverlay.narrowBelow
        ? SettingsOverlay.navWidthNarrow
        : SettingsOverlay.navWidth;
    final AsyncValue<ConfigValues> config = ref.watch(configProvider);
    final LlmKeyStatus status =
        ref.watch(llmKeyProvider).value ?? LlmKeyStatus.unset;
    final ConfigValues? values = config.value;
    final String query = _query.text;
    final bool searching = query.trim().isNotEmpty;
    // Die linke Kante der Seite: Spalte, Linie und Rand der Seite. Dazu der
    // Einzug der Zeile, und auf dieser Kante beginnt auch das Suchfeld.
    final double textEdge = nav + HSize.hairline + tokens.spacing.x4;

    final Widget page = values == null
        ? Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: SettingRowGrid.inset,
            ),
            child: Text(
              config.hasError ? l10n.settingsLoadFailed : l10n.settingsLoading,
              key: const Key('settings-loading'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          )
        : searching
        ? SettingsSearchResults(
            query: query,
            groups: searchSettings(
              query,
              l10n,
              ref.watch(daemonInfoProvider).value,
            ),
            onChoose: _chooseHit,
            onClear: _clearSearch,
          )
        : SettingsCategoryPage(
            category: _category,
            data: SettingsPageData(
              config: values,
              status: status,
              daemonRestart: widget.daemonRestart,
            ),
          );

    return DecoratedBox(
      key: const Key('settings-overlay'),
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border.all(
          color: tokens.colors.lineStrong,
          width: HSize.hairline,
        ),
        borderRadius: const BorderRadius.vertical(
          top: Radius.circular(HRadius.card),
        ),
      ),
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(
          top: Radius.circular(HRadius.card),
        ),
        child: SettingsScope(
          controller: _settings,
          child: FocusTraversalGroup(
            policy: OrderedTraversalPolicy(),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                SettingsHeader(
                  textEdge: textEdge,
                  pageMaxWidth: SettingsOverlay.pageMaxWidth,
                  searchController: _query,
                  searchFocus: _searchNode,
                  onSubmitSearch: () => _submitSearch(l10n),
                  onClose: widget.onClose,
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
                                    : 'settings-scroll-${_category.name}',
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
                                    maxWidth: SettingsOverlay.pageMaxWidth,
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
        ),
      ),
    );
  }
}
