// Der Abschnitt „Agenten" der Einstellungen (HUM-521): eine Karte je Agent
// mit Quelle, Version, Anmeldung, Größe und den Knöpfen. Die App entscheidet
// nichts selbst: Ob ein Knopf offen ist, sagt `AgentDepot.actions`, die
// Sperre `locked_by`, welche URL sich öffnen lässt `openable`, ob das
// Callback-Feld erscheint `callback_paste`. Jeder Test prüft, was die App an
// den Daemon schickt und dass sie zeigt, was er antwortet (ADR-018).

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/settings/providers/agents_provider.dart';
import 'package:humanitl/features/settings/settings_sheet.dart';

import '../../goldens/support/golden_variants.dart';
import '../../harness/settings_harness.dart';

/// Linux und macOS: Der Abschnitt hat keinen eigenen Weg je Plattform, und
/// das soll so bleiben (`flutter test` läuft sonst als Android).
final TargetPlatformVariant _desktops = TargetPlatformVariant(<TargetPlatform>{
  TargetPlatform.linux,
  TargetPlatform.macOS,
});

const String _policy = '/etc/humanitl/policy.toml';

/// Was der Test an Nebenwirkungen der Karte sieht.
class _Host {
  final List<String> opened = <String>[];
  int sessionsOpened = 0;
}

