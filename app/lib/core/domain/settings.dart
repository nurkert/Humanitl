/// The configuration as the settings form reads it, and the state of the
/// access key (HUM-069, HUM-140).
///
/// Plain value types and no freezed: both are read-only views of an answer of
/// the daemon, and neither is ever copied with a change. The form writes
/// through `SetConfig` and `LlmKey` and then asks again.
library;

import 'diagnostic.dart';

/// The keys the settings form writes, in the order the form shows them.
///
/// The same list as `FORM_KEYS` in `daemon/crates/ipc/src/config_rpc.rs`;
/// the daemon refuses every other key with `CONFIG_014`. The access key is not
/// among them: it goes through `LlmKey` and never into `config.toml`.
abstract final class FormKeys {
  /// Seconds a held request waits for a decision.
  static const String holdTimeoutSecs = 'hold.timeout_secs';

  /// Where a held request is asked: `ui`, `terminal` or `none`.
  static const String holdAskMode = 'hold.ask_mode';

  /// The language of the interface: `en` or `de`.
  static const String uiLanguage = 'ui.language';

  /// The appearance: `dark`, `light` or `system`.
  static const String uiTheme = 'ui.theme';

  /// Whether the desktop announces a waiting request.
  static const String uiNotifications = 'ui.notifications';

  /// The project directory mounted as `/work`.
  static const String sandboxWorkDir = 'sandbox.work_dir';

  /// The OpenAI-compatible endpoint of the model.
  static const String llmEndpoint = 'llm.endpoint';

  /// The models the endpoint offers; the first is the agent's default.
  static const String llmModels = 'llm.models';

  /// Whether the daemon trusts the certificate store of the system on its
  /// way to a target, such as a company CA (HUM-257).
  static const String tlsTrustSystemStore = 'tls.trust_system_store';

  /// All of them.
  static const List<String> all = <String>[
    holdTimeoutSecs,
    holdAskMode,
    uiLanguage,
    uiTheme,
    uiNotifications,
    sandboxWorkDir,
    llmEndpoint,
    llmModels,
    tlsTrustSystemStore,
  ];

  /// The reference to the access key, as the daemon names it in the
  /// `ChangeSetting` fix of `LLM_002` (HUM-275).
  ///
  /// Not in [all]: the form never writes it through `SetConfig`. Its field in
  /// the form is the access key, which goes through `LlmKey`.
  static const String llmApiKeyRef = 'llm.api_key_ref';

  /// Every key the settings sheet shows a field for, so a `ChangeSetting` on
  /// one of them can open the sheet at that field (HUM-275). A key outside
  /// this set keeps the badge that names it. [sandboxWorkDir] is in the set,
  /// but its fix stays with the folder chooser of the screen (HUM-262).
  static const Set<String> shownInSheet = <String>{...all, llmApiKeyRef};
}

/// The effective configuration, mirror of `ConfigSnapshot` (HUM-069).
///
/// [values] is the configuration as one JSON object, nested by group. The
/// typed getters below read the keys of the form and fall back to the default
/// of the schema when a value is missing or of the wrong shape: a daemon that
/// answers with less never takes the form down.
final class ConfigValues {
  /// Creates a snapshot.
  const ConfigValues({
    this.values = const <String, Object?>{},
    this.origins = const <String, String>{},
    this.configPath = '',
    this.diagnostics = const <Diagnostic>[],
  });

  /// A snapshot that knows nothing; every getter answers its default.
  static const ConfigValues empty = ConfigValues();

  /// The configuration, nested by group as in `config.toml`.
  final Map<String, Object?> values;

  /// Where each value comes from: `default`, `global`, `env`, …
  final Map<String, String> origins;

  /// The `config.toml` that `SetConfig` writes; empty for a daemon without
  /// one, such as the fake.
  final String configPath;

  /// What the daemon reported while loading.
  final List<Diagnostic> diagnostics;

