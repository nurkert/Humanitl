/// The configuration and the access key, as the whole app reads them
/// (`configProvider`, CONVENTIONS 3.9; HUM-069, HUM-140).
///
/// They live in `core/ipc` and not in the settings feature: the theme of the
/// shell, the notifications of the tray and the language of the window read
/// the same values the form writes, and no feature imports another one
/// (ARCHITECTURE 5).
///
/// Every value comes from the daemon (`GetConfig`); nothing here reads
/// `config.toml` or decides what a key means beyond its default (ADR-018).
/// While no daemon answers, a reader gets the defaults of [ConfigValues].
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../domain/domain.dart';
import 'client_providers.dart';
import 'connection.dart';
import 'daemon_client.dart';

part 'config_providers.g.dart';

/// Kein stiller Wiederholungsversuch von Riverpod: Beide Provider fragen
/// neu, wenn die Verbindung zurückkommt (`linkLiveProvider`), und ein Timer,
/// der gegen einen Daemon ohne Antwort weiterfragte, wäre ein zweiter
/// Herzschlag neben dem der Verbindung.
Duration? noConfigRetry(int retryCount, Object error) => null;

/// The configuration as the next start of the daemon loads it.
///
/// Asked again whenever the link to the daemon comes back: a daemon that
/// restarted reads `config.toml` anew, and so does this provider.
@Riverpod(keepAlive: true, name: 'configProvider', retry: noConfigRetry)
class ConfigSetting extends _$ConfigSetting {
  @override
  Future<ConfigValues> build() {
    ref.watch(linkLiveProvider);
    return ref.watch(daemonClientProvider).getConfig();
  }

  /// Asks the daemon again.
  Future<void> refresh() async {
    state = await AsyncValue.guard(
      () => ref.read(daemonClientProvider).getConfig(),
    );
  }

  /// Writes [value] under [key] (`SetConfig`) and reads the configuration
  /// again. A refusal of the daemon arrives as the `DaemonException` it
  /// threw; nothing was written then.
  ///
  /// [expectedServer] is the saved server whose view the value belongs to
  /// (HUM-289); the daemon refuses the write once another one is active.
  Future<void> write(String key, String value, {String? expectedServer}) async {
    await ref
        .read(daemonClientProvider)
        .setConfig(key, value, expectedServer: expectedServer);
    await refresh();
  }
}

/// Whether an access key is stored, and where (`LlmKey`, HUM-140).
///
/// The state never holds the key: [LlmKeyStatus] has no field for it.
@Riverpod(keepAlive: true, name: 'llmKeyProvider', retry: noConfigRetry)
class LlmKeySetting extends _$LlmKeySetting {
  @override
  Future<LlmKeyStatus> build() {
    ref.watch(linkLiveProvider);
    return ref.watch(daemonClientProvider).llmKeyStatus();
  }

  /// Stores [key] and points `llm.api_key_ref` at it. A refusal arrives as
  /// the `DaemonException` it threw, and the state stays as it was.
  ///
  /// With [server] the key belongs to that saved server (HUM-289); the state
  /// is then asked again, because it is the active server's.
  Future<LlmKeyStatus> store(String key, {String? server}) async {
    final LlmKeyStatus status = await ref
        .read(daemonClientProvider)
        .setLlmKey(key, server: server);
    _settle(status, server);
    return status;
  }

  /// Removes the key and the reference, of [server] when given.
  Future<LlmKeyStatus> clear({String? server}) async {
    final LlmKeyStatus status = await ref
        .read(daemonClientProvider)
        .clearLlmKey(server: server);
    _settle(status, server);
    return status;
  }

  void _settle(LlmKeyStatus status, String? server) {
    if (server == null) {
      state = AsyncValue<LlmKeyStatus>.data(status);
      return;
    }
    ref.invalidateSelf();
    ref.invalidate(llmServersProvider);
  }
}

/// The saved model servers and which one is active (`LlmServers`, HUM-289).
///
/// Read by the settings and by the status bar, so it lives here and not in
/// a feature. Every change asks the configuration and the key state again:
/// the active server is what `llm.endpoint`, `llm.models` and the key
/// without a name show. A switch applies to the next session; nothing here
/// touches a running one.
@Riverpod(keepAlive: true, name: 'llmServersProvider', retry: noConfigRetry)
class LlmServersSetting extends _$LlmServersSetting {
  @override
  Future<SavedServers> build() {
    ref.watch(linkLiveProvider);
    return ref.watch(daemonClientProvider).listLlmServers();
  }

  /// Saves a server and, when [key] is not empty, its key under its name.
  /// A refusal arrives as the `DaemonException` it threw.
  Future<SavedServers> add({
    required String name,
    required String endpoint,
    List<String> models = const <String>[],
    String key = '',
    bool activate = false,
  }) async {
    final DaemonClient client = ref.read(daemonClientProvider);
    SavedServers list = await client.addLlmServer(
      name: name,
      endpoint: endpoint,
      models: models,
      activate: activate,
    );
    if (key.isNotEmpty) {
      try {
        await client.setLlmKey(key, server: name);
      } on DaemonException catch (error) {
        // Der Server steht schon; nur der Schlüssel fehlt. Die Oberfläche
        // sagt das und bietet das Setzen erneut an, statt „gibt es schon".
        await _settled(await client.listLlmServers());
        throw ServerSavedWithoutKey(name, error.diagnostic);
      }
      list = await client.listLlmServers();
    }
    return _settled(list);
  }

  /// Stores [key] for the saved server [server] and asks again.
  Future<SavedServers> storeKey(String server, String key) async {
    final DaemonClient client = ref.read(daemonClientProvider);
    await client.setLlmKey(key, server: server);
    return _settled(await client.listLlmServers());
  }

  /// Renames a server; its key stays where it is.
  Future<SavedServers> rename(String name, String newName) async => _settled(
    await ref.read(daemonClientProvider).renameLlmServer(name, newName),
  );

  /// Removes a server, its keyring item with [deleteKey].
  Future<SavedServers> remove(
    String name, {
    bool deleteKey = false,
    String? replacement,
  }) async => _settled(
    await ref
        .read(daemonClientProvider)
        .removeLlmServer(name, deleteKey: deleteKey, replacement: replacement),
  );

  /// Makes a server the active one, from the next session on.
  Future<SavedServers> activate(String name) async =>
      _settled(await ref.read(daemonClientProvider).activateLlmServer(name));

  /// Takes [list] as the new state and asks for what follows from it.
  Future<SavedServers> _settled(SavedServers list) async {
    state = AsyncValue<SavedServers>.data(list);
    await ref.read(configProvider.notifier).refresh();
    ref.invalidate(llmKeyProvider);
    return list;
  }
}

/// The server was saved, and its key was not (HUM-289): the daemon refused
/// the key with [diagnostic]. The server stands; only the key is missing.
final class ServerSavedWithoutKey implements Exception {
  /// Creates the exception for the saved server [name].
  const ServerSavedWithoutKey(this.name, this.diagnostic);

  /// The saved server.
  final String name;

  /// Why the key was refused.
  final Diagnostic diagnostic;

  @override
  String toString() => 'ServerSavedWithoutKey($name, ${diagnostic.code})';
}
