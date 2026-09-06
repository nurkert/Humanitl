/// What `ProbeLlm` measured, mirror of `ProbeLlmResponse` (HUM-039, HUM-044).
///
/// The probe is the one call of this application that reaches a machine on the
/// network, and it runs only when a person asks for it. Never on a keystroke:
/// every character typed into the endpoint field would otherwise become a DNS
/// query for a name nobody has decided on yet (HUM-044, feindliche Eingabe).
///
/// # The model names come from outside
///
/// [LlmProbe.models] is what an unauthenticated server on the LAN answered,
/// word for word. The daemon caps the body at one mebibyte and puts every
/// string through `sanitize_note`, and this file caps how many of them and how
/// long a single one may be before anything is drawn. A model name never
/// becomes a command, a link or a path.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'diagnostic.dart';

part 'llm.freezed.dart';

/// Which API answered, mirror of `LlmProduct`.
enum LlmFlavor {
  /// Ollama (`/api/tags`).
  ollama,

  /// An OpenAI-compatible server (`/v1/models`).
  openAiCompatible,

  /// Something answered, and it is neither of the two.
  unknown,
}

/// How many model names a screen shows, and how long one may be.
///
/// Both are caps on a value from the network. The list is shown to say "this
/// server has models", not to be a catalogue; a server that answers with a
/// thousand names must not turn the setup screen into a thousand chips.
abstract final class LlmModelLimits {
  /// How many names are shown at most.
  static const int count = 6;

  /// How many characters of one name are shown at most.
  static const int nameLength = 40;
}

/// What one probe of one endpoint measured.
@freezed
abstract class LlmProbe with _$LlmProbe {
  /// Creates a result.
  const factory LlmProbe({
    required String endpoint,
    @Default(<String>[]) List<String> models,
    @Default(LlmFlavor.unknown) LlmFlavor flavor,
    @Default(0) int latencyMs,
    @Default(false) bool endpointIsPrivate,
    @Default(<Diagnostic>[]) List<Diagnostic> diagnostics,
  }) = _LlmProbe;

  const LlmProbe._();

  /// The first finding, the one a row with room for one shows.
  Diagnostic? get first => diagnostics.isEmpty ? null : diagnostics.first;

  /// True when the probe found nothing to report.
  bool get isClean => diagnostics.isEmpty;

  /// The names to show: at most [LlmModelLimits.count] of them, each cut to
  /// [LlmModelLimits.nameLength] code points.
  ///
  /// Cut here and not with an overflow in the layout on purpose: a name of ten
  /// thousand characters must not reach the layout at all. Cut along runes and
  /// not along code units, so a shortened name is never half a surrogate pair.
  List<String> get shownModels => <String>[
    for (final String name in models.take(LlmModelLimits.count))
      if (name.runes.length <= LlmModelLimits.nameLength)
        name
      else
        String.fromCharCodes(name.runes.take(LlmModelLimits.nameLength)),
  ];

  /// How many names are not shown; zero when all of them are.
  int get hiddenModels => models.length <= LlmModelLimits.count
      ? 0
      : models.length - LlmModelLimits.count;
}

/// One server the search in the local network found, mirror of
/// `DiscoverResult` (HUM-076).
///
/// Everything in here was said by a machine nobody had decided on yet: the
/// host is an address the search itself picked, the models are what the server
/// answered. The same caps as for [LlmProbe] apply, and for the same reason —
/// a row in a list is not a place a stranger gets to fill freely.
@freezed
abstract class LlmServer with _$LlmServer {
  /// Creates a found server.
  const factory LlmServer({
    required String host,
    required int port,
    @Default(LlmFlavor.unknown) LlmFlavor flavor,
    @Default(<String>[]) List<String> models,
    @Default(0) int latencyMs,
    @Default(false) bool authRequired,
  }) = _LlmServer;

  const LlmServer._();

  /// The endpoint a click would take over.
  String get endpoint => 'http://$host:$port';

  /// The names to show, capped exactly like [LlmProbe.shownModels].
  List<String> get shownModels => <String>[
    for (final String name in models.take(LlmModelLimits.count))
      if (name.runes.length <= LlmModelLimits.nameLength)
        name
      else
        String.fromCharCodes(name.runes.take(LlmModelLimits.nameLength)),
  ];

  /// How many names are not shown; zero when all of them are.
  int get hiddenModels => models.length <= LlmModelLimits.count
      ? 0
      : models.length - LlmModelLimits.count;

  /// True when this row can be taken over as `llm.endpoint`.
  ///
  /// A server that answered neither API is listed — somebody is listening
  /// there — but it is not offered as an endpoint: a click would put an
  /// address into the configuration that has never answered as an LLM.
  ///
  /// One that asked for credentials is offered. It answered, and it answered
  /// with the one sentence that ends every probe early: what it is stays
  /// unknown until somebody gives it a key, and that is a decision for a
  /// person and not for this list.
  bool get isUsable => flavor != LlmFlavor.unknown || authRequired;
}