  /// The value at a dotted key such as `hold.timeout_secs`, or null.
  Object? at(String key) {
    Object? cursor = values;
    for (final String part in key.split('.')) {
      if (cursor is Map<String, Object?>) {
        cursor = cursor[part];
      } else {
        return null;
      }
    }
    return cursor;
  }

  String _text(String key, String fallback) => switch (at(key)) {
    final String text => text,
    _ => fallback,
  };

  /// `hold.timeout_secs`, default 300.
  int get holdTimeoutSecs => switch (at(FormKeys.holdTimeoutSecs)) {
    final int seconds => seconds,
    _ => 300,
  };

  /// `hold.ask_mode`, default `ui`.
  String get holdAskMode => _text(FormKeys.holdAskMode, 'ui');

  /// `ui.language`, default `en`.
  String get uiLanguage => _text(FormKeys.uiLanguage, 'en');

  /// `ui.theme`, default `dark`.
  String get uiTheme => _text(FormKeys.uiTheme, 'dark');

  /// `ui.notifications`, default true.
  bool get uiNotifications => switch (at(FormKeys.uiNotifications)) {
    final bool on => on,
    _ => true,
  };

  /// `tls.trust_system_store`, default true (HUM-257).
  bool get tlsTrustSystemStore => switch (at(FormKeys.tlsTrustSystemStore)) {
    final bool on => on,
    _ => true,
  };

  /// `sandbox.work_dir`, empty when unset.
  String get sandboxWorkDir => _text(FormKeys.sandboxWorkDir, '');

  /// `llm.endpoint`, empty when unset.
  String get llmEndpoint => _text(FormKeys.llmEndpoint, '');

  /// `llm.models`, empty when unset.
  List<String> get llmModels => switch (at(FormKeys.llmModels)) {
    final List<Object?> list => List<String>.unmodifiable(
      list.whereType<String>(),
    ),
    _ => const <String>[],
  };

  /// Whether the value at [key] was set by somebody, in any file, the
  /// environment or the command line, rather than left at its default.
  bool isSet(String key) {
    final String? origin = origins[key];
    return origin != null && origin != 'default';
  }
}

/// Where an access key lies (`LlmKeyStore`).
enum LlmKeyStore {
  /// No key is stored.
  none,

  /// In the system keyring, through the Secret Service.
  secretService,

  /// In a file with mode 0600, because the Secret Service did not answer.
  file,
}

/// The state of the access key, without the key (`LlmKeyStatus`, HUM-140).
///
/// There is no field for the key itself, and that is the point: the daemon
/// never sends it back, so this type cannot carry it to a widget by mistake.
final class LlmKeyStatus {
  /// Creates a state.
  const LlmKeyStatus({
    this.set = false,
    this.store = LlmKeyStore.none,
    this.keyRef = '',
    this.diagnostics = const <Diagnostic>[],
  });

  /// No key stored, nothing to say.
  static const LlmKeyStatus unset = LlmKeyStatus();

  /// True when a key is stored and the store hands it out.
  final bool set;

  /// Where it lies.
  final LlmKeyStore store;

  /// `llm.api_key_ref`, such as `secret-service:humanitl/llm`.
  final String keyRef;

  /// The fallback to the file (`CONFIG_019`), or `LLM_009` when the
  /// configuration names a key the store does not hold.
  final List<Diagnostic> diagnostics;

  @override
  bool operator ==(Object other) =>
      other is LlmKeyStatus &&
      other.set == set &&
      other.store == store &&
      other.keyRef == keyRef &&
      _same(other.diagnostics, diagnostics);

  @override
  int get hashCode =>
      Object.hash(set, store, keyRef, Object.hashAll(diagnostics));
}

bool _same<T>(List<T> a, List<T> b) {
  if (a.length != b.length) {
    return false;
  }
  for (int i = 0; i < a.length; i++) {
    if (a[i] != b[i]) {
      return false;
    }
  }
  return true;
}
