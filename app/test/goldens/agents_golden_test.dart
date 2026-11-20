// Goldens des Abschnitts „Agenten" (HUM-521): die Karte in ihren Zuständen
// (nicht installiert, installiert und angemeldet mit älteren Versionen im
// Depot, gesperrt mit Befund, ohne Rezept) und die Läufe (Anmeldung mit der
// URL eines Anmeldehosts, einer fremden und dem Callback-Feld,
// Konfigurationslauf mit hervorgehobenen Befehls-Schlüsseln), dazu die
// Verschlüsselung der Zugangsdateien an und ohne Schlüsselbund (HUM-523). Je
// in den vier Varianten aus `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/widgets/agents/agent_card.dart';
import 'package:humanitl/features/settings/widgets/agents/credential_encryption_card.dart';

import 'support/golden_variants.dart';

/// Claude Code, installiert, angemeldet, mit zwei älteren Versionen.
AgentDepot installed() => FakeAgents.fakeAgent(
  'claude',
  'Claude Code',
  depotSupported: true,
  source: AgentSource.depot,
  version: '2.1.0',
  versions: const <String>['2.1.0', '2.0.31', '2.0.14'],
  loggedIn: true,
  lastWriteback: '2026-11-19T10:02:41Z session 7f3a9c',
  sizeBytes: 187400000,
  loginDeclared: true,
);

/// Claude Code mit Rezept, nichts installiert.
AgentDepot notInstalled() =>
    FakeAgents.fakeAgent('claude', 'Claude Code', depotSupported: true);

/// Claude Code, durch eine Firmenvorgabe auf das Depot gesperrt, ohne
/// Anmeldung: der Befund des Daemons an der Karte.
AgentDepot locked() => FakeAgents.fakeAgent(
  'claude',
  'Claude Code',
  depotSupported: true,
  source: AgentSource.depot,
  lockedBy: '/etc/humanitl/policy.toml',
  version: '2.1.0',
  sizeBytes: 187400000,
  diagnostics: const <Diagnostic>[
    Diagnostic(
      code: 'AGENT_009',
      severity: Severity.warning,
      why:
          'the depot holds Claude Code and no login for it; the agent asks '
          'for one in the session',
      fix: FixAction.copyCommand(command: 'humanitl agents login claude'),
    ),
  ],
);

/// Codex, dessen Adapter noch kein Rezept mitbringt.
AgentDepot noRecipe() => FakeAgents.fakeAgent('codex', 'Codex');

/// Eine Anmeldung, die läuft: eine URL eines Anmeldehosts, eine fremde, und
/// das Feld für die Callback-URL.
AgentDepot loggingIn() => FakeAgents.fakeAgent(
  'claude',
  'Claude Code',
  depotSupported: true,
  source: AgentSource.depot,
  version: '2.1.0',
  sizeBytes: 187400000,
  loginDeclared: true,
  maintenance: const AgentMaintenance(
    sandboxId: 'maint-claude-login',
    task: AgentRunTask.login,
    state: AgentRunState.running,
    loginUrls: <AgentLoginUrl>[
      AgentLoginUrl(
        url: 'https://claude.ai/oauth/authorize?code=true&client_id=9d1c&state=4f2e',
        openable: true,
      ),
      AgentLoginUrl(url: 'https://example.net/help/login'),
    ],
    callbackPaste: true,
  ),
);

/// Ein Konfigurationslauf, der auf Übernehmen oder Verwerfen wartet.
AgentDepot configWaiting() => FakeAgents.fakeAgent(
  'claude',
  'Claude Code',
  depotSupported: true,
  source: AgentSource.depot,
  version: '2.1.0',
  loggedIn: true,
  sizeBytes: 187400000,
  maintenance: const AgentMaintenance(
    sandboxId: 'maint-claude-config',
    task: AgentRunTask.config,
    state: AgentRunState.confirm,
    configChanges: <AgentConfigChange>[
      AgentConfigChange(
        path: 'settings.json',
        change: 'changed',
        keys: <String>['hooks', 'mcpServers', 'theme', 'model'],
        commandKeys: <String>['hooks', 'mcpServers'],
        keysOmitted: 3,
      ),
      AgentConfigChange(path: 'agents/reviewer.md'),
    ],
  ),
);

