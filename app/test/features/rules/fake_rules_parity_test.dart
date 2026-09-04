// Der Fake gegen die Engine: jede Abweichung von `daemon/crates/rules/` und
// `daemon/crates/proxy/src/rules_store.rs` bringt jedem Test eine Regel bei,
// die der Daemon nicht kennt. Diese Datei hält die Stellen fest, an denen das
// schon einmal auseinandergelaufen ist.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import 'fixtures.dart';

/// Eine Regel, die alles trifft, was der Matcher nicht ausschließt.
Rule matcherRule({
  String host = 'registry.npmjs.org',
  List<Method> methods = const <Method>[],
  String path = '',
  Upgrade? upgrade,
  Scheme? scheme,
  int port = 0,
  RuleExpiry expires = const RuleExpiry.never(),
}) => Rule(
  action: RuleAction.allow,
  matcher: RuleMatcher(
    host: host,
    methods: methods,
    path: path,
    upgrade: upgrade,
    scheme: scheme,
    port: port,
  ),
  expires: expires,
);

void main() {
  group('upgrade is symmetric in both directions', () {
    final Flow plain = testFlow(n: 1, host: 'ws.example.org', path: '/agent');
    final Flow upgrade = plain.copyWith(scheme: Scheme.wss);

    test('a rule without upgrade never matches an upgrade', () {
      expect(
        ruleMatchesFlow(
          matcherRule(host: 'ws.example.org'),
          upgrade,
          now: rulesTestNow,
        ),
        isFalse,
      );
    });

    test('a rule with upgrade never matches an ordinary request', () {
      expect(
        ruleMatchesFlow(
          matcherRule(host: 'ws.example.org', upgrade: Upgrade.websocket),
          plain,
          now: rulesTestNow,
        ),
        isFalse,
      );
    });

    test('each of them matches its own kind', () {
      expect(
        ruleMatchesFlow(
          matcherRule(host: 'ws.example.org'),
          plain,
          now: rulesTestNow,
        ),
        isTrue,
      );
      expect(
        ruleMatchesFlow(
          matcherRule(host: 'ws.example.org', upgrade: Upgrade.websocket),
          upgrade,
          now: rulesTestNow,
        ),
        isTrue,
      );
    });
  });

  group('the host glob compares whole labels', () {
    bool hits(String pattern, String host) => ruleMatchesFlow(
      matcherRule(host: pattern),
      testFlow(n: 1, host: host),
      now: rulesTestNow,
    );

    test('a leading ** also matches the apex itself', () {
      expect(hits('**.example.com', 'api.example.com'), isTrue);
      expect(hits('**.example.com', 'a.b.example.com'), isTrue);
      expect(hits('**.example.com', 'example.com'), isTrue);
    });

    test('a ** in the middle still wants at least one label', () {
      expect(hits('api.**.example.com', 'api.eu.example.com'), isTrue);
      expect(hits('api.**.example.com', 'api.example.com'), isFalse);
    });

    test('a single star is exactly one label', () {
      expect(hits('*.example.com', 'api.example.com'), isTrue);
      expect(hits('*.example.com', 'a.b.example.com'), isFalse);
      expect(hits('*.example.com', 'example.com'), isFalse);
    });

    test('the comparison never runs on text', () {
      expect(hits('*.github.com', 'evil-github.com'), isFalse);
      expect(hits('*.github.com', 'github.com.evil.io'), isFalse);
    });
  });

  test('a rule whose time has passed matches nothing', () {
    final Rule over = matcherRule(
      expires: RuleExpiry.at(
        at: rulesTestNow.subtract(const Duration(hours: 1)),
      ),
    );
    final Rule running = matcherRule(
      expires: RuleExpiry.at(at: rulesTestNow.add(const Duration(hours: 1))),
    );
    final Flow flow = testFlow(n: 1);
    expect(ruleMatchesFlow(over, flow, now: rulesTestNow), isFalse);
    expect(ruleMatchesFlow(running, flow, now: rulesTestNow), isTrue);
  });

  test('a method the contract does not know matches nothing', () {
    final Flow odd = testFlow(n: 1)
        .copyWith(method: Method.other, methodRaw: 'PROPFIND');
    expect(ruleMatchesFlow(matcherRule(), odd, now: rulesTestNow), isFalse);
  });

  group('the diagnostics are the ones of the store', () {
    test('a duplicate id is RULES_007, not an invalid request', () async {
      final RulesTestClient client = RulesTestClient();
      client.savedRules.add(testRule(n: 1));
      await expectLater(
        client.addRule(testRule(n: 1)),
        throwsA(
          isA<DaemonException>()
              .having(
                (DaemonException e) => e.code,
                'code',
                DiagnosticCodes.ruleIdDuplicate,
              )
              .having(
                (DaemonException e) => e.diagnostic.why,
                'why',
                contains('is already taken'),
              ),
        ),
      );
    });

    test('an unknown id is IPC_005, word for word', () async {
      final RulesTestClient client = RulesTestClient();
      await expectLater(
        client.updateRule(testRule(n: 9)),
        throwsA(
          isA<DaemonException>()
              .having(
                (DaemonException e) => e.code,
                'code',
                DiagnosticCodes.rulesRequestInvalid,
              )
              .having(
                (DaemonException e) => e.diagnostic.why,
                'why',
                'there is no rule with the id ${testRuleId(9).value}',
              ),
        ),
      );
    });

    test('a bundled rule is refused with the way around it', () async {
      final RulesTestClient client = RulesTestClient();
      try {
        await client.removeRule(FakeDaemonClient.bundledBlockRule);
        fail('the fake let a bundled rule be removed');
      } on DaemonException catch (error) {
        expect(error.code, DiagnosticCodes.ruleBundled);
        // The remedy is a `FixAction`, not a sentence: a diagnostic that
        // carries one and shows no action is a defect (`docs/UX.md` 4.4).
        final FixAction? fix = error.diagnostic.fix;
        expect(fix, isA<FixActionAddRule>());
        final Rule proposed = (fix! as FixActionAddRule).rule;
        expect(proposed.action, RuleAction.ask);
        expect(proposed.matcher, FakeDaemonClient.bundledBlock.matcher);
        expect(error.diagnostic.why, isNot(contains('put a rule')));
      }
    });

    test('a rule that claims to be bundled is refused as such', () async {
      final RulesTestClient client = RulesTestClient();
      await expectLater(
        client.addRule(testRule(n: 2, bundled: true)),
        throwsA(
          isA<DaemonException>().having(
            (DaemonException e) => e.diagnostic.why,
            'why',
            'a rule that arrives over the wire is never bundled',
          ),
        ),
      );
    });

    test('a rule that is already permanent cannot be made permanent', () async {
      final RulesTestClient client = RulesTestClient();
      client.savedRules.add(testRule(n: 1));
      await expectLater(
        client.makeRulePermanent(testRuleId(1)),
        throwsA(
          isA<DaemonException>()
              .having(
                (DaemonException e) => e.code,
                'code',
                DiagnosticCodes.rulesRequestInvalid,
              )
              .having(
                (DaemonException e) => e.diagnostic.why,
                'why',
                contains('already permanent'),
              ),
        ),
      );
    });
  });

  test(
    'a dry run without a limit reads the number the contract names',
    () async {
      final RulesTestClient client = RulesTestClient();
      for (int i = 1; i <= 4; i++) {
        final Flow flow = testFlow(n: i);
        client.state.flows[flow.id] = flow;
      }
      final DryRun answered = await client.dryRunRule(matcherRule(), limit: 0);
      expect(answered.scanned, 4);
      expect(answered.matches, hasLength(4));
    },
  );
}
