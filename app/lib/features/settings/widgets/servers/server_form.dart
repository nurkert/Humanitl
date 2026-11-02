/// The form of one saved model server, or of a new one: its fields, what
/// they would change, and the three gestures that reach the daemon, "Check",
/// "Save" and "Save as new" (HUM-405, split out in HUM-414).
///
/// # Nothing goes out while typing
///
/// Name, address, model and key stay here until [ServerForm.save]; nothing
/// is written on its own. [ServerForm.check] is the one gesture that reaches
/// the server before (HUM-069, HUM-140), and it goes to the address in the
/// field.
///
/// # The key goes only to its own server
///
/// The key field starts empty, never holds a stored key and is emptied after
/// every check and every save. A typed key belongs to the address it was
/// typed beside: editing the address empties it. A check sends it once to
/// that address, and a save stores it under this server's name, so the
/// daemon gives it to this server's origin and no other one. Without a typed
/// key, a check asks the daemon for this server's stored key, which it sends
/// only to this server's own origin (`ProbeLlmRequest.server`).
///
/// # A save writes the name first
///
/// The name is checked against the daemon's rule before anything is
/// written, then renamed first, so a refused name never follows a stored
/// address or key. If a later step is refused, the form says exactly what
/// was saved before it.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ipc/config_providers.dart' show ServerSavedWithoutKey;
import '../../../../core/ipc/daemon_client.dart';
import '../../../../core/ui/pasted_text.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/server_checks.dart';
import 'server_form_deps.dart';

export 'server_form_deps.dart';

/// The fields and the state of one server's form.
class ServerForm extends ChangeNotifier {
  /// A form over [server], or over a new server when it is null.
  ServerForm({required SavedServer? server, required SavedServers list})
    : _server = server,
      _list = list, // ignore: prefer_initializing_formals
      name = TextEditingController(text: server?.name ?? ''),
      endpoint = TextEditingController(text: server?.endpoint ?? ''),
      model = TextEditingController(text: server?.model ?? '') {
    _keyEndpoint = endpoint.text;
    endpoint
      ..addListener(_dropKeyOnEndpointEdit)
      ..addListener(_proposeName);
    for (final TextEditingController field in _fields) {
      field.addListener(notifyListeners);
    }
  }

  /// The name field.
  final TextEditingController name;

  /// The address field.
  final TextEditingController endpoint;

  /// The model field.
  final TextEditingController model;

  /// The key field; never filled from a stored key.
  final TextEditingController key = TextEditingController();

  SavedServer? _server;
  SavedServers _list;
  bool _disposed = false;
  late String _keyEndpoint;
  String _proposedName = '';

  List<TextEditingController> get _fields => <TextEditingController>[
    name,
    endpoint,
    model,
    key,
  ];

  /// A write or a removal is under way.
  bool busy = false;

  /// A check is under way.
  bool probing = false;

  /// "Remove key" is chosen; it happens on Save.
  bool removeKey = false;

  /// What the last check answered, or why it failed.
  LlmProbe? probe;

  /// Why the last check failed.
  Diagnostic? probeFailure;

  /// The daemon's last refusal of a write.
  Diagnostic? refusal;

  /// What a save wrote before a later step was refused, such as
  /// "Name, Endpoint"; null otherwise.
  String? partlySaved;

  /// The name of a new server that was saved while its key was refused; the
  /// form then offers only the key again (HUM-289).
  String? savedWithoutKey;

  /// What the daemon says about this server's key; never the key.
  LlmKeyStatus? keyStatus;

  /// The address another client gave this server while the form held a
  /// draft; the draft stays and the view says so.
  String? endpointElsewhere;

  /// The server shown; null for a new one.
  SavedServer? get server => _server;

  /// Whether the form adds a server.
  bool get isNew => _server == null;

  /// The address as it will be saved.
  String get endpointText => stripPasteDebris(endpoint.text);

  /// The name as it will be saved.
  String get nameText => name.text.trim();

  /// Whether a key was typed.
  bool get keyTyped => stripPasteDebris(key.text).isNotEmpty;

  bool get _nameChanged => !isNew && nameText != _server!.name;

  bool get _endpointChanged => endpointText != (_server?.endpoint ?? '');

  bool get _modelChanged => model.text.trim() != (_server?.model ?? '');

  /// The models to save: the default first, the others of the server after.
  List<String> get models {
    final String chosen = model.text.trim();
    return <String>[
      if (chosen.isNotEmpty) chosen,
      for (final String known in _server?.models ?? const <String>[])
        if (known != chosen) known,
    ];
  }

  /// Whether a key is stored for this server: what the daemon said, else
  /// whether a reference stands.
  bool get keyStored {
    final SavedServer? server = _server;
    return server != null && (keyStatus?.set ?? server.hasKey);
  }

  /// Whether Save takes the stored key away: the address gets another
  /// origin, and the daemon then drops the key stored for the old one.
  bool get keyGoesWithOrigin {
    final SavedServer? server = _server;
    return server != null &&
        keyStored &&
        !removeKey &&
        _endpointChanged &&
        !sameOrigin(endpointText, server.endpoint);
  }