/// [entries] als Karten untereinander, auf der Fläche einer Seite.
Widget cards(GoldenVariant variant, List<AgentDepot> entries) => goldenFrame(
  variant: variant,
  background: variant.tokens.colors.bg1,
  padding: const EdgeInsets.all(16),
  overrides: <Override>[
    daemonClientProvider.overrideWithValue(FakeDaemonClient.empty()),
  ],
  child: Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      for (int i = 0; i < entries.length; i++) ...<Widget>[
        if (i > 0) const SizedBox(height: 12),
        AgentCard(
          entry: entries[i],
          config: ConfigValues.empty,
          onOpenSession: () {},
        ),
      ],
    ],
  ),
);

/// Die Verschlüsselung der Zugangsdateien (HUM-523): eingeschaltet über
/// einer Karte, deren Anmeldung verschlüsselt liegt, und ausgeschaltet ohne
/// Schlüsselbund, mit dem Befund an der Karte.
Widget encryption(GoldenVariant variant) => goldenFrame(
  variant: variant,
  background: variant.tokens.colors.bg1,
  padding: const EdgeInsets.all(16),
  overrides: <Override>[
    daemonClientProvider.overrideWithValue(FakeDaemonClient.empty()),
  ],
  child: Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      const CredentialEncryptionCard(
        encryption: CredentialEncryption(
          enabled: true,
          configured: true,
          keyring: true,
        ),
      ),
      const SizedBox(height: 12),
      AgentCard(
        entry: AgentDepot(
          agent: 'claude',
          displayName: 'Claude Code',
          source: AgentSource.depot,
          origin: 'agents.<id>.source',
          depotSupported: true,
          version: '2.1.0',
          signature: 'verified',
          loggedIn: true,
          credentialsEncrypted: true,
          lastWriteback: '2026-11-19T10:02:41Z session 7f3a9c',
          sizeBytes: 187400000,
          actions: const AgentActions(
            update: true,
            login: true,
            config: true,
            configShow: true,
            remove: true,
            sourceHost: true,
            sourceDepot: true,
          ),
        ),
        config: ConfigValues.empty,
        onOpenSession: () {},
      ),
      const SizedBox(height: 12),
      const CredentialEncryptionCard(
        encryption: CredentialEncryption(
          diagnostics: <Diagnostic>[
            Diagnostic(
              code: 'AGENT_020',
              severity: Severity.error,
              why:
                  'the credential files of the depot cannot be encrypted or '
                  'opened: this system has no keyring the daemon can ask (on '
                  'Linux secret-tool of libsecret-tools), and the key lies only '
                  'there, never in a file next to the data; nothing was changed',
              fix: FixAction.copyCommand(
                command: 'sudo apt-get install libsecret-tools',
              ),
            ),
          ],
        ),
      ),
    ],
  ),
);

void main() {
  goldenVariants(
    'settings_agents_cards',
    constraints: const BoxConstraints.tightFor(width: 900, height: 1720),
    builder: (GoldenVariant variant) => cards(variant, <AgentDepot>[
      notInstalled(),
      installed(),
      locked(),
      noRecipe(),
    ]),
  );

  goldenVariants(
    'settings_agents_encryption',
    constraints: const BoxConstraints.tightFor(width: 900, height: 980),
    builder: encryption,
  );

  goldenVariants(
    'settings_agents_runs',
    constraints: const BoxConstraints.tightFor(width: 900, height: 1500),
    builder: (GoldenVariant variant) =>
        cards(variant, <AgentDepot>[loggingIn(), configWaiting()]),
  );
}
