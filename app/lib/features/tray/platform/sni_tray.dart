/// The tray port on StatusNotifierItem (HUM-034).
///
/// Every Linux desktop that still has a tray speaks this protocol: KDE and
/// the GNOME AppIndicator extension natively, XFCE, Cinnamon, Budgie and the
/// Wayland panels through their own hosts. It is D-Bus and nothing else, so
/// this file exports two objects -- the item and its menu -- and registers
/// them with `org.kde.StatusNotifierWatcher`.
///
/// Why not `tray_manager`: that package binds `libayatana-appindicator3`, and
/// without the development package installed `flutter build linux` fails
/// before it reaches Dart. A protocol this small does not justify a build
/// dependency that can stop a build on a machine that never wanted a tray.
///
/// What a Linux desktop cannot do, and what is therefore not attempted here:
///
/// * **GNOME without the AppIndicator extension has no tray at all.** There
///   is no fallback and no second mechanism; the registration fails and this
///   port reports it once, with the extension as the fix. The count still
///   stands in the window title, which every desktop shows.
/// * **A tray icon carries no text of its own.** The number is drawn into the
///   image (`tray_icon.dart`); what it counts is said in the tooltip and in
///   the menu, because a tooltip is the only text a tray is allowed.
/// * **Whether the panel behind the icon is dark or light is not knowable.**
///   The icon therefore carries its own area colour instead of assuming one.
///
/// Every catch below is `on Object catch` and not `on Exception catch`: the
/// `dbus` package throws bare strings in several places -- among them
/// `D-Bus address transport not supported`, which is what an address of the
/// form `autolaunch:` produces -- and a `String` is not an `Exception`. With
/// the narrower catch that throw travels out of [SniTrayPort.start], through
/// the post-frame callback that starts it, and into the zone: no `UI_002`,
/// and a person on a desktop without a session bus is told nothing at all.
library;

import 'dart:async';
import 'dart:io' show pid;

import 'package:dbus/dbus.dart';

import '../../../core/domain/domain.dart';
import '../desktop_ports.dart';
import '../tray_diagnostics.dart';
import '../tray_icon.dart';

/// The watcher every host registers itself with.
const String _watcherName = 'org.kde.StatusNotifierWatcher';

/// Its object path.
final DBusObjectPath _watcherPath = DBusObjectPath('/StatusNotifierWatcher');

/// The interface of the item.
const String _itemInterface = 'org.kde.StatusNotifierItem';

/// The interface of the menu.
const String _menuInterface = 'com.canonical.dbusmenu';

/// The path the item is exported on.
final DBusObjectPath _itemPath = DBusObjectPath('/StatusNotifierItem');

/// The path the menu is exported on.
final DBusObjectPath _menuPath = DBusObjectPath('/StatusNotifierItem/Menu');

/// The tray over StatusNotifierItem.
class SniTrayPort implements TrayPort {
  /// Creates a port that talks to [bus].
  ///
  /// The caller owns [bus] when it passes one; otherwise the port opens the
  /// session bus itself and closes it again in [dispose].
  SniTrayPort({DBusClient? bus})
    : _bus = bus ?? DBusClient.session(),
      _ownsBus = bus == null;

  final DBusClient _bus;
  final bool _ownsBus;
  final StreamController<TrayCommand> _commands =
      StreamController<TrayCommand>.broadcast();

  late final _TrayItem _item = _TrayItem(onCommand: _emit);
  late final _TrayMenu _menu = _TrayMenu(onCommand: _emit);

  StreamSubscription<DBusNameOwnerChangedEvent>? _watcher;
  bool _registered = false;
  TrayFace? _drawn;

  /// The bus name of this item, unique per process.
  String get _busName => 'org.kde.StatusNotifierItem-$pid-1';

  @override
  Stream<TrayCommand> get commands => _commands.stream;

