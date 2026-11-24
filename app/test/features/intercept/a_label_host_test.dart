// Der Host einer Anfrage steht als A-Label da, wo ein Mensch entscheidet
// (GitHub #103, `docs/SECURITY.md`, Normalisierung der Hostnamen). Die
// Unicode-Form aus `Authority.displayHost` erscheint nur daneben und als
// solche benannt, nie allein und nie an seiner Stelle.
//
// Gegen welche Umkehrung jeder Test gerichtet ist:
//
// - `Flow.url` oder `Authority.display` baut den Host wieder aus
//   `displayHost`/`shownHost`: `the_card_header_shows_the_a_label` und
//   `the_domain_builds_urls_from_the_a_label` werden rot.
// - `Flow.host` liefert wieder `shownHost`:
//   `the_domain_builds_urls_from_the_a_label` und
//   `the_terminal_strip_names_the_a_label` werden rot.
// - Die Zeile mit der Unicode-Form fehlt oder steht ohne Kennzeichnung:
//   `the_card_header_shows_the_a_label` wird rot.
// - Ein Regelmuster nimmt das A-Label statt des Namens:
//   `a_rule_pattern_keeps_the_written_name` wird rot.
// - Knopf oder Satz einer Regel zeigen den Wert des Musters statt des
//   A-Labels (`shownHostPattern` gibt `hostPattern` zurück, oder
//   `requestRuleSentence` tauscht den Host nicht):
//   `the_quick_rule_label_names_the_a_label` und
//   `the_rule_sentence_names_the_a_label` werden rot.
// - Der Fake vergleicht ein Muster mit dem Namen nur gegen das A-Label:
//   `the_fake_matches_a_written_name_against_both_forms` wird rot.

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/features/intercept/widgets/domain_panel.dart';
import 'package:humanitl/features/intercept/widgets/request_card.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../goldens/support/golden_variants.dart';
import '../../harness/ui_state.dart';
import 'fixtures.dart';

const String aLabel = 'xn--mnchen-3ya.de';
const String uLabel = 'münchen.de';

/// Ein gehaltener Flow an [aLabel], dessen Unicode-Form [uLabel] ist.
Flow idnFlow() {
  final Flow flow = heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 4)),
    host: aLabel,
    path: '/api',
  );
  return flow.copyWith(authority: flow.authority.copyWith(displayHost: uLabel));
}

/// Die Karte zu [flow], in [variant].
Widget card(Flow flow, {GoldenVariant variant = GoldenVariant.darkEn}) {
  final TestDaemonClient client = TestDaemonClient();
  client.details[flow.id] = detailFor(flow);
  return goldenFrame(
    variant: variant,
    overrides: <Override>[
      uiStateOverride(),
      daemonClientProvider.overrideWithValue(client),
      connectionHeartbeatProvider.overrideWithValue(null),
      nowProvider.overrideWith(() => FixedNow(testStart)),
      flowsProvider.overrideWith(
        () => FixedFlows(<FlowId, Flow>{flow.id: flow}),
      ),
    ],
    child: RequestCard(flow: flow),
  );
}

Future<void> show(
  WidgetTester tester,
  Flow flow, {
  GoldenVariant variant = GoldenVariant.darkEn,
}) async {
  await tester.binding.setSurfaceSize(const Size(1000, 700));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(card(flow, variant: variant));
  await tester.pump();
  await tester.pump();
}

String cardUrl(WidgetTester tester) => tester
    .widget<Text>(
      find.descendant(
        of: find.byKey(const Key('intercept-card-url')),
        matching: find.byType(Text),
      ),
    )
    .data!;

RuleDraft hostDraft(RuleAction action) => RuleDraft(
  duration: RememberDuration.session,
  target: RememberTarget.host,
  flow: idnFlow(),
  action: action,
);

String noApex(String host) => '';

