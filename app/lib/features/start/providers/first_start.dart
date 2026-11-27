/// The first-start screen of the start window (HUM-538, ADR-0024 point 3):
/// one screen, everything defaulted, nothing must be set, everything can.
///
/// Everything on it is the daemon's answer (ADR-018, sprint 8 boundary 2):
/// the fields, their options, the defaults, why an option is not available
/// and which value a company policy locks come from `Projects.proposal`.
/// A change of a field asks `proposal` again with the pairs of the chosen
/// option laid over the stored choice, and the screen shows what comes
/// back. "Start" saves exactly the choice of the proposal on screen
/// (`StartProposal.choice`, `save_choice`) and starts the project by its id.
///
/// A folder the register knows gets no screen: it starts with its choice,
/// as "Recent" does. "Change start choice" on a project of the list opens
/// the screen for it on purpose.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import 'projects.dart';

/// What the first-start screen shows.
final class FirstStart {
  /// Creates the state.
  const FirstStart({
    required this.path,
    this.projectId = '',
    this.proposal,
    this.values = const <ChoiceValue>[],
    this.asking = false,
    this.saving = false,
    this.askingCloud = false,
    this.diagnostics = const <Diagnostic>[],
    this.otherStartBusy = false,
  });

  /// The folder as the dialog returned it, or the shown path of the
  /// project whose choice changes.
  final String path;

  /// The project whose choice changes; empty for a new folder.
  final String projectId;

  /// The daemon's latest proposal; null until the first answer.
  final StartProposal? proposal;

  /// The partial choice the person made on the screen, laid over the
  /// stored one in every `proposal`.
  final List<ChoiceValue> values;

  /// True while a `proposal` is under way: a change is not taken until the
  /// daemon answered it, and "Start" waits.
  final bool asking;

  /// True while the choice is saved and the start is handed on.
  final bool saving;

  /// True while the question on cloud inference stands.
  final bool askingCloud;

  /// What the daemon said against loading, saving or confirming.
  final List<Diagnostic> diagnostics;

  /// True when "Start" found another start of the window under way: the
  /// screen stays and says so, nothing was saved.
  final bool otherStartBusy;

  /// Whether "Start" may go now: a proposal is there and current, nothing
  /// is being saved, and no finding of the daemon refuses the defaults.
  bool get canStart {
    final StartProposal? shown = proposal;
    if (shown == null || asking || saving || shown.refused) {
      return false;
    }
    return shown.fields.every(
      (ProposalField field) => field.chosen?.available ?? true,
    );
  }

  /// A copy with the given parts replaced.
  FirstStart copyWith({
    StartProposal? proposal,
    List<ChoiceValue>? values,
    bool? asking,
    bool? saving,
    bool? askingCloud,
    List<Diagnostic>? diagnostics,
    bool? otherStartBusy,
  }) => FirstStart(
    path: path,
    projectId: projectId,
    proposal: proposal ?? this.proposal,
    values: values ?? this.values,
    asking: asking ?? this.asking,
    saving: saving ?? this.saving,
    askingCloud: askingCloud ?? this.askingCloud,
    diagnostics: diagnostics ?? this.diagnostics,
    otherStartBusy: otherStartBusy ?? this.otherStartBusy,
  );
}

/// The first-start screen; null while the start window shows its tabs.
final NotifierProvider<FirstStartNotifier, FirstStart?> firstStartProvider =
    NotifierProvider.autoDispose<FirstStartNotifier, FirstStart?>(
      FirstStartNotifier.new,
    );

/// The notifier behind [firstStartProvider].
class FirstStartNotifier extends Notifier<FirstStart?> {
  /// Counts the `proposal` calls; only the answer to the latest one counts.
  int _asked = 0;

  @override
  FirstStart? build() => null;

  DaemonClient get _client => ref.read(daemonClientProvider);

  /// The hand-off for a folder chosen under "New project": the daemon's
  /// proposal for [path], the screen when the register does not know the
  /// folder, and otherwise the start of the known project without it.
  Future<void> openFolder(String path) async {
    final FirstStart opening = FirstStart(path: path, asking: true);
    state = opening;
    final StartProposal? proposal = await _ask(opening, path: path);
    if (proposal == null || !ref.mounted || state != opening) {
      return;
    }
    if (proposal.known) {
      state = null;
      await ref
          .read(startLaunchProvider.notifier)
          .startKnown(proposal.projectId, proposal.path);
      return;
    }
    state = opening.copyWith(proposal: proposal, asking: false);
  }