  @override
  Future<Diagnostic?> start() async {
    try {
      await _bus.registerObject(_item);
      await _bus.registerObject(_menu);
      await _bus.requestName(_busName);
    } on Object catch (error) {
      return _unavailable('the session bus refused the tray item: $error');
    }
    // A host that arrives later -- the extension switched on while the
    // program runs -- gets the item without a restart. Once per ownership
    // change, so this is not the retry spam HUM-034 rules out.
    _watcher = _bus.nameOwnerChanged.listen((DBusNameOwnerChangedEvent event) {
      if (event.name == _watcherName && event.newOwner != null) {
        unawaited(_register());
      }
    });
    if (await _register()) {
      return null;
    }
    return _unavailable(
      'no $_watcherName answers on the session bus; '
      'GNOME needs the AppIndicator extension',
    );
  }

  @override
  Future<void> show(TrayFace face) async {
    if (face == _drawn) {
      return;
    }
    _drawn = face;
    _item.pixmaps = await renderTrayIcon(state: face.state, count: face.count);
    _item.face = face;
    _menu.face = face;
    if (!_registered) {
      return;
    }
    try {
      await _item.emitSignal(_itemInterface, 'NewIcon');
      await _item.emitSignal(_itemInterface, 'NewToolTip');
      await _item.emitSignal(_itemInterface, 'NewStatus', <DBusValue>[
        DBusString(_item.status),
      ]);
      await _menu.announce();
    } on Object {
      // The host went away. The next face tries again; nothing here is worth
      // an error on the screen.
    }
  }

  @override
  Future<void> dispose() async {
    await _watcher?.cancel();
    await _commands.close();
    if (_ownsBus) {
      await _bus.close();
    }
  }

  void _emit(TrayCommand command) {
    if (!_commands.isClosed) {
      _commands.add(command);
    }
  }

  Future<bool> _register() async {
    try {
      await _bus.callMethod(
        destination: _watcherName,
        path: _watcherPath,
        interface: _watcherName,
        name: 'RegisterStatusNotifierItem',
        values: <DBusValue>[DBusString(_busName)],
        replySignature: DBusSignature(''),
      );
      _registered = true;
      return true;
    } on Object {
      return false;
    }
  }

  static Diagnostic _unavailable(String why) =>
      TrayDiagnostics.trayUnavailable(why);
}

/// The exported `org.kde.StatusNotifierItem`.
class _TrayItem extends DBusObject {
  _TrayItem({required this.onCommand}) : super(_itemPath);

  final void Function(TrayCommand command) onCommand;

  /// What the icon shows; null until the first face arrives.
  TrayFace? face;

  /// The rendered icon in every size.
  List<TrayPixmap> pixmaps = const <TrayPixmap>[];

  /// `NeedsAttention` is the one thing a host may treat louder than normal,
  /// and a hold that ran out unnoticed is the one thing that deserves it.
  String get status =>
      face?.state == TrayIconState.alert ? 'NeedsAttention' : 'Active';

  @override
  Future<DBusMethodResponse> handleMethodCall(DBusMethodCall call) async {
    if (call.interface != _itemInterface) {
      return DBusMethodErrorResponse.unknownInterface();
    }
    switch (call.name) {
      case 'Activate':
      case 'SecondaryActivate':
        onCommand(TrayCommand.show);
        return DBusMethodSuccessResponse();
      case 'ContextMenu':
      case 'Scroll':
        // The menu is a `com.canonical.dbusmenu` object; a host that asks the
        // item to open one does not get a second, different menu here.
        return DBusMethodSuccessResponse();
      default:
        return DBusMethodErrorResponse.unknownMethod();
    }
  }

  @override
  Future<DBusMethodResponse> getProperty(String interface, String name) async {
    if (interface != _itemInterface) {
      return DBusMethodErrorResponse.unknownProperty();
    }
    final DBusValue? value = _properties()[name];
    return value == null
        ? DBusMethodErrorResponse.unknownProperty()
        : DBusGetPropertyResponse(value);
  }

  @override
  Future<DBusMethodResponse> getAllProperties(String interface) async {
    if (interface != _itemInterface) {
      return DBusGetAllPropertiesResponse(const <String, DBusValue>{});
    }
    return DBusGetAllPropertiesResponse(_properties());
  }