void main() {
  late AppLocalizations en;
  late AppLocalizations de;

  setUpAll(() async {
    en = await AppLocalizations.delegate.load(const Locale('en'));
    de = await AppLocalizations.delegate.load(const Locale('de'));
  });

  test('the_domain_builds_urls_from_the_a_label', () {
    final Flow flow = idnFlow();
    expect(flow.host, aLabel);
    expect(flow.url, 'https://$aLabel/api');
    expect(flow.authority.display(Scheme.https), aLabel);
    expect(flow.authority.unicodeHost, uLabel);
    // Die Exporte behalten ihre Form (`host_display`, URL in curl und HAR).
    expect(flow.authority.shownHost, uLabel);
    expect(flow.authority.shownDisplay(Scheme.https), uLabel);
    final Authority ported = flow.authority.copyWith(port: 8443);
    expect(ported.display(Scheme.https), '$aLabel:8443');
  });

  test('a_host_without_a_unicode_form_is_unchanged', () {
    const Authority plain = Authority(host: 'api.github.com', port: 443);
    expect(plain.display(Scheme.https), 'api.github.com');
    expect(plain.unicodeHost, isEmpty);
    // Schickt der Daemon die Unicode-Form gleich dem A-Label, gibt es keine.
    expect(plain.copyWith(displayHost: 'api.github.com').unicodeHost, isEmpty);
  });

  testWidgets('the_card_header_shows_the_a_label', (WidgetTester tester) async {
    await show(tester, idnFlow());
    expect(cardUrl(tester), 'https://$aLabel/api');
    final Finder unicode = find.byKey(const Key('intercept-card-unicode-host'));
    expect(unicode, findsOneWidget);
    expect(tester.widget<Text>(unicode).data, 'Unicode form: $uLabel');
    // Die Unicode-Form steht nirgends sonst auf der Karte, auch nicht im
    // Hinweis zum WebSocket-Aufbau oder in einer zweiten URL.
    expect(
      find.byWidgetPredicate(
        (Widget widget) =>
            widget is Text &&
            (widget.data ?? '').contains(uLabel) &&
            widget.key != const Key('intercept-card-unicode-host'),
      ),
      findsNothing,
    );
  });

  testWidgets('a_plain_host_has_no_unicode_line', (WidgetTester tester) async {
    await show(
      tester,
      heldFlow(n: 2, deadline: testStart.add(const Duration(minutes: 4))),
    );
    expect(cardUrl(tester), 'https://api.github.com/graphql');
    expect(find.byKey(const Key('intercept-card-unicode-host')), findsNothing);
  });

  test('a_rule_pattern_keeps_the_written_name', () {
    // Ein Regelmuster ist keine Anzeige einer Anfrage: Der Daemon normalisiert
    // es selbst und warnt vor einem Punycode-Label darin (`RULES_002`).
    final RuleDraft draft = RuleDraft(
      duration: RememberDuration.session,
      target: RememberTarget.host,
      flow: idnFlow(),
    );
    expect(hostPattern(draft, (String host) => ''), uLabel);
  });

  test('the_terminal_strip_names_the_a_label', () async {
    final StreamController<FlowEvent> events =
        StreamController<FlowEvent>.broadcast();
    addTearDown(events.close);
    final ProviderContainer container = ProviderContainer(
      overrides: <Override>[
        flowEventsProvider.overrideWith((Ref ref) => events.stream),
      ],
    );
    addTearDown(container.dispose);
    final ProviderSubscription<TerminalNotice?> notice = container.listen(
      heldNoticeProvider,
      (TerminalNotice? previous, TerminalNotice? next) {},
    );
    final Flow flow = idnFlow().copyWith(state: FlowState.received);
    events
      ..add(FlowEvent.received(at: testStart, flow: flow))
      ..add(
        FlowEvent.held(
          at: testStart,
          flowId: flow.id,
          deadline: testStart.add(const Duration(minutes: 5)),
        ),
      );
    await pumpEventQueue();
    expect(notice.read()?.host, aLabel);
  });

  testWidgets('the_unicode_line_speaks_german', (WidgetTester tester) async {
    await show(tester, idnFlow(), variant: GoldenVariant.darkDe);
    expect(cardUrl(tester), 'https://$aLabel/api');
    expect(
      tester
          .widget<Text>(find.byKey(const Key('intercept-card-unicode-host')))
          .data,
      'Unicode-Form: $uLabel',
    );
  });

  test('the_quick_rule_label_names_the_a_label', () {
    expect(
      quickRuleLabel(hostDraft(RuleAction.allow), en, noApex),
      'Allow $aLabel (Unicode form: $uLabel) this session',
    );
    expect(
      quickRuleLabel(hostDraft(RuleAction.block), de, noApex),
      '$aLabel (Unicode-Form: $uLabel) blockieren',
    );
    // Ohne Unicode-Form bleibt die Aufschrift, wie sie war.
    expect(
      quickRuleLabel(
        RuleDraft(
          duration: RememberDuration.session,
          target: RememberTarget.host,
          flow: heldFlow(n: 2, deadline: testStart),
        ),
        en,
        noApex,
      ),
      'Allow api.github.com this session',
    );
  });

  test('the_rule_sentence_names_the_a_label', () {
    final String text = ruleSentence(
      hostDraft(RuleAction.allow),
      en,
      apexOf: noApex,
      now: testStart,
    );
    expect(text, contains('$aLabel (Unicode form: $uLabel)'));
    expect(
      text.replaceAll('(Unicode form: $uLabel)', ''),
      isNot(contains(uLabel)),
    );
  });

  test('the_fake_matches_a_written_name_against_both_forms', () {
    final Flow flow = idnFlow();
    for (final String pattern in <String>[uLabel, aLabel, '**.$uLabel']) {
      expect(
        ruleMatchesFlow(
          Rule(
            action: RuleAction.allow,
            matcher: RuleMatcher(host: pattern),
          ),
          flow,
          now: testStart,
        ),
        isTrue,
        reason: pattern,
      );
    }
  });
}