  /// The screen for a folder the register does not know, with the
  /// [proposal] the daemon already gave for it: `humanitl app PATH`
  /// preselected it (HUM-539). Unlike [openFolder] nothing starts from
  /// here, not even for a known folder; that is the caller's business, and
  /// it marks a known one in the list instead.
  void show(String path, StartProposal proposal) {
    _asked++;
    state = FirstStart(path: path, proposal: proposal);
  }

  /// "Change start choice" on [entry]: the screen for a project the
  /// register knows, with its stored choice as the defaults.
  Future<void> openProject(ProjectEntry entry) async {
    final FirstStart opening = FirstStart(
      path: entry.path,
      projectId: entry.projectId,
      asking: true,
    );
    state = opening;
    final StartProposal? proposal = await _ask(
      opening,
      projectId: entry.projectId,
    );
    if (proposal == null || !ref.mounted || state != opening) {
      return;
    }
    state = opening.copyWith(proposal: proposal, asking: false);
  }

  /// Chooses [option] for [field] and asks the daemon again.
  ///
  /// The pairs of the option replace every key of the field; a key of the
  /// field the option does not carry goes out empty, which takes it out of
  /// the choice (`Proposal.values`). An option the daemon calls not
  /// available, or a field a company policy locks, is not taken.
  Future<void> choose(ProposalField field, ProposalOption option) async {
    final FirstStart? now = state;
    if (now == null ||
        now.saving ||
        field.locked ||
        !option.available ||
        option.value == field.value) {
      return;
    }
    final Map<String, String> pairs = <String, String>{
      for (final ChoiceValue pair in now.values) pair.key: pair.value,
      for (final String key in field.keys) key: '',
      for (final ChoiceValue pair in option.choice) pair.key: pair.value,
    };
    final List<ChoiceValue> values = List<ChoiceValue>.unmodifiable(
      <ChoiceValue>[
        for (final MapEntry<String, String> pair in pairs.entries)
          ChoiceValue(pair.key, pair.value),
      ],
    );
    final FirstStart asking = now.copyWith(
      values: values,
      asking: true,
      diagnostics: const <Diagnostic>[],
      otherStartBusy: false,
    );
    state = asking;
    final StartProposal? proposal = await _ask(
      asking,
      // Scheitert die Frage, gilt die Wahl davor wieder: Die Felder zeigen
      // dann, was der Daemon zuletzt beantwortet hat.
      failed: now.copyWith(asking: false),
      path: now.projectId.isEmpty ? now.path : null,
      projectId: now.projectId.isEmpty ? null : now.projectId,
      values: values,
    );
    if (proposal == null || !ref.mounted || state != asking) {
      return;
    }
    state = asking.copyWith(proposal: proposal, asking: false);
  }

  /// "Start": the question on cloud inference first when the defaults send
  /// the inference outside this machine and nobody confirmed that, else
  /// save and start.
  Future<void> start() async {
    final FirstStart? now = state;
    if (now == null || !now.canStart) {
      return;
    }
    final CloudInference? cloud = now.proposal?.cloudInference;
    if (cloud != null && cloud.needsConfirmation) {
      state = now.copyWith(askingCloud: true);
      return;
    }
    await _saveAndStart(now);
  }

  /// The person confirmed the notice on cloud inference: the daemon keeps
  /// that for agent and provider, then the choice is saved and the project
  /// starts.
  Future<void> confirmCloud() async {
    final FirstStart? now = state;
    final CloudInference? cloud = now?.proposal?.cloudInference;
    if (now == null || cloud == null) {
      return;
    }
    final FirstStart confirming = now.copyWith(
      askingCloud: false,
      saving: true,
    );
    state = confirming;
    try {
      await _client.acknowledgeCloudInference(cloud.agent, cloud.provider);
    } on DaemonException catch (error) {
      if (ref.mounted && state == confirming) {
        state = now.copyWith(
          askingCloud: false,
          diagnostics: <Diagnostic>[error.diagnostic],
        );
      }
      return;
    }
    if (!ref.mounted || state != confirming) {
      return;
    }
    await _saveAndStart(now.copyWith(askingCloud: false));
  }

