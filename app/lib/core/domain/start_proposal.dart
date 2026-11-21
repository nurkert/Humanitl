/// The start proposal of the daemon for a folder (HUM-536, HUM-538): the
/// first-start screen as data.
///
/// Plain value types, mirror of `StartProposal`, `ProposalField`,
/// `ProposalOption` and `AgentSourceOption`. Every field, every option, its
/// default, whether it is available and why not come from the daemon
/// (ADR-018, sprint 8 boundary 2); the app shows no option the proposal does
/// not name and decides nothing itself. A change of a field asks the daemon
/// again with the pairs of the option ([ProposalOption.choice]).
library;

import 'cloud_inference.dart';
import 'diagnostic.dart';
import 'settings_catalog.dart';

/// One key of the project choice with its value as text (`ChoiceValue`).
final class ChoiceValue {
  /// Creates the pair.
  const ChoiceValue(this.key, this.value);

  /// The key, such as `sandbox.mode`.
  final String key;

  /// The value as text.
  final String value;

  @override
  bool operator ==(Object other) =>
      other is ChoiceValue && other.key == key && other.value == value;

  @override
  int get hashCode => Object.hash(key, value);

  @override
  String toString() => '$key=$value';
}

/// One source of an agent in the proposal (`AgentSourceOption`).
final class AgentSourceOption {
  /// Creates the source.
  const AgentSourceOption({
    required this.source,
    required this.available,
    this.reasons = const <Diagnostic>[],
    this.selected = false,
  });

  /// `host` or `depot`.
  final String source;

  /// Whether the program lies ready there.
  final bool available;

  /// Why not, or what is to be said.
  final List<Diagnostic> reasons;

  /// True for the source a start takes.
  final bool selected;
}

/// One option of a field (`ProposalOption`).
final class ProposalOption {
  /// Creates the option.
  const ProposalOption({
    required this.value,
    required this.label,
    required this.available,
    this.reasons = const <Diagnostic>[],
    this.choice = const <ChoiceValue>[],
    this.source = '',
    this.sources = const <AgentSourceOption>[],
    this.cloudInference,
  });

  /// The value, such as `disposable` or `kernel`.
  final String value;

  /// The name for display, English, from the daemon.
  final String label;

  /// Whether a start with this option would go.
  final bool available;

  /// Why not; empty when available.
  final List<Diagnostic> reasons;

  /// The pairs for the keys of the field; a key of the field missing here
  /// drops out of the choice.
  final List<ChoiceValue> choice;

  /// Only `agent`: the source a start takes, `host` or `depot`.
  final String source;

  /// Only `agent`: both sources with availability and reason.
  final List<AgentSourceOption> sources;

  /// Only `agent` and `llm_server`: the notice on cloud inference with this
  /// option.
  final CloudInference? cloudInference;
}

/// One field of the proposal (`ProposalField`).
final class ProposalField {
  /// Creates the field.
  const ProposalField({
    required this.id,
    required this.options,
    this.keys = const <String>[],
    this.value = '',
    this.origin = '',
    this.lock,
  });

  /// `agent`, `folder_mode`, `isolation`, `ask_mode`, `llm_server` or
  /// `profile`.
  final String id;

  /// The keys of the project choice the field writes.
  final List<String> keys;

  /// The value of the default, one of [options]; empty when there is none.
  final String value;

  /// The origin of the default, as in `FieldOrigin.origin`.
  final String origin;

  /// Set when a company policy fixes a key of the field.
  final SettingsLock? lock;

  /// The options in the daemon's order.
  final List<ProposalOption> options;

  /// The option of the default, or null.
  ProposalOption? get chosen {
    for (final ProposalOption option in options) {
      if (option.value == value) {
        return option;
      }
    }
    return null;
  }

  /// True when a company policy fixes the field: it stands read-only, with
  /// the policy as the reason.
  bool get locked => lock != null;
}

/// The start proposal for a folder (`StartProposal`).
final class StartProposal {
  /// Creates the proposal.
  const StartProposal({
    required this.path,
    required this.displayName,
    required this.fields,
    this.projectId = '',
    this.hasChoice = false,
    this.choice = const <ChoiceValue>[],
    this.defaultsLine = '',
    this.diagnostics = const <Diagnostic>[],
    this.cloudInference,
  });

  /// The canonical folder, cleaned for display.
  final String path;

  /// The folder name, cleaned for display.
  final String displayName;

  /// The id in the register; empty for a folder it does not know.
  final String projectId;

  /// Whether the register holds a project choice for the project.
  final bool hasChoice;

  /// The fields in the order of the screen.
  final List<ProposalField> fields;

  /// The choice `save_choice` takes for exactly the defaults shown.
  final List<ChoiceValue> choice;

  /// The line for the way without the screen, starting with `Defaults: `.
  final String defaultsLine;

  /// What stands in the way of a start with the defaults, or is to be said.
  final List<Diagnostic> diagnostics;

  /// The notice on cloud inference for agent and server of the defaults;
  /// null when the inference stays on this machine.
  final CloudInference? cloudInference;

  /// True when the register knows the folder: then the start window starts
  /// it without the screen (ADR-0024 point 3).
  bool get known => projectId.isNotEmpty;

  /// The field [id], or null.
  ProposalField? field(String id) {
    for (final ProposalField field in fields) {
      if (field.id == id) {
        return field;
      }
    }
    return null;
  }

  /// Whether a start with the defaults is refused by a finding of the
  /// daemon (error or blocking).
  bool get refused => diagnostics.any(
    (Diagnostic d) =>
        d.severity == Severity.error || d.severity == Severity.blocking,
  );
}