  /// Whether the form holds anything a save would write.
  bool get dirty {
    if (isNew) {
      return savedWithoutKey != null
          ? keyTyped
          : nameText.isNotEmpty ||
                endpointText.isNotEmpty ||
                model.text.trim().isNotEmpty ||
                keyTyped;
    }
    return _nameChanged ||
        _endpointChanged ||
        _modelChanged ||
        keyTyped ||
        removeKey;
  }

  /// What is wrong with the typed name, or null. [removed] is true while
  /// the server was removed by another client and the draft would be saved
  /// as a new one.
  String? nameProblem(AppLocalizations l10n, {bool removed = false}) {
    final String typed = nameText;
    if (typed.isEmpty ||
        savedWithoutKey != null ||
        (!isNew && !_nameChanged && !removed)) {
      return null;
    }
    if (!serverNamePattern.hasMatch(typed)) {
      return l10n.settingsServerNameInvalid;
    }
    final bool taken =
        _list.named(typed) != null && (removed || typed != _server?.name);
    return taken ? l10n.settingsServerNameTaken(typed) : null;
  }

  /// Takes the server and the list the daemon listed now. An untouched form
  /// follows a change made elsewhere; a draft stays with its typed key, and
  /// [endpointElsewhere] names the new address instead.
  void update(SavedServer? server, SavedServers list) {
    final SavedServer? before = _server;
    _server = server;
    _list = list;
    if (before == null || server == null || before.name != server.name) {
      return;
    }
    final bool clean =
        endpoint.text == before.endpoint &&
        model.text.trim() == before.model &&
        nameText == before.name &&
        !keyTyped &&
        !removeKey;
    if (clean) {
      endpoint.text = server.endpoint;
      model.text = server.model;
      endpointElsewhere = null;
    } else if (server.endpoint != before.endpoint &&
        server.endpoint != endpointText) {
      endpointElsewhere = server.endpoint;
    }
  }

  /// A key typed for one address never goes out to the next one, not even
  /// through a check before the field loses focus.
  void _dropKeyOnEndpointEdit() {
    if (endpoint.text == _keyEndpoint) {
      return;
    }
    _keyEndpoint = endpoint.text;
    key.clear();
    probe = null;
    probeFailure = null;
  }

  /// A new server takes a name from its address while nobody typed one:
  /// read from the text alone, nothing goes out.
  void _proposeName() {
    if (!isNew || savedWithoutKey != null) {
      return;
    }
    final String typed = nameText;
    if (typed.isNotEmpty && typed != _proposedName) {
      return;
    }
    final String proposal = proposeServerName(endpointText);
    if (proposal == typed) {
      return;
    }
    _proposedName = proposal;
    name.text = proposal;
  }

  /// Toggles "Remove key"; it happens on Save.
  void toggleRemoveKey() {
    removeKey = !removeKey;
    if (removeKey) {
      key.clear();
    }
    notifyListeners();
  }

  /// Picks [chosen] as the model.
  void pickModel(String chosen) => model.text = chosen;

  /// Drops every change of an existing server.
  void cancel() {
    final SavedServer? server = _server;
    if (server == null) {
      return;
    }
    name.text = server.name;
    endpoint.text = server.endpoint;
    model.text = server.model;
    key.clear();
    removeKey = false;
    refusal = null;
    partlySaved = null;
    probe = null;
    probeFailure = null;
    endpointElsewhere = null;
    notifyListeners();
  }

  /// Asks the daemon about the key of this server.
  Future<void> loadKeyStatus(DaemonClient client) async {
    final SavedServer? server = _server;
    if (server == null) {
      return;
    }
    try {
      keyStatus = await client.llmKeyStatus(server: server.name);
      notifyListeners();
    } on DaemonException {
      // Die Zeile sagt dann nur, ob ein Verweis steht.
    }
  }

  /// "Check": the one gesture of the form that reaches the network.
  Future<void> check(ServerFormDeps deps) async {
    final String address = endpointText;
    if (address.isEmpty || probing) {
      return;
    }
    final String typed = stripPasteDebris(key.text);
    final SavedServer? server = _server;
    // Der hinterlegte Schlüssel nur für die gespeicherte Adresse dieses
    // Servers; der Daemon gibt ihn ohnehin nur an deren Herkunft.
    final bool stored =
        typed.isEmpty &&
        server != null &&
        server.hasKey &&
        !removeKey &&
        address == server.endpoint;
    probing = true;
    probeFailure = null;
    notifyListeners();
    try {
      probe = await deps.client.probeLlm(
        address,
        apiKey: typed.isEmpty ? null : typed,
        useStoredKey: stored,
        server: server?.name,
      );
      if (server != null) {
        deps.checks.record(
          server.name,
          ServerCheck.found(endpoint: address, models: probe!.models.length),
        );
      }
    } on DaemonException catch (error) {
      if (server != null) {
        deps.checks.record(
          server.name,
          ServerCheck.failed(endpoint: address, failure: error.diagnostic),
        );
      }
      probe = null;
      probeFailure = error.diagnostic;
    } finally {
      // Leer nach der Probe: Der Schlüssel ging einmal an diese Adresse und
      // wartet nicht auf die nächste Geste. Geleert wird nur der, der
      // mitging; das Feld ist während der Probe gesperrt.
      if (!_disposed && stripPasteDebris(key.text) == typed) {
        key.clear();
      }
      probing = false;
      notifyListeners();
    }
  }