/// Das Overlay über [client], auf der Seite Agenten.
Future<_Host> _open(WidgetTester tester, FakeDaemonClient client) async {
  final _Host host = _Host();
  tester.view
    ..physicalSize = const Size(1280, 1000)
    ..devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    goldenFrame(
      variant: GoldenVariant.darkEn,
      background: GoldenVariant.darkEn.tokens.colors.bg0,
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        loginUrlOpenerProvider.overrideWithValue((String url) async {
          host.opened.add(url);
          return true;
        }),
      ],
      child: SettingsSheet(
        onClose: () {},
        onOpenSession: () => host.sessionsOpened++,
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.pumpAndSettle();
  await showSettingsCategory(tester, SettingsCategory.agents);
  return host;
}

Future<void> _tap(WidgetTester tester, String key) async {
  final Finder target = find.byKey(ValueKey<String>(key));
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

String _text(WidgetTester tester, String key) =>
    tester.widget<Text>(find.byKey(ValueKey<String>(key))).data ?? '';

/// Ob der Knopf [key] gedrückt werden kann.
bool _enabled(WidgetTester tester, String key) {
  final Finder button = find.byKey(ValueKey<String>(key));
  expect(button, findsOneWidget, reason: key);
  return tester.widget<HButton>(button).onPressed != null;
}

/// Ein Fake mit einem Claude Code in [entry] und Codex ohne Rezept.
FakeDaemonClient _with(AgentDepot entry) {
  final FakeDaemonClient client = FakeDaemonClient.empty();
  client
    ..agentsWithLogin = <String>{'claude'}
    ..agentsStatus = AgentsStatus(
      agents: <AgentDepot>[entry, FakeAgents.fakeAgent('codex', 'Codex')],
    );
  return client;
}

AgentDepot _installed({
  bool loggedIn = false,
  String lockedBy = '',
  List<String> versions = const <String>['2.1.0', '2.0.31', '2.0.14'],
  List<Diagnostic> diagnostics = const <Diagnostic>[],
  AgentMaintenance? maintenance,
}) => FakeAgents.fakeAgent(
  'claude',
  'Claude Code',
  depotSupported: true,
  source: AgentSource.depot,
  lockedBy: lockedBy,
  version: '2.1.0',
  versions: versions,
  loggedIn: loggedIn,
  lastWriteback: loggedIn ? '2026-11-19T10:02:41Z session 7f3a9c' : '',
  sizeBytes: 187400000,
  loginDeclared: true,
  diagnostics: diagnostics,
  maintenance: maintenance,
);

void main() {
  group('the card follows the state of the daemon', () {
    testWidgets('not installed: Install is the one open step', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = _with(
        FakeAgents.fakeAgent('claude', 'Claude Code', depotSupported: true),
      );
      await _open(tester, client);
      expect(_text(tester, 'agent-version-claude'), 'Not installed');
      expect(_text(tester, 'agent-login-claude'), 'Not logged in');
      expect(_enabled(tester, 'agent-install-claude'), isTrue);
      for (final String closed in <String>[
        'agent-update-claude',
        'agent-log-in-claude',
        'agent-configure-claude',
        'agent-show-config-claude',
        'agent-remove-claude',
      ]) {
        expect(_enabled(tester, closed), isFalse, reason: closed);
      }
      // Codex bringt kein Rezept: keine Knöpfe, nur der Satz dazu.
      expect(
        find.byKey(const ValueKey<String>('agent-install-codex')),
        findsNothing,
      );
      expect(
        _text(tester, 'agent-meaning-codex'),
        'No depot recipe for this agent yet, so its program comes from this machine.',
      );
    }, variant: _desktops);

    testWidgets('installed: version, signature and size, Update and Remove '
        'open, Install closed', (WidgetTester tester) async {
      await _open(tester, _with(_installed()));
      expect(_text(tester, 'agent-version-claude'), '2.1.0');
      expect(find.text('Signature verified'), findsOneWidget);
      expect(_text(tester, 'agent-size-claude'), '187 MB');
      expect(_enabled(tester, 'agent-install-claude'), isFalse);
      expect(_enabled(tester, 'agent-update-claude'), isTrue);
      expect(_enabled(tester, 'agent-configure-claude'), isTrue);
      expect(_enabled(tester, 'agent-remove-claude'), isTrue);
    }, variant: _desktops);

    testWidgets('logged in: the state and the last write-back', (
      WidgetTester tester,
    ) async {
      await _open(tester, _with(_installed(loggedIn: true)));
      expect(_text(tester, 'agent-login-claude'), 'Logged in');
      // Lesbar in Ortszeit, mit der Sitzung; nie der Rohtext RFC 3339.
      final String last = _text(tester, 'agent-writeback-last-claude');
      expect(last, startsWith('Last write-back '));
      expect(last, contains('2026'));
      expect(last, endsWith('session 7f3a9c'));
      expect(last, isNot(contains('T10:02:41Z')));
    }, variant: _desktops);

    testWidgets('locked: the switch has no effect and the policy is named', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = _with(_installed(lockedBy: _policy));
      await _open(tester, client);
      expect(
        _text(tester, 'agent-lock-claude'),
        'Set by your organisation: $_policy',
      );
      await tapSegment(
        tester,
        const ValueKey<String>('agent-source-claude'),
        'Host',
      );
      expect(
        client.agentsCalls.where(
          (AgentsOp op) => op.kind == AgentsOpKind.setSource,
        ),
        isEmpty,
        reason: 'a locked source sends nothing',
      );
      expect(client.agentsStatus.of('claude')!.source, AgentSource.depot);
      expect(
        find.byKey(const ValueKey<String>('agent-refusal-claude')),
        findsNothing,
      );
    }, variant: _desktops);

    testWidgets('error: the findings stand at the card with why and fix', (
      WidgetTester tester,
    ) async {
      await _open(
        tester,
        _with(
          _installed(
            diagnostics: const <Diagnostic>[
              Diagnostic(
                code: 'AGENT_013',
                severity: Severity.error,
                why: 'the depot of Claude Code is unusable: current is no link',
                fix: FixAction.copyCommand(
                  command: 'humanitl agents remove claude',
                ),
              ),
            ],
          ),
        ),
      );
      expect(
        find.text('the depot of Claude Code is unusable: current is no link'),
        findsOneWidget,
      );
      expect(find.text('humanitl agents remove claude'), findsOneWidget);
    }, variant: _desktops);

    testWidgets('the section says when the daemon does not answer', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = _with(_installed())
        ..agentsRefusals[AgentsOpKind.status] = const Diagnostic(
          code: 'IPC_006',
          severity: Severity.error,
          why: 'this daemon runs no sandboxes',
        );
      await _open(tester, client);
      expect(find.byKey(const Key('settings-agents-failed')), findsOneWidget);
      expect(find.text('this daemon runs no sandboxes'), findsOneWidget);
      await _tap(tester, 'settings-agents-retry');
      expect(find.byKey(const Key('agent-card-claude')), findsOneWidget);
    }, variant: _desktops);
  });

  testWidgets('switching the source calls the RPC and shows the new state', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _with(
      FakeAgents.fakeAgent('claude', 'Claude Code', depotSupported: true),
    );
    await _open(tester, client);
    await tapSegment(
      tester,
      const ValueKey<String>('agent-source-claude'),
      'Depot',
    );
    expect(
      client.agentsCalls.last,
      const AgentsOp.setSource('claude', AgentSource.depot),
    );
    expect(
      _text(tester, 'agent-meaning-claude'),
      'Sessions start the copy in the depot, never one on this machine.',
    );
  }, variant: _desktops);

  testWidgets('the write-back switch writes the key and shows the new state', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _with(_installed(loggedIn: true));
    await _open(tester, client);
    await tapSegment(
      tester,
      const ValueKey<String>('agent-writeback-claude'),
      'Off',
    );
    expect(client.configWrites.last, (
      'agents.claude.credential_writeback',
      'false',
    ));
    expect(client.configValues.at('agents.claude.credential_writeback'), false);
    // Der neue Stand kommt aus `GetConfig`: „Aus" ist gewählt.
    await tapSegment(
      tester,
      const ValueKey<String>('agent-writeback-claude'),
      'Off',
    );
    expect(
      client.configWrites.where(
        ((String, String) write) =>
            write.$1 == 'agents.claude.credential_writeback',
      ),
      hasLength(1),
      reason: 'Off is shown as chosen, so a second tap sends nothing',
    );
    await tapSegment(
      tester,
      const ValueKey<String>('agent-writeback-claude'),
      'On',
    );
    expect(client.configWrites.last, (
      'agents.claude.credential_writeback',
      'true',
    ));
  }, variant: _desktops);

  testWidgets('Install sends the version typed, and the run opens its '
      'terminal like any session', (WidgetTester tester) async {
    final FakeDaemonClient client = _with(
      FakeAgents.fakeAgent('claude', 'Claude Code', depotSupported: true),
    );
    final _Host host = await _open(tester, client);
    await _tap(tester, 'agent-install-claude');
    await tapSegment(
      tester,
      const ValueKey<String>('agent-version-mode-claude'),
      'Specific version',
    );
    await tester.enterText(
      find.byKey(const ValueKey<String>('agent-version-input-claude')),
      '2.0.31',
    );
    await tester.pumpAndSettle();
    await _tap(tester, 'agent-version-start-claude');
    expect(
      client.agentsCalls.last,
      const AgentsOp.install('claude', version: '2.0.31'),
    );
    expect(host.sessionsOpened, 1);
    expect(_text(tester, 'agent-run-state-claude'), 'Installing in a sandbox');
    // Während des Laufs ist kein weiterer Lauf offen; das sagt der Daemon.
    expect(_enabled(tester, 'agent-install-claude'), isFalse);
    client.finishAgentRun('claude', version: '2.0.31');
    await tester.pump(agentsPollInterval);
    await tester.pumpAndSettle();
    expect(_text(tester, 'agent-version-claude'), '2.0.31');
  }, variant: _desktops);

  testWidgets('Latest sends no version', (WidgetTester tester) async {
    final FakeDaemonClient client = _with(_installed());
    await _open(tester, client);
    await _tap(tester, 'agent-update-claude');
    await _tap(tester, 'agent-version-start-claude');
    expect(client.agentsCalls.last, const AgentsOp.update('claude'));
  }, variant: _desktops);

  testWidgets('back to an older version from the list of the depot', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _with(_installed());
    final _Host host = await _open(tester, client);
    await _tap(tester, 'agent-update-claude');
    // Die installierte Version steht nicht in der Liste für ein Zurück.
    expect(
      find.byKey(const ValueKey<String>('agent-version-held-claude-2.1.0')),
      findsNothing,
    );
    await _tap(tester, 'agent-version-held-claude-2.0.14');
    expect(find.text('Switch to 2.0.14'), findsOneWidget);
    await _tap(tester, 'agent-version-start-claude');
    expect(
      client.agentsCalls.last,
      const AgentsOp.update('claude', version: '2.0.14'),
    );
    // Ein Umschalten ohne Lauf hat kein Terminal.
    expect(host.sessionsOpened, 0);
    expect(_text(tester, 'agent-version-claude'), '2.0.14');
  }, variant: _desktops);

  testWidgets('the configuration view shows the files of the depot', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _with(_installed())
      ..agentsConfigFiles = const <AgentConfigChange>[
        AgentConfigChange(
          path: 'settings.json',
          keys: <String>['hooks', 'theme'],
          commandKeys: <String>['hooks'],
        ),
        AgentConfigChange(path: 'CLAUDE.md'),
      ];
    await _open(tester, client);
    await _tap(tester, 'agent-show-config-claude');
    expect(client.agentsCalls.last, const AgentsOp.configShow('claude'));
    expect(find.text('settings.json'), findsOneWidget);
    expect(find.text('CLAUDE.md'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('agent-command-key-claude-hooks')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('agent-key-claude-theme')),
      findsOneWidget,
    );
    // Nur lesend: kein Übernehmen, kein Verwerfen.
    expect(
      find.byKey(const ValueKey<String>('agent-config-accept-claude')),
      findsNothing,
    );
    await _tap(tester, 'agent-show-config-claude');
    expect(find.text('settings.json'), findsNothing);
  }, variant: _desktops);

  testWidgets('after a configuration run: the changes with highlighted '
      'command keys, then Accept', (WidgetTester tester) async {
    final FakeDaemonClient client = _with(_installed(loggedIn: true))
      ..agentsConfigChanges = const <AgentConfigChange>[
        AgentConfigChange(
          path: 'settings.json',
          change: 'changed',
          keys: <String>['hooks', 'theme'],
          commandKeys: <String>['hooks'],
          keysOmitted: 2,
        ),
      ];
    final _Host host = await _open(tester, client);
    await _tap(tester, 'agent-configure-claude');
    expect(client.agentsCalls.last, const AgentsOp.config('claude'));
    expect(host.sessionsOpened, 1);
    client.finishAgentRun('claude');
    await tester.pump(agentsPollInterval);
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('agent-command-key-claude-hooks')),
      findsOneWidget,
    );
    expect(find.text('2 more keys'), findsOneWidget);
    await _tap(tester, 'agent-config-accept-claude');
    expect(client.agentsCalls.last, const AgentsOp.configAccept('claude'));
    expect(find.text('Configuration taken over.'), findsOneWidget);
  }, variant: _desktops);

  testWidgets('Discard drops the waiting configuration', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _with(
      _installed(
        maintenance: const AgentMaintenance(
          sandboxId: 'maint-claude-config',
          task: AgentRunTask.config,
          state: AgentRunState.confirm,
          configChanges: <AgentConfigChange>[
            AgentConfigChange(path: 'settings.json', change: 'changed'),
          ],
        ),
      ),
    );
    await _open(tester, client);
    await _tap(tester, 'agent-config-discard-claude');
    expect(client.agentsCalls.last, const AgentsOp.configDiscard('claude'));
    expect(
      find.text('Changes discarded; the configuration stays as it was.'),
      findsOneWidget,
    );
    expect(find.text('Configuration taken over.'), findsNothing);
  }, variant: _desktops);

  testWidgets('a login offers Open in browser only for a login host; a URL '
      'of another host opens nothing', (WidgetTester tester) async {
    final FakeDaemonClient client = _with(_installed())
      ..agentsLoginUrls = const <AgentLoginUrl>[
        AgentLoginUrl(
          url: 'https://claude.ai/oauth/authorize?state=1',
          openable: true,
        ),
        AgentLoginUrl(url: 'https://evil.example/login'),
      ];
    final _Host host = await _open(tester, client);
    await _tap(tester, 'agent-log-in-claude');
    expect(client.agentsCalls.last, const AgentsOp.login('claude'));
    // Eine Anmeldung bleibt hier: URL und Callback-Feld stehen an der Karte.
    expect(host.sessionsOpened, 0);
    expect(
      find.byKey(const ValueKey<String>('agent-url-open-claude-1')),
      findsNothing,
    );
    expect(
      find.byKey(const ValueKey<String>('agent-url-shown-claude-1')),
      findsOneWidget,
    );
    await _tap(tester, 'agent-url-text-claude-1');
    expect(
      host.opened,
      isEmpty,
      reason: 'a foreign URL is shown, never opened',
    );
    await _tap(tester, 'agent-url-open-claude-0');
    expect(host.opened, <String>['https://claude.ai/oauth/authorize?state=1']);
    // Ohne `callback_paste` kein Feld.
    expect(
      find.byKey(const ValueKey<String>('agent-callback-input-claude')),
      findsNothing,
    );
  }, variant: _desktops);

  testWidgets('the callback field appears where the daemon asks for it and '
      'hands the URL to the login', (WidgetTester tester) async {
    final FakeDaemonClient client = _with(_installed())
      ..agentsCallbackPaste = true;
    await _open(tester, client);
    await _tap(tester, 'agent-log-in-claude');
    final Finder field = find.byKey(
      const ValueKey<String>('agent-callback-input-claude'),
    );
    await tester.ensureVisible(field);
    await tester.enterText(field, 'http://localhost:54545/callback?code=x');
    await _tap(tester, 'agent-callback-send-claude');
    expect(client.agentsCallbacks, <String>[
      'http://localhost:54545/callback?code=x',
    ]);
    expect(
      find.byKey(const ValueKey<String>('agent-callback-sent-claude')),
      findsOneWidget,
    );
  }, variant: _desktops);

  testWidgets('Remove asks first; Cancel keeps the depot', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _with(_installed());
    await _open(tester, client);
    await _tap(tester, 'agent-remove-claude');
    expect(
      client.agentsCalls.where((AgentsOp op) => op.kind == AgentsOpKind.remove),
      isEmpty,
    );
    await _tap(tester, 'agent-remove-cancel-claude');
    expect(client.agentsStatus.of('claude')!.installed, isTrue);
    await _tap(tester, 'agent-remove-claude');
    await _tap(tester, 'agent-remove-yes-claude');
    expect(client.agentsCalls.last, const AgentsOp.remove('claude'));
    expect(_text(tester, 'agent-version-claude'), 'Not installed');
  }, variant: _desktops);

  testWidgets('a refusal of the daemon stands at the card', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _with(_installed())
      ..agentsRefusals[AgentsOpKind.config] = const Diagnostic(
        code: 'CLI_005',
        severity: Severity.error,
        why: 'a session runs in this daemon; the configuration run waits',
      );
    await _open(tester, client);
    await _tap(tester, 'agent-configure-claude');
    expect(
      _text(tester, 'agent-refusal-claude'),
      'a session runs in this daemon; the configuration run waits',
    );
  }, variant: _desktops);

  group('review fixes', () {
    testWidgets('an agent without a recipe shows what the daemon accepts: '
        'its depot can go, the write-back key is settable, Depot is open', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = FakeDaemonClient.empty();
      client.agentsStatus = AgentsStatus(
        agents: <AgentDepot>[
          FakeAgents.fakeAgent(
            'opencode',
            'OpenCode',
            version: '1.0.0',
            sizeBytes: 4000000,
          ),
        ],
      );
      await _open(tester, client);
      expect(_enabled(tester, 'agent-remove-opencode'), isTrue);
      expect(_enabled(tester, 'agent-install-opencode'), isFalse);
      await tapSegment(
        tester,
        const ValueKey<String>('agent-writeback-opencode'),
        'Off',
      );
      expect(client.configWrites.last, (
        'agents.opencode.credential_writeback',
        'false',
      ));
      await tapSegment(
        tester,
        const ValueKey<String>('agent-source-opencode'),
        'Depot',
      );
      expect(
        client.agentsCalls.last,
        const AgentsOp.setSource('opencode', AgentSource.depot),
      );
      // Der Satz folgt der Quelle: mit Depot bricht eine Sitzung ab.
      expect(
        _text(tester, 'agent-meaning-opencode'),
        'No depot recipe for this agent yet: sessions with the source Depot '
        'stop with AGENT_015 until there is one.',
      );
    }, variant: _desktops);

    testWidgets('while the daemon could not start a session, no run is open', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = _with(_installed())
        ..agentsStartable = false;
      await _open(tester, client);
      expect(_enabled(tester, 'agent-update-claude'), isFalse);
      expect(_enabled(tester, 'agent-configure-claude'), isFalse);
      expect(_enabled(tester, 'agent-remove-claude'), isTrue);
      // Auch die Antwort auf `config_show` öffnet keinen Lauf.
      await _tap(tester, 'agent-show-config-claude');
      expect(client.agentsCalls.last, const AgentsOp.configShow('claude'));
      expect(_enabled(tester, 'agent-update-claude'), isFalse);
      expect(_enabled(tester, 'agent-configure-claude'), isFalse);
    }, variant: _desktops);

    testWidgets('a configuration waiting on disk after a restart can be '
        'accepted', (WidgetTester tester) async {
      final FakeDaemonClient client = _with(
        _installed(
          maintenance: const AgentMaintenance(
            task: AgentRunTask.config,
            state: AgentRunState.confirm,
            configChanges: <AgentConfigChange>[
              AgentConfigChange(path: 'settings.json', change: 'changed'),
            ],
          ),
        ),
      );
      await _open(tester, client);
      await _tap(tester, 'agent-config-accept-claude');
      expect(client.agentsCalls.last, const AgentsOp.configAccept('claude'));
      expect(find.text('Configuration taken over.'), findsOneWidget);
    }, variant: _desktops);

    testWidgets('a poll that keeps failing ends in the finding, not in a '
        'run shown forever', (WidgetTester tester) async {
      final FakeDaemonClient client = _with(_installed());
      await _open(tester, client);
      await _tap(tester, 'agent-configure-claude');
      client.agentsStatusFailures = agentsPollFailureCap;
      for (int i = 0; i < agentsPollFailureCap; i++) {
        await tester.pump(agentsPollInterval);
      }
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('settings-agents-failed')), findsOneWidget);
      expect(find.text('the daemon does not answer'), findsOneWidget);
    }, variant: _desktops);

    testWidgets('a single failed poll is tried again', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = _with(_installed());
      await _open(tester, client);
      await _tap(tester, 'agent-configure-claude');
      client.agentsStatusFailures = 1;
      await tester.pump(agentsPollInterval);
      client.finishAgentRun('claude');
      await tester.pump(agentsPollInterval);
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey<String>('agent-config-accept-claude')),
        findsOneWidget,
      );
    }, variant: _desktops);

    testWidgets('Send of the callback waits for text; the switch of the '
        'write-back rests during a run; Hide always works', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = _with(_installed())
        ..agentsCallbackPaste = true
        ..agentsConfigFiles = const <AgentConfigChange>[
          AgentConfigChange(path: 'settings.json'),
        ];
      await _open(tester, client);
      await _tap(tester, 'agent-show-config-claude');
      expect(find.text('settings.json'), findsOneWidget);
      await _tap(tester, 'agent-log-in-claude');
      expect(_enabled(tester, 'agent-callback-send-claude'), isFalse);
      await tester.enterText(
        find.byKey(const ValueKey<String>('agent-callback-input-claude')),
        'http://localhost:1/cb',
      );
      await tester.pump();
      expect(_enabled(tester, 'agent-callback-send-claude'), isTrue);
      final int writes = client.configWrites.length;
      await tapSegment(
        tester,
        const ValueKey<String>('agent-writeback-claude'),
        'Off',
      );
      expect(client.configWrites, hasLength(writes));
      // Sagt der Daemon, das Zeigen sei zu, bleibt Ausblenden offen.
      client.agentsStatus = AgentsStatus(
        agents: <AgentDepot>[
          AgentDepot(
            agent: 'claude',
            displayName: 'Claude Code',
            depotSupported: true,
            version: '2.1.0',
            maintenance: client.agentsStatus.of('claude')!.maintenance,
          ),
        ],
      );
      await tester.pump(agentsPollInterval);
      await tester.pumpAndSettle();
      expect(_enabled(tester, 'agent-show-config-claude'), isTrue);
      await _tap(tester, 'agent-show-config-claude');
      expect(find.text('settings.json'), findsNothing);
    }, variant: _desktops);
  });
}
