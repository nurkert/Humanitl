// Der Regelsatz und die Regel dahinter (HUM-028): dieselben fünf Beispiele
// wie in der Spezifikation, in beiden Sprachen, und die Zuordnung von Dauer
// und Ziel auf `RuleMatch` und `Expiry`.
//
// Der Satz selbst kommt seit HUM-033 aus `core/text/rule_sentence.dart`; hier
// steht die Übersetzung Entwurf zu Regel und der Wortlaut, den beide Seiten
// teilen.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

/// Der Apex, den sonst der Daemon liefert (`DomainInfo.apex`).
String apexOf(String host) => 'github.com';

Flow githubFlow({
  Method method = Method.get,
  String host = 'api.github.com',
  String path = '/graphql?first=20',
}) => heldFlow(
  n: 1,
  deadline: testStart.add(const Duration(minutes: 5)),
  method: method,
  host: host,
  path: path,
);

void main() {
  late AppLocalizations en;
  late AppLocalizations de;

  setUpAll(() async {
    en = await AppLocalizations.delegate.load(const Locale('en'));
    de = await AppLocalizations.delegate.load(const Locale('de'));
  });

  group('ruleSentence', () {
    test('host, session', () {
      final RuleDraft draft = RuleDraft(
        duration: RememberDuration.session,
        target: RememberTarget.host,
        flow: githubFlow(),
      );
      expect(
        ruleSentence(draft, en, apexOf: apexOf),
        'allow · ∗ · api.github.com · this session',
      );
      expect(
        ruleSentence(draft, de, apexOf: apexOf),
        'Erlauben · ∗ · api.github.com · diese Session',
      );
    });

    test('host and method, forever', () {
      final RuleDraft draft = RuleDraft(
        duration: RememberDuration.forever,
        target: RememberTarget.hostMethod,
        flow: githubFlow(),
      );
      expect(
        ruleSentence(draft, en, apexOf: apexOf),
        'allow · GET · api.github.com · always',
      );
      expect(
        ruleSentence(draft, de, apexOf: apexOf),
        'Erlauben · GET · api.github.com · immer',
      );
    });

    test('apex, one hour', () {
      final RuleDraft draft = RuleDraft(
        duration: RememberDuration.oneHour,
        target: RememberTarget.apex,
        flow: githubFlow(),
      );
      // Die Frist steht so da, wie sie danach im Regel-Bildschirm steht: der
      // Satz kommt aus dem einen Generator in `core/text/rule_sentence.dart`,
      // und der liest die Restlaufzeit der gebauten Regel, statt die
      // Beschriftung des Segments zu wiederholen (CONVENTIONS 4.13).
      expect(
        ruleSentence(draft, en, apexOf: apexOf),
        'allow · ∗ · **.github.com · expires in 60 min',
      );
      expect(
        ruleSentence(draft, de, apexOf: apexOf),
        'Erlauben · ∗ · **.github.com · endet in 60 min',
      );
    });

    test('url, session', () {
      final RuleDraft draft = RuleDraft(
        duration: RememberDuration.session,
        target: RememberTarget.url,
        flow: githubFlow(method: Method.post),
      );
      // Schema und Port stehen im Satz, weil sie im Matcher stehen: eine
      // `url`-Regel nagelt beide fest, und ein Satz, der davon schwiege,
      // verspräche eine weitere Regel, als angelegt wird.
      expect(
        ruleSentence(draft, en, apexOf: apexOf),
        'allow · POST · api.github.com · /graphql · https · :443 · this session',
      );
      expect(
        ruleSentence(draft, de, apexOf: apexOf),
        'Erlauben · POST · api.github.com · /graphql · https · :443 · '
        'diese Session',
      );
    });

    test('block, host, session', () {
      final RuleDraft draft = RuleDraft(
        duration: RememberDuration.session,
        target: RememberTarget.host,
        flow: githubFlow(host: 'evil.example'),
        action: RuleAction.block,
      );
      expect(
        ruleSentence(draft, en, apexOf: apexOf),
        'block · ∗ · evil.example · this session',
      );
      expect(
        ruleSentence(draft, de, apexOf: apexOf),
        'Blockieren · ∗ · evil.example · diese Session',
      );
    });

    test('once says nothing, because no rule is created', () {
      final RuleDraft draft = RuleDraft(
        duration: RememberDuration.once,
        target: RememberTarget.host,
        flow: githubFlow(),
      );
      expect(ruleSentence(draft, en, apexOf: apexOf), isEmpty);
      expect(ruleSentence(draft, de, apexOf: apexOf), isEmpty);
    });
  });

  group('buildRule', () {
    final DateTime now = DateTime.utc(2026, 9, 3, 12);

    Rule? ruleFor(RememberDuration duration, RememberTarget target) =>
        buildRule(
          RuleDraft(duration: duration, target: target, flow: githubFlow()),
          now: now,
          apexOf: apexOf,
        );

    test('once creates no rule', () {
      expect(ruleFor(RememberDuration.once, RememberTarget.host), isNull);
    });

    test('host matches the host and nothing else', () {
      final Rule rule = ruleFor(RememberDuration.session, RememberTarget.host)!;
      expect(rule.matcher, const RuleMatcher(host: 'api.github.com'));
      expect(rule.expires, const RuleExpiry.session());
      expect(rule.action, RuleAction.allow);
      expect(rule.createdFrom, testFlowId(1));
      // The daemon assigns the id; the client never invents one.
      expect(rule.id, isNull);
    });

    test('host and method carries the method', () {
      final Rule rule = ruleFor(
        RememberDuration.forever,
        RememberTarget.hostMethod,
      )!;
      expect(
        rule.matcher,
        const RuleMatcher(
          host: 'api.github.com',
          methods: <Method>[Method.get],
        ),
      );
      expect(rule.expires, const RuleExpiry.never());
    });

    test('apex matches every label under the registrable domain', () {
      final Rule rule = ruleFor(RememberDuration.oneHour, RememberTarget.apex)!;
      expect(rule.matcher, const RuleMatcher(host: '**.github.com'));
      expect(
        rule.expires,
        RuleExpiry.at(at: now.add(const Duration(hours: 1))),
      );
    });

    test('url carries method, path, scheme and port, but never the query', () {
      final Rule rule = ruleFor(RememberDuration.session, RememberTarget.url)!;
      expect(
        rule.matcher,
        const RuleMatcher(
          host: 'api.github.com',
          methods: <Method>[Method.get],
          path: '/graphql',
          scheme: Scheme.https,
          port: 443,
        ),
      );
    });

    test('a block draft builds a block rule', () {
      final Rule rule = buildRule(
        RuleDraft(
          duration: RememberDuration.session,
          target: RememberTarget.host,
          flow: githubFlow(),
          action: RuleAction.block,
        ),
        now: now,
        apexOf: apexOf,
      )!;
      expect(rule.action, RuleAction.block);
    });
  });
}