  /// Writes the form; returns the name the server has afterwards, or null
  /// when nothing or not everything was saved. [onRenamed] runs right after
  /// a rename, before the next step, so the page keeps this form chosen.
  Future<String?> save(
    ServerFormDeps deps,
    AppLocalizations l10n, {
    ValueChanged<String>? onRenamed,
  }) async {
    if (busy || !dirty || nameProblem(l10n) != null) {
      return null;
    }
    final String typed = stripPasteDebris(key.text);
    busy = true;
    notifyListeners();
    try {
      final SavedServer? server = _server;
      if (server == null) {
        if (savedWithoutKey case final String saved) {
          await deps.servers.storeKey(saved, typed);
          return saved;
        }
        await deps.servers.add(
          name: nameText,
          endpoint: endpointText,
          models: models,
          key: typed,
        );
        return nameText;
      }
      return await _saveExisting(deps, l10n, server, typed, onRenamed);
    } on ServerSavedWithoutKey catch (error) {
      savedWithoutKey = error.name;
      refusal = error.diagnostic;
      return null;
    } on DaemonException catch (error) {
      refusal = error.diagnostic;
      return null;
    } finally {
      // Leer nach jedem Versuch, auch nach einer Absage.
      _clearKey();
      busy = false;
      notifyListeners();
    }
  }

  Future<String?> _saveExisting(
    ServerFormDeps deps,
    AppLocalizations l10n,
    SavedServer server,
    String typed,
    ValueChanged<String>? onRenamed,
  ) async {
    String current = server.name;
    final bool editEndpoint = _endpointChanged;
    final bool editModel = _modelChanged;
    final List<String> done = <String>[];
    try {
      if (_nameChanged) {
        final String newName = nameText;
        await deps.servers.rename(current, newName);
        deps.checks.renamed(current, newName);
        current = newName;
        done.add(l10n.settingsServerNameLabel);
        onRenamed?.call(newName);
      }
      if (editEndpoint || editModel) {
        await deps.servers.edit(
          current,
          endpoint: editEndpoint ? endpointText : null,
          models: editModel ? models : null,
        );
        if (editEndpoint) {
          done.add(l10n.settingsEndpointLabel);
        }
        if (editModel) {
          done.add(l10n.settingsModelLabel);
        }
      }
      // Nach der Adresse: Der Schlüssel gehört zu der Adresse, die jetzt im
      // Feld steht, und geht unter dem Namen dieses Servers in den Speicher.
      if (typed.isNotEmpty) {
        await deps.servers.storeKey(current, typed);
      } else if (removeKey) {
        await deps.keys.clear(server: current);
      }
    } on DaemonException catch (error) {
      if (done.isEmpty) {
        rethrow;
      }
      refusal = error.diagnostic;
      partlySaved = done.join(', ');
      return null;
    }
    refusal = null;
    partlySaved = null;
    removeKey = false;
    endpointElsewhere = null;
    return current;
  }

  /// Saves the draft of a server another client removed as a new server
  /// under the name in the field; returns that name, or null.
  Future<String?> saveAsNew(ServerFormDeps deps, AppLocalizations l10n) async {
    if (busy || nameText.isEmpty || nameProblem(l10n, removed: true) != null) {
      return null;
    }
    final String typed = stripPasteDebris(key.text);
    final String newName = nameText;
    busy = true;
    notifyListeners();
    try {
      await deps.servers.add(
        name: newName,
        endpoint: endpointText,
        models: models,
        key: typed,
      );
      return newName;
    } on ServerSavedWithoutKey catch (error) {
      refusal = error.diagnostic;
      return error.name;
    } on DaemonException catch (error) {
      refusal = error.diagnostic;
      return null;
    } finally {
      _clearKey();
      busy = false;
      notifyListeners();
    }
  }

  /// Runs [change], a write that is no part of the form, such as "Make
  /// default" or a removal; its refusal stands in the footer.
  Future<bool> run(Future<void> Function() change) async {
    if (busy) {
      return false;
    }
    busy = true;
    notifyListeners();
    try {
      await change();
      refusal = null;
      return true;
    } on DaemonException catch (error) {
      refusal = error.diagnostic;
      return false;
    } finally {
      busy = false;
      notifyListeners();
    }
  }

  /// Silent once the view is gone: a gesture may end after it.
  @override
  void notifyListeners() {
    if (!_disposed) {
      super.notifyListeners();
    }
  }

  /// Empties the key field unless the view is gone.
  void _clearKey() {
    if (!_disposed) {
      key.clear();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    endpoint
      ..removeListener(_dropKeyOnEndpointEdit)
      ..removeListener(_proposeName);
    for (final TextEditingController field in _fields) {
      field
        ..removeListener(notifyListeners)
        ..dispose();
    }
    super.dispose();
  }
}