  Map<String, DBusValue> _properties() => <String, DBusValue>{
    'Category': const DBusString('ApplicationStatus'),
    'Id': const DBusString('humanitl'),
    'Title': const DBusString('Humanitl'),
    'Status': DBusString(status),
    'WindowId': const DBusUint32(0),
    'IconName': const DBusString(''),
    'IconPixmap': _pixmapArray(),
    'OverlayIconName': const DBusString(''),
    'OverlayIconPixmap': _emptyPixmaps(),
    'AttentionIconName': const DBusString(''),
    'AttentionIconPixmap': _pixmapArray(),
    'AttentionMovieName': const DBusString(''),
    'ToolTip': DBusStruct(<DBusValue>[
      const DBusString(''),
      _emptyPixmaps(),
      DBusString(face?.title ?? ''),
      DBusString(face?.detail ?? ''),
    ]),
    // False: a left click shows the window, the menu belongs to the right
    // one. An item that is a menu has no way left to bring a window forward.
    'ItemIsMenu': const DBusBoolean(false),
    'Menu': _menuPath,
  };

  DBusArray _pixmapArray() => DBusArray(
    DBusSignature('(iiay)'),
    pixmaps.map(
      (TrayPixmap pixmap) => DBusStruct(<DBusValue>[
        DBusInt32(pixmap.width),
        DBusInt32(pixmap.height),
        DBusArray.byte(pixmap.argb),
      ]),
    ),
  );

  static DBusArray _emptyPixmaps() =>
      DBusArray(DBusSignature('(iiay)'), const <DBusValue>[]);
}

/// Which entry of the menu is which.
///
/// Ids are stable, because a host caches them: the informational line keeps
/// its id when its text changes, so the panel updates the text instead of
/// rebuilding the menu under the pointer.
const int _menuRoot = 0;
const int _menuShow = 1;
const int _menuSeparator = 2;
const int _menuCount = 3;
const int _menuQuit = 4;

/// The exported `com.canonical.dbusmenu`.
class _TrayMenu extends DBusObject {
  _TrayMenu({required this.onCommand}) : super(_menuPath);

  final void Function(TrayCommand command) onCommand;

  /// What the menu says; null until the first face arrives.
  TrayFace? face;

  int _revision = 1;

  /// Tells the host that the labels changed.
  Future<void> announce() async {
    _revision++;
    await emitSignal(_menuInterface, 'LayoutUpdated', <DBusValue>[
      DBusUint32(_revision),
      const DBusInt32(_menuRoot),
    ]);
  }

  @override
  Future<DBusMethodResponse> handleMethodCall(DBusMethodCall call) async {
    if (call.interface != _menuInterface) {
      return DBusMethodErrorResponse.unknownInterface();
    }
    switch (call.name) {
      case 'GetLayout':
        return DBusMethodSuccessResponse(<DBusValue>[
          DBusUint32(_revision),
          _layout(),
        ]);
      case 'GetGroupProperties':
        return DBusMethodSuccessResponse(<DBusValue>[_groupProperties()]);
      case 'GetProperty':
        return _property(call);
      case 'Event':
        _event(call);
        return DBusMethodSuccessResponse();
      case 'EventGroup':
        return DBusMethodSuccessResponse(<DBusValue>[
          DBusArray(DBusSignature('i'), const <DBusValue>[]),
        ]);
      case 'AboutToShow':
        return DBusMethodSuccessResponse(<DBusValue>[const DBusBoolean(false)]);
      case 'AboutToShowGroup':
        return DBusMethodSuccessResponse(<DBusValue>[
          DBusArray(DBusSignature('i'), const <DBusValue>[]),
          DBusArray(DBusSignature('i'), const <DBusValue>[]),
        ]);
      default:
        return DBusMethodErrorResponse.unknownMethod();
    }
  }

  @override
  Future<DBusMethodResponse> getProperty(String interface, String name) async {
    if (interface != _menuInterface) {
      return DBusMethodErrorResponse.unknownProperty();
    }
    return switch (name) {
      'Version' => DBusGetPropertyResponse(const DBusUint32(3)),
      'TextDirection' => DBusGetPropertyResponse(const DBusString('ltr')),
      'Status' => DBusGetPropertyResponse(const DBusString('normal')),
      'IconThemePath' => DBusGetPropertyResponse(
        DBusArray(DBusSignature('s'), const <DBusValue>[]),
      ),
      _ => DBusMethodErrorResponse.unknownProperty(),
    };
  }

