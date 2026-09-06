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
