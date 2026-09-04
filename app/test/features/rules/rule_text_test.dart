// Der Textteil des Rules-Screens ohne Widgets: der Satz (der eine Generator
// aus `core/text/rule_sentence.dart`, den auch die Aktionsleiste liest), die
// Restlaufzeit, die Vorprüfung der Muster und die Reihenfolge, die ein Zug
// erzeugt.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
// Die Datumsformate der ARB-Schlüssel brauchen die Locale-Daten. Im Programm
// lädt `GlobalMaterialLocalizations.delegate` sie mit; dieser Test hat keinen
// Widget-Baum und holt sie selbst.
import 'package:intl/date_symbol_data_local.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/text/rule_sentence.dart';
// Die andere Seite desselben Satzes. Ein Feature importiert kein anderes; ein
// Test, der beweist, dass beide Seiten denselben Generator lesen, muss beide
// sehen.
import 'package:humanitl/features/intercept/rule_sentence.dart' as intercept;
import 'package:humanitl/features/rules/providers/rules.dart';
import 'package:humanitl/features/rules/rule_text.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

void main() {
  late AppLocalizations en;
  late AppLocalizations de;

  setUpAll(() async {
    await initializeDateFormatting('en');
    await initializeDateFormatting('de');
    en = await AppLocalizations.delegate.load(const Locale('en'));
    de = await AppLocalizations.delegate.load(const Locale('de'));
  });

  group('rule_match_summary', () {
    test('lists the methods it names', () {
      final Rule rule = testRule(
        n: 1,
        methods: <Method>[Method.get, Method.head],
        host: '**.npmjs.org',
        path: '/**',
      );
      expect(ruleMatchSummary(rule, en), 'GET,HEAD · **.npmjs.org · /**');
    });

    test(
      'writes the star for a rule without methods and drops an empty path',
      () {
        final Rule rule = testRule(n: 2, host: 'api.github.com');
        expect(ruleMatchSummary(rule, en), '∗ · api.github.com');
      },
    );

    test('names scheme, port and upgrade when the rule pins them', () {
      final Rule rule = Rule(
        action: RuleAction.block,
        matcher: const RuleMatcher(
          host: 'ws.example.org',
          scheme: Scheme.wss,
          port: 8443,
          upgrade: Upgrade.websocket,
        ),
      );
      expect(
        ruleMatchSummary(rule, en),
        '∗ · ws.example.org · wss · :8443 · websocket',
      );
    });
  });

  group('rule_sentence', () {
    test('English names the policy verb, German the noun', () {
      final Rule rule = testRule(n: 3, host: 'api.github.com');
      expect(
        ruleSentence(rule, en, now: rulesTestNow),
        'allow · ∗ · api.github.com · always',
      );
      expect(
        ruleSentence(rule, de, now: rulesTestNow),
        'Erlauben · ∗ · api.github.com · immer',
      );
    });
  });

  group('arb', () {
    /// Die Schlüssel einer Sprachdatei, ohne die `@`-Einträge.
    Map<String, Object?> read(String locale) =>
        (jsonDecode(File('l10n/app_$locale.arb').readAsStringSync())
                as Map<String, Object?>)
            .cast<String, Object?>();

    test('every rules key carries its description, in both languages', () {
      final Map<String, Object?> source = read('en');
      final Map<String, Object?> german = read('de');
      final List<String> keys =
          source.keys
              .where((String key) => key.startsWith('rules') && key != 'rules')
              .toList()
            ..sort();
      expect(keys, isNotEmpty);
      for (final String key in keys) {
        // Ein Schlüssel ohne Beschreibung ist ein Schlüssel, den die nächste
        // Übersetzung raten muss -- auch einer, der auf sein Control wartet
        // (CONVENTIONS 4.16).
        expect(
          source['@$key'],
          isNotNull,
          reason: '$key has no @description in app_en.arb',
        );
        expect(
          german[key],
          isNotNull,
          reason: '$key is missing from app_de.arb',
        );
      }
    });
  });

  group('one generator', () {
    /// Der Apex kommt sonst vom Daemon; hier braucht ihn keiner der Fälle.
    String noApex(String host) => '';

    test('the sentence before a rule exists is the sentence after it', () {
      for (final intercept.RememberDuration duration
          in <intercept.RememberDuration>[
            intercept.RememberDuration.session,
            intercept.RememberDuration.oneHour,
            intercept.RememberDuration.forever,
          ]) {
        for (final intercept.RememberTarget target
            in <intercept.RememberTarget>[
              intercept.RememberTarget.url,
              intercept.RememberTarget.host,
              intercept.RememberTarget.hostMethod,
            ]) {
          final intercept.RuleDraft draft = intercept.RuleDraft(
            duration: duration,
            target: target,
            flow: testFlow(n: 1),
          );
          final Rule rule = intercept.buildRule(
            draft,
            now: rulesTestNow,
            apexOf: noApex,
          )!;
          for (final AppLocalizations l10n in <AppLocalizations>[en, de]) {
            // Ein Wortlaut für dieselbe Regel, vor dem Anlegen und danach.
            // Zwei Generatoren waren zwei Sätze, und mindestens einer sagte
            // etwas über den Verkehr, das nicht stimmte (CONVENTIONS 4.13).
            expect(
              intercept.ruleSentence(
                draft,
                l10n,
                apexOf: noApex,
                now: rulesTestNow,
              ),
              ruleSentence(rule, l10n, now: rulesTestNow),
            );
          }
        }
      }
    });
  });

  group('rule_expiry', () {
    test('a session rule says what ends it, in one wording', () {
      // Derselbe Wortlaut wie vor dem Anlegen: die Aktionsleiste und diese
      // Zeile lesen denselben Generator und denselben Schlüssel, sonst liest
      // sich dieselbe Regel vorher und nachher verschieden (CONVENTIONS
      // 4.13).
      expect(
        ruleExpiryLabel(const RuleExpiry.session(), en, now: rulesTestNow),
        'this session',
      );
      expect(
        ruleExpiryLabel(const RuleExpiry.session(), de, now: rulesTestNow),
        'diese Session',
      );
    });

    test('a rule that ends counts in the unit that fits', () {
      expect(
        ruleExpiryLabel(
          RuleExpiry.at(at: rulesTestNow.add(const Duration(minutes: 41))),
          en,
          now: rulesTestNow,
        ),
        'expires in 41 min',
      );
      // A minute that has started counts: rounding down would say the rule
      // holds shorter than it does.
      expect(
        ruleExpiryLabel(
          RuleExpiry.at(
            at: rulesTestNow.add(const Duration(minutes: 41, seconds: 30)),
          ),
          en,
          now: rulesTestNow,
        ),
        'expires in 42 min',
      );
      expect(
        ruleExpiryLabel(
          RuleExpiry.at(at: rulesTestNow.add(const Duration(hours: 5))),
          en,
          now: rulesTestNow,
        ),
        'expires in 5 h',
      );
      // Rounded up, like the minutes: an hour and 55 minutes is two hours of
      // rule left, not one.
      expect(
        ruleExpiryLabel(
          RuleExpiry.at(
            at: rulesTestNow.add(const Duration(hours: 1, minutes: 55)),
          ),
          en,
          now: rulesTestNow,
        ),
        'expires in 2 h',
      );
      expect(
        ruleExpiryLabel(
          RuleExpiry.at(at: rulesTestNow.add(const Duration(days: 9))),
          en,
          now: rulesTestNow,
        ),
        'expires in 9 d',
      );
    });

    test('a rule whose time has passed says so instead of counting down', () {
      expect(
        ruleExpiryLabel(
          RuleExpiry.at(at: rulesTestNow.subtract(const Duration(minutes: 1))),
          en,
          now: rulesTestNow,
        ),
        'expired',
      );
    });

    test('the exact end travels beside the coarse one', () {
      final String exact = ruleExpiryExact(RuleExpiry.at(at: rulesTestNow), en);
      expect(exact, isNotEmpty);
      expect(ruleExpiryExact(const RuleExpiry.never(), en), isEmpty);
    });
  });

  group('host_pre_check', () {
    test('a wildcard has to be a whole label', () {
      expect(
        hostPatternProblem('*npmjs.org'),
        HostPatternProblem.wildcardInLabel,
      );
      expect(hostPatternProblem('*.npmjs.org'), isNull);
      expect(hostPatternProblem('**.npmjs.org'), isNull);
    });

    test('an empty pattern and an empty label are refused', () {
      expect(hostPatternProblem(''), HostPatternProblem.empty);
      expect(hostPatternProblem('a..b'), HostPatternProblem.emptyLabel);
    });

    test('addresses need to be addresses', () {
      expect(hostPatternProblem('ip:192.168.1.50'), isNull);
      expect(hostPatternProblem('cidr:192.168.0.0/16'), isNull);
      expect(
        hostPatternProblem('ip:not-an-address'),
        HostPatternProblem.notAnAddress,
      );
      expect(
        hostPatternProblem('cidr:192.168.0.0'),
        HostPatternProblem.notAnAddress,
      );
    });

    test('a label carries no space, slash or colon', () {
      expect(hostPatternProblem('two words.org'), HostPatternProblem.notALabel);
      expect(hostPatternProblem('host:443'), HostPatternProblem.notALabel);
    });

    test('every reason has a sentence in both languages', () {
      for (final String pattern in <String>[
        '',
        '*npmjs.org',
        'a..b',
        'ip:nope',
        'two words.org',
      ]) {
        expect(hostProblemText(pattern, en), isNotNull);
        expect(hostProblemText(pattern, de), isNotNull);
      }
      expect(hostProblemText('**.npmjs.org', en), isNull);
    });

    test(
      'a path regex that cannot be built is caught before the round trip',
      () {
        expect(
          pathPatternProblem('~(unclosed'),
          PathPatternProblem.invalidRegex,
        );
        expect(pathPatternProblem('~^/repos/'), isNull);
        expect(pathPatternProblem('/**'), isNull);
      },
    );
  });

  group('chain_order_after_move', () {
    RuleSet set() => RuleSet(
      rules: <Rule>[
        testRule(n: 1, expires: const RuleExpiry.session()),
        testRule(n: 2, expires: const RuleExpiry.session()),
        testRule(n: 3),
        testRule(n: 4),
        testRule(n: 5),
        testRule(n: 9, bundled: true),
      ],
    );

    test('moving a saved rule leaves the session rules where they are', () {
      final List<RuleId> order = chainOrderAfterMove(
        set(),
        RuleTab.saved,
        from: 2,
        to: 0,
      );
      expect(order, <RuleId>[
        testRuleId(1),
        testRuleId(2),
        testRuleId(5),
        testRuleId(3),
        testRuleId(4),
      ]);
    });

    test('a bundled rule is never named: it cannot move', () {
      final List<RuleId> order = chainOrderAfterMove(
        set(),
        RuleTab.saved,
        from: 0,
        to: 2,
      );
      expect(order.contains(testRuleId(9)), isFalse);
      expect(order, hasLength(5));
    });

    test('moving a session rule keeps the saved chain intact', () {
      final List<RuleId> order = chainOrderAfterMove(
        set(),
        RuleTab.temporary,
        from: 0,
        to: 1,
      );
      expect(order, <RuleId>[
        testRuleId(2),
        testRuleId(1),
        testRuleId(3),
        testRuleId(4),
        testRuleId(5),
      ]);
    });
  });

  group('tab_of', () {
    test(
      'only a session rule is temporary; a rule with an end is written down',
      () {
        expect(tabOf(testRule(n: 1)), RuleTab.saved);
        expect(
          tabOf(testRule(n: 2, expires: const RuleExpiry.session())),
          RuleTab.temporary,
        );
        expect(
          tabOf(testRule(n: 3, expires: RuleExpiry.at(at: rulesTestNow))),
          RuleTab.saved,
        );
      },
    );
  });
}