  @override
  Future<DBusMethodResponse> getAllProperties(String interface) async {
    if (interface != _menuInterface) {
      return DBusGetAllPropertiesResponse(const <String, DBusValue>{});
    }
    return DBusGetAllPropertiesResponse(<String, DBusValue>{
      'Version': const DBusUint32(3),
      'TextDirection': const DBusString('ltr'),
      'Status': const DBusString('normal'),
      'IconThemePath': DBusArray(DBusSignature('s'), const <DBusValue>[]),
    });
  }

  void _event(DBusMethodCall call) {
    if (call.values.length < 2) {
      return;
    }
    final DBusValue id = call.values[0];
    final DBusValue kind = call.values[1];
    if (id is! DBusInt32 || kind is! DBusString || kind.value != 'clicked') {
      return;
    }
    switch (id.value) {
      case _menuShow:
        onCommand(TrayCommand.show);
      case _menuQuit:
        onCommand(TrayCommand.quit);
      default:
        // The informational line is disabled; a host that sends a click for
        // it anyway gets nothing, which is what "disabled" means.
        break;
    }
  }

  DBusMethodResponse _property(DBusMethodCall call) {
    if (call.values.length < 2) {
      return DBusMethodErrorResponse.invalidArgs();
    }
    final DBusValue id = call.values[0];
    final DBusValue name = call.values[1];
    if (id is! DBusInt32 || name is! DBusString) {
      return DBusMethodErrorResponse.invalidArgs();
    }
    final DBusValue? value = _entryProperties(id.value)[name.value];
    return value == null
        ? DBusMethodErrorResponse.invalidArgs()
        : DBusMethodSuccessResponse(<DBusValue>[DBusVariant(value)]);
  }

  DBusStruct _layout() => DBusStruct(<DBusValue>[
    const DBusInt32(_menuRoot),
    DBusDict.stringVariant(_entryProperties(_menuRoot)),
    DBusArray(
      DBusSignature('v'),
      <int>[_menuShow, _menuSeparator, _menuCount, _menuQuit].map(
        (int id) => DBusVariant(
          DBusStruct(<DBusValue>[
            DBusInt32(id),
            DBusDict.stringVariant(_entryProperties(id)),
            DBusArray(DBusSignature('v'), const <DBusValue>[]),
          ]),
        ),
      ),
    ),
  ]);

  DBusArray _groupProperties() => DBusArray(
    DBusSignature('(ia{sv})'),
    <int>[_menuRoot, _menuShow, _menuSeparator, _menuCount, _menuQuit].map(
      (int id) => DBusStruct(<DBusValue>[
        DBusInt32(id),
        DBusDict.stringVariant(_entryProperties(id)),
      ]),
    ),
  );

  Map<String, DBusValue> _entryProperties(int id) => switch (id) {
    _menuRoot => <String, DBusValue>{
      'children-display': const DBusString('submenu'),
    },
    _menuShow => <String, DBusValue>{
      'label': DBusString(face?.menuShow ?? ''),
      'enabled': const DBusBoolean(true),
      'visible': const DBusBoolean(true),
    },
    _menuSeparator => <String, DBusValue>{
      'type': const DBusString('separator'),
      'visible': const DBusBoolean(true),
    },
    // Informative and nothing else: the tray is a state display, not a
    // control (`docs/UX.md` 8, HUM-034).
    _menuCount => <String, DBusValue>{
      'label': DBusString(face?.title ?? ''),
      'enabled': const DBusBoolean(false),
      'visible': const DBusBoolean(true),
    },
    _menuQuit => <String, DBusValue>{
      'label': DBusString(face?.menuQuit ?? ''),
      'enabled': const DBusBoolean(true),
      'visible': const DBusBoolean(true),
    },
    _ => const <String, DBusValue>{},
  };
}