  /// Asks the daemon again for the choice on screen: the guided agent setup
  /// (GitHub #111) may have installed an agent or chosen a server since.
  Future<void> refresh() async {
    final FirstStart? now = state;
    if (now == null || now.saving) {
      return;
    }
    final FirstStart asking = now.copyWith(
      asking: true,
      diagnostics: const <Diagnostic>[],
    );
    state = asking;
    final StartProposal? proposal = await _ask(
      asking,
      failed: now.copyWith(asking: false),
      path: now.projectId.isEmpty ? now.path : null,
      projectId: now.projectId.isEmpty ? null : now.projectId,
      values: now.values,
    );
    if (proposal == null || !ref.mounted || state != asking) {
      return;
    }
    state = asking.copyWith(proposal: proposal, asking: false);
  }

  /// Closes the question on cloud inference; nothing starts.
  void dismissCloud() {
    final FirstStart? now = state;
    if (now != null) {
      state = now.copyWith(askingCloud: false);
    }
  }

  /// Leaves the screen; nothing is saved and nothing starts.
  void cancel() {
    _asked++;
    state = null;
  }

  Future<void> _saveAndStart(FirstStart now) async {
    final StartProposal proposal = now.proposal!;
    // Läuft schon ein Start des Fensters, nähme `startKnown` diesen nicht
    // an; der Bildschirm schlösse sonst still, mit gespeicherter Wahl und
    // ohne Start.
    if (ref.read(startLaunchProvider).busyPath != null) {
      state = now.copyWith(otherStartBusy: true);
      return;
    }
    final FirstStart saving = now.copyWith(saving: true, otherStartBusy: false);
    state = saving;
    final bool byFolder = proposal.projectId.isEmpty;
    if (byFolder) {
      DaemonInfo? info = ref.read(daemonInfoProvider).value;
      if (info == null) {
        try {
          info = await ref.read(daemonInfoProvider.future);
        } on Object {
          info = null;
        }
      }
      if (!ref.mounted || state != saving) {
        return;
      }
      if (!(info?.savesFolderChoice ?? false)) {
        state = now.copyWith(
          diagnostics: <Diagnostic>[
            ClientDiagnostics.folderChoiceDaemonTooOld(
              info?.protoVersion ?? '?',
            ),
          ],
        );
        return;
      }
    }
    final ProjectEntry saved;
    try {
      saved = await _client.projectsSaveChoice(
        projectId: byFolder ? null : proposal.projectId,
        path: byFolder ? now.path : null,
        values: proposal.choice,
      );
    } on DaemonException catch (error) {
      if (ref.mounted && state == saving) {
        state = now.copyWith(diagnostics: <Diagnostic>[error.diagnostic]);
      }
      return;
    }
    if (!ref.mounted || state != saving) {
      return;
    }
    state = null;
    final StartLaunchNotifier launch = ref.read(startLaunchProvider.notifier);
    // Die Liste zeigt das Projekt, sobald es eingetragen ist; der Start
    // läuft daneben und meldet sich an seiner Zeile.
    final Future<void> reloaded = ref.read(projectsProvider.notifier).reload();
    await launch.startKnown(saved.projectId, saved.path);
    await reloaded;
  }

  /// One `proposal`; null when a newer question or leaving the screen made
  /// the answer stale, or when the daemon refused (then the screen says so).
  Future<StartProposal?> _ask(
    FirstStart asking, {
    FirstStart? failed,
    String? path,
    String? projectId,
    List<ChoiceValue> values = const <ChoiceValue>[],
  }) async {
    final int ticket = ++_asked;
    try {
      final StartProposal proposal = await _client.projectsProposal(
        path: path,
        projectId: projectId,
        values: values,
      );
      return ticket == _asked ? proposal : null;
    } on DaemonException catch (error) {
      if (ticket == _asked && ref.mounted && state == asking) {
        state = (failed ?? asking.copyWith(asking: false)).copyWith(
          diagnostics: <Diagnostic>[error.diagnostic],
        );
      }
      return null;
    }
  }
}
