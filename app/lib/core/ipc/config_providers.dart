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
  Future<void> write(String key, String value) async {
    await ref.read(daemonClientProvider).setConfig(key, value);
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
  Future<LlmKeyStatus> store(String key) async {
    final LlmKeyStatus status = await ref
        .read(daemonClientProvider)
        .setLlmKey(key);
    state = AsyncValue<LlmKeyStatus>.data(status);
    return status;
  }

  /// Removes the key and the reference.
  Future<LlmKeyStatus> clear() async {
    final LlmKeyStatus status = await ref
        .read(daemonClientProvider)
        .clearLlmKey();
    state = AsyncValue<LlmKeyStatus>.data(status);
    return status;
  }
}
