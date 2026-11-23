/// `Projects(proposal)` and `Projects(save_choice)` without a daemon
/// (HUM-538).
///
/// The fake plays a system without `/dev/kvm`: the kernel level is there,
/// not available, with `SANDBOX_045` as its reason, as the daemon answers
/// from the shared result of `doctor` (HUM-536). Asking the upstream proxy
/// is not available without `egress.proxy`, and OpenCode lies neither on
/// the host nor in the depot. A partial choice moves the default of every
/// field whose option pairs it carries; an empty value drops a key, as the
/// daemon does. A lock of the company policy takes the locked keys out of
/// every pair and makes every other option unavailable.
library;

import 'dart:async';

import '../../domain/domain.dart';
import '../daemon_client.dart';
import '../fake_daemon_client.dart';

/// One option of the fake before locks: value, label, pairs, reasons.
typedef _Option = (String, String, List<ChoiceValue>, List<Diagnostic>);

/// `Projects(proposal)` and `Projects(save_choice)` without a daemon.
mixin FakeStartProposal on FakeProjects {
  /// Every `proposal` asked for, in order: folder, project id and the
  /// partial choice.
  final List<(String?, String?, List<ChoiceValue>)> proposalCalls =
      <(String?, String?, List<ChoiceValue>)>[];

  /// Every `save_choice`, in order: project id, folder and the pairs.
  final List<(String?, String?, List<ChoiceValue>)> savedChoices =
      <(String?, String?, List<ChoiceValue>)>[];

  /// The saved choice of a project, by project id.
  final Map<String, List<ChoiceValue>> projectChoices =
      <String, List<ChoiceValue>>{};

  /// Locks of the company policy: field id to the policy that fixes it.
  Map<String, String> proposalLocks = <String, String>{};

  /// The notice on cloud inference of the agent `claude`, when set.
  CloudInference? proposalCloud;

  /// Whether `proposal` sends the field `agent_source` (#15); false plays a
  /// daemon before it, which names the sources only on the agent option.
  bool proposalSourceField = true;

  /// When set, `proposal` answers only once it completes.
  Completer<void>? proposalHold;

  /// What `save_choice` throws once instead of saving, when set.
  Diagnostic? saveChoiceFailure;

  /// What `proposal` throws once instead of answering, when set.
  Diagnostic? proposalFailure;

  int _newProjects = 0;

  /// `Projects(proposal)`.
  Future<StartProposal> projectsProposal({
    String? path,
    String? projectId,
    List<ChoiceValue> values = const <ChoiceValue>[],
  }) async {
    proposalCalls.add((path, projectId, List<ChoiceValue>.of(values)));
    final Completer<void>? hold = proposalHold;
    if (hold != null) {
      await hold.future;
    }
    if (proposalFailure case final Diagnostic failure) {
      proposalFailure = null;
      throw DaemonException(failure);
    }
    final ProjectEntry? entry = _known(path, projectId);
    final Map<String, String> chosen = <String, String>{
      for (final ChoiceValue pair
          in projectChoices[entry?.projectId] ?? const <ChoiceValue>[])
        pair.key: pair.value,
    };
    for (final ChoiceValue pair in values) {
      if (pair.value.isEmpty) {
        chosen.remove(pair.key);
      } else {
        chosen[pair.key] = pair.value;
      }
    }
    final List<ProposalField> fields = <ProposalField>[
      for (final (String id, String fallback, List<_Option> options)
          in _fields())
        _field(id, fallback, options, chosen),
    ];
    final ProposalOption? agent = fields.first.chosen;
    // The source of the chosen agent as its own field, after the agent
    // (#15), as the daemon sends it.
    if (proposalSourceField) {
      fields.insert(1, _sourceField(fields.first.value, agent, chosen));
    }
    return StartProposal(
      path: entry?.path ?? path ?? '',
      displayName:
          entry?.displayName ??
          (path ?? '')
              .split('/')
              .lastWhere(
                (String part) => part.isNotEmpty,
                orElse: () => path ?? '',
              ),
      projectId: entry?.projectId ?? '',
      hasChoice: entry?.hasChoice ?? false,
      fields: List<ProposalField>.unmodifiable(fields),
      choice: List<ChoiceValue>.unmodifiable(<ChoiceValue>[
        for (final ProposalField field in fields) ...?field.chosen?.choice,
      ]),
      defaultsLine:
          'Defaults: ${<String>[for (final ProposalField field in fields)
            if (field.id != 'agent_source') _shown(field)].join(' · ')}',
      cloudInference: agent?.cloudInference,
    );
  }

  /// `Projects(save_choice)`: by id, or by folder, which registers a folder
  /// the register does not know, without a start (contract 1.59).
  Future<ProjectEntry> projectsSaveChoice({
    String? projectId,
    String? path,
    required List<ChoiceValue> values,
  }) async {
    projectsCalls.add('save_choice');
    savedChoices.add((projectId, path, List<ChoiceValue>.of(values)));
    if (saveChoiceFailure case final Diagnostic failure) {
      saveChoiceFailure = null;
      throw DaemonException(failure);
    }
    ProjectEntry? entry = _known(path, projectId);
    if (entry == null) {
      if (projectId != null && projectId.isNotEmpty) {
        throw DaemonException(
          Diagnostic(
            code: 'CONFIG_003',
            severity: Severity.error,
            why: 'no project "$projectId" is known to this daemon',
          ),
        );
      }
      final String folder = path ?? '';
      entry = ProjectEntry(
        projectId: 'fake-project-${++_newProjects}',
        path: folder,
        displayName: folder
            .split('/')
            .lastWhere((String part) => part.isNotEmpty, orElse: () => folder),
        hasChoice: values.isNotEmpty,
      );
      projects = <ProjectEntry>[entry, ...projects];
    } else {
      final ProjectEntry known = entry;
      entry = ProjectEntry(
        projectId: known.projectId,
        path: known.path,
        displayName: known.displayName,
        lastStarted: known.lastStarted,
        lastAgent: known.lastAgent,
        runs: known.runs,
        running: known.running,
        sessionId: known.sessionId,
        changesWaiting: known.changesWaiting,
        missing: known.missing,
        hasChoice: values.isNotEmpty,
      );
      projects = <ProjectEntry>[
        for (final ProjectEntry each in projects)
          each.projectId == known.projectId ? entry : each,
      ];
    }
    projectChoices[entry.projectId] = List<ChoiceValue>.unmodifiable(values);
    return entry;
  }

  ProjectEntry? _known(String? path, String? projectId) {
    for (final ProjectEntry entry in projects) {
      if (entry.replaced) {
        continue;
      }
      if (projectId != null && projectId.isNotEmpty) {
        if (entry.projectId == projectId) {
          return entry;
        }
      } else if (path != null && entry.path == path) {
        return entry;
      }
    }
    return null;
  }

  static List<ChoiceValue> _pairs(Map<String, String> pairs) => <ChoiceValue>[
    for (final MapEntry<String, String> pair in pairs.entries)
      ChoiceValue(pair.key, pair.value),
  ];

  static Diagnostic _blocking(String code, String why) =>
      Diagnostic(code: code, severity: Severity.blocking, why: why);

  /// The fields of the fake system: id, default and options.
  static List<(String, String, List<_Option>)> _fields() =>
      <(String, String, List<_Option>)>[
        (
          'agent',
          'claude',
          <_Option>[
            (
              'claude',
              'Claude Code',
              _pairs(<String, String>{'agent.adapter': 'claude'}),
              const <Diagnostic>[],
            ),
            (
              'codex',
              'Codex',
              _pairs(<String, String>{'agent.adapter': 'codex'}),
              const <Diagnostic>[],
            ),
            (
              'opencode',
              'OpenCode',
              _pairs(<String, String>{'agent.adapter': 'opencode'}),
              <Diagnostic>[
                _blocking(
                  'AGENT_001',
                  'opencode is neither on PATH on the host nor in the agent '
                      'depot',
                ),
              ],
            ),
          ],
        ),
        (
          'folder_mode',
          'disposable',
          <_Option>[
            (
              'disposable',
              'disposable folder',
              _pairs(<String, String>{
                'sandbox.mode': 'disposable',
                'sandbox.work_mode': 'rw',
              }),
              const <Diagnostic>[],
            ),
            (
              'rw',
              'folder read-write',
              _pairs(<String, String>{
                'sandbox.mode': 'direct',
                'sandbox.work_mode': 'rw',
              }),
              const <Diagnostic>[],
            ),
            (
              'ro',
              'folder read-only',
              _pairs(<String, String>{
                'sandbox.mode': 'direct',
                'sandbox.work_mode': 'ro',
              }),
              const <Diagnostic>[],
            ),
          ],
        ),
        (
          'isolation',
          'shared_kernel',
          <_Option>[
            (
              'shared_kernel',
              'shared kernel',
              _pairs(<String, String>{'sandbox.backend': 'auto'}),
              const <Diagnostic>[],
            ),
            (
              'kernel',
              'kernel level',
              _pairs(<String, String>{
                'sandbox.backend': FakeSandboxBackends.kernelBackendName,
              }),
              <Diagnostic>[
                _blocking(
                  'SANDBOX_045',
                  'the kernel level needs /dev/kvm, and this system has none',
                ),
              ],
            ),
          ],
        ),
        (
          'ask_mode',
          'ui',
          <_Option>[
            (
              'ui',
              'ask in the app',
              _pairs(<String, String>{'hold.ask_mode': 'ui'}),
              const <Diagnostic>[],
            ),
            (
              'terminal',
              'ask in the terminal',
              _pairs(<String, String>{'hold.ask_mode': 'terminal'}),
              const <Diagnostic>[],
            ),
            (
              'none',
              'do not ask',
              _pairs(<String, String>{'hold.ask_mode': 'none'}),
              const <Diagnostic>[],
            ),
            (
              'upstream',
              'ask the upstream proxy',
              _pairs(<String, String>{'hold.ask_mode': 'upstream'}),
              <Diagnostic>[
                _blocking(
                  'CONFIG_003',
                  'asking the upstream proxy needs egress.proxy, and none is '
                      'set; requests without a rule would have no one to '
                      'decide them',
                ),
              ],
            ),
          ],
        ),
        (
          'llm_server',
          'local',
          <_Option>[
            (
              'local',
              'local',
              _pairs(<String, String>{'llm.server': 'local'}),
              const <Diagnostic>[],
            ),
          ],
        ),
        (
          'profile',
          'default',
          <_Option>[
            (
              'default',
              'default',
              _pairs(<String, String>{'sandbox.profile': 'default'}),
              const <Diagnostic>[],
            ),
            (
              'strict',
              'strict',
              _pairs(<String, String>{'sandbox.profile': 'strict'}),
              const <Diagnostic>[],
            ),
          ],
        ),
      ];

  ProposalField _field(
    String id,
    String fallback,
    List<_Option> options,
    Map<String, String> chosen,
  ) {
    final String? lockedBy = proposalLocks[id];
    String value = fallback;
    for (final (String option, _, List<ChoiceValue> pairs, _) in options) {
      if (pairs.every((ChoiceValue p) => chosen[p.key] == p.value)) {
        value = option;
      }
    }
    if (lockedBy != null) {
      value = fallback;
    }
    final List<String> keys = options.first.$3
        .map((ChoiceValue p) => p.key)
        .toList(growable: false);
    return ProposalField(
      id: id,
      keys: keys,
      value: value,
      origin: lockedBy != null
          ? 'policy'
          : (value == fallback ? 'default' : 'project-choice'),
      lock: lockedBy == null
          ? null
          : SettingsLock(
              lockedBy: lockedBy,
              reason: 'the company policy fixes ${keys.first} to $fallback',
            ),
      options: <ProposalOption>[
        for (final (
              String option,
              String label,
              List<ChoiceValue> pairs,
              List<Diagnostic> reasons,
            )
            in options)
          _option(id, option, label, pairs, reasons, lockedBy, fallback),
      ],
    );
  }

  ProposalOption _option(
    String field,
    String value,
    String label,
    List<ChoiceValue> pairs,
    List<Diagnostic> reasons,
    String? lockedBy,
    String fallback,
  ) {
    final List<Diagnostic> all = <Diagnostic>[
      ...reasons,
      if (lockedBy != null && value != fallback)
        _blocking(
          'CONFIG_003',
          'the company policy $lockedBy fixes ${pairs.first.key} to '
              '$fallback; this option is not available',
        ),
    ];
    final bool claude = field == 'agent' && value == 'claude';
    return ProposalOption(
      value: value,
      label: label,
      available: all.every(
        (Diagnostic d) =>
            d.severity != Severity.error && d.severity != Severity.blocking,
      ),
      reasons: List<Diagnostic>.unmodifiable(all),
      choice: lockedBy == null
          ? List<ChoiceValue>.unmodifiable(pairs)
          : const <ChoiceValue>[],
      source: field == 'agent' ? 'host' : '',
      sources: field == 'agent'
          ? <AgentSourceOption>[
              AgentSourceOption(
                source: 'host',
                available: reasons.isEmpty,
                reasons: reasons,
                selected: true,
              ),
              AgentSourceOption(
                source: 'depot',
                available: false,
                reasons: <Diagnostic>[
                  _blocking(
                    'AGENT_009',
                    '$value is not installed in the agent depot',
                  ),
                ],
              ),
            ]
          : const <AgentSourceOption>[],
      cloudInference: claude ? proposalCloud : null,
    );
  }

  /// The field `agent_source` of [agentId]: `host` with the agent's own
  /// reasons, and `depot`, which the fake never holds. A lock under the id
  /// `agent_source` in [proposalLocks] fixes it to the depot, read-only,
  /// with the policy named and `host` not available.
  ProposalField _sourceField(
    String agentId,
    ProposalOption? agent,
    Map<String, String> chosen,
  ) {
    final String key = 'agents.$agentId.source';
    final String? lockedBy = proposalLocks['agent_source'];
    final String value = lockedBy != null ? 'depot' : (chosen[key] ?? 'host');
    final List<Diagnostic> hostReasons = <Diagnostic>[
      ...?agent?.reasons,
      if (lockedBy != null)
        _blocking(
          'AGENT_010',
          'the company policy $lockedBy fixes the source of the agent '
              '$agentId to the depot; the source host is refused',
        ),
    ];
    final List<Diagnostic> depotReasons = <Diagnostic>[
      _blocking('AGENT_013', '$agentId is not installed in the agent depot'),
    ];
    ProposalOption option(
      String source,
      String label,
      List<Diagnostic> reasons,
    ) => ProposalOption(
      value: source,
      label: label,
      available: reasons.every(
        (Diagnostic d) =>
            d.severity != Severity.error && d.severity != Severity.blocking,
      ),
      reasons: List<Diagnostic>.unmodifiable(reasons),
      choice: lockedBy == null
          ? <ChoiceValue>[ChoiceValue(key, source)]
          : const <ChoiceValue>[],
    );
    return ProposalField(
      id: 'agent_source',
      keys: <String>[key],
      value: value,
      origin: lockedBy != null
          ? 'policy'
          : (chosen.containsKey(key) ? 'project-choice' : 'default'),
      lock: lockedBy == null
          ? null
          : SettingsLock(
              lockedBy: lockedBy,
              reason: 'the company policy fixes $key to depot',
            ),
      options: <ProposalOption>[
        option('host', 'host', hostReasons),
        option('depot', 'agent depot', depotReasons),
      ],
    );
  }

  static String _shown(ProposalField field) {
    final ProposalOption? option = field.chosen;
    if (option == null) {
      return '${field.id} unset';
    }
    return switch (field.id) {
      'agent' => '${option.label} from the ${option.source}',
      'llm_server' => 'model server ${option.label}',
      'profile' => 'profile ${option.label}',
      _ => option.label,
    };
  }
}
