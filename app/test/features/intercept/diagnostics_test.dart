// Unit-Tests von `diagnosticsProvider` (HUM-106): Was der Daemon meldet,
// kommt an, bleibt einzeln adressierbar und wird beim Übertreten der Grenze
// von fremdem Text befreit.
//
// Jede Zusicherung, die eine Schutzmaßnahme prüft, nennt in ihrem Kommentar
// die Änderung, die sie rot macht; die Proben sind von Hand gefahren worden.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/features/intercept/body/body_span.dart';
import 'package:humanitl/features/intercept/providers/diagnostics.dart';

import 'fixtures.dart';

/// Ein Container, in dem `diagnosticsProvider` einen Zuhörer hat.
///
/// Riverpod 3 pausiert einen Provider ohne Zuhörer samt seinem `ref.listen`;
/// im Baum hängen Widgets daran, hier dieser Zuhörer.
ProviderContainer makeContainer(TestDaemonClient client) {
  final ProviderContainer container = ProviderContainer.test(
    overrides: [
      daemonClientProvider.overrideWithValue(client),
      reconnectBackoffProvider.overrideWithValue(
        const Duration(milliseconds: 1),
      ),
    ],
  );
  container.listen(diagnosticsProvider, (_, _) {});
  return container;
}

/// Ein `TLS_001` mit dem Satz und dem Vorschlag des Daemons.
Diagnostic tls001({
  String why = 'curl in the sandbox does not trust the Humanitl CA yet',
}) => Diagnostic(
  code: 'TLS_001',
  severity: Severity.warning,
  why: why,
  fix: const FixAction.setEnv(
    key: 'CURL_CA_BUNDLE',
    value: '/etc/humanitl/ca.crt',
  ),
);

void main() {
  test('collects_every_diagnostic_of_the_stream', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(client);
    expect(container.read(diagnosticsProvider), isEmpty);
    await settle();

    client
      ..emit(
        FlowEvent.diagnostic(
          at: testStart,
          diagnostic: tls001(),
          flowId: const FlowId('018f0001-0000-7000-8000-000000060000'),
        ),
      )
      ..emit(
        FlowEvent.diagnostic(
          at: testStart.add(const Duration(seconds: 1)),
          diagnostic: tls001(why: 'another host, same code'),
        ),
      );
    await settle();

    final List<SessionDiagnostic> found = container.read(diagnosticsProvider);
    // Zwei Karten für zwei Befunde, auch bei gleichem Code: Der Daemon
    // entstört bereits je Host und Hinweis, diese Seite fasst nichts zusammen.
    // Rot, sobald hier nach `code` entdoppelt wird.
    expect(found, hasLength(2));
    expect(found.first.diagnostic.code, 'TLS_001');
    expect(found.last.diagnostic.code, 'TLS_001');
    expect(found.first.id, isNot(found.last.id));
    // Die Kennung des Flusses reist bis hierher durch, und ein Befund ohne
    // Kennung verschwindet trotzdem nicht.
    expect(
      found.first.flowId,
      const FlowId('018f0001-0000-7000-8000-000000060000'),
    );
    expect(found.last.flowId, isNull);
    expect(found.first.at, testStart);
  });

  test('dismiss_removes_only_that_one', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(client);
    expect(container.read(diagnosticsProvider), isEmpty);
    await settle();

    client
      ..emit(FlowEvent.diagnostic(at: testStart, diagnostic: tls001()))
      ..emit(
        FlowEvent.diagnostic(
          at: testStart,
          diagnostic: tls001(why: 'the second one'),
        ),
      );
    await settle();
    final List<SessionDiagnostic> both = container.read(diagnosticsProvider);
    expect(both, hasLength(2));

    container.read(diagnosticsProvider.notifier).dismiss(both.first.id);
    final List<SessionDiagnostic> left = container.read(diagnosticsProvider);
    expect(left, hasLength(1));
    expect(left.single.id, both.last.id);
    expect(left.single.diagnostic.why, 'the second one');

    // Ein späterer Befund desselben Codes ergibt eine neue Karte; das
    // Ausblenden merkt sich nichts. Rot, sobald `dismiss` den Code sperrt.
    client.emit(
      FlowEvent.diagnostic(
        at: testStart,
        diagnostic: tls001(why: 'again'),
      ),
    );
    await settle();
    final List<SessionDiagnostic> after = container.read(diagnosticsProvider);
    expect(after, hasLength(2));
    expect(after.last.diagnostic.why, 'again');
    expect(
      after.map((SessionDiagnostic entry) => entry.id),
      isNot(contains(both.first.id)),
    );
  });

  test('a_flow_event_that_is_no_diagnostic_changes_nothing', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(client);
    expect(container.read(diagnosticsProvider), isEmpty);
    await settle();

    final Flow flow = heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(minutes: 5)),
    );
    client
      ..emit(FlowEvent.received(at: testStart, flow: flow))
      ..emit(
        FlowEvent.held(
          at: testStart,
          flowId: flow.id,
          deadline: testStart.add(const Duration(minutes: 5)),
        ),
      )
      ..emit(
        FlowEvent.failed(
          at: testStart,
          flowId: flow.id,
          error: UpstreamError.tls,
        ),
      )
      ..emit(FlowEvent.lagged(at: testStart, dropped: 3));
    await settle();

    expect(container.read(diagnosticsProvider), isEmpty);
  });

  test('foreign_text_is_sanitized_at_this_boundary', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(client);
    expect(container.read(diagnosticsProvider), isEmpty);
    await settle();

    // Der Satz des Daemons trägt Material von außen: den Hostnamen, mit dem
    // der Handschlag scheiterte. U+202E dreht die Leserichtung um, U+200B ist
    // unsichtbar, U+2066 isoliert. Ohne Bereinigung liest ein Mensch auf der
    // Karte etwas anderes, als in der Zwischenablage landet.
    // Rot, sobald der Provider den Text unbereinigt weitergibt.
    const String hostile = 'handshake with evil\u202Emoc.rekcatta failed';
    const String hostilePath = '/etc/humanitl\u202E/ca.crt';
    client.emit(
      FlowEvent.diagnostic(
        at: testStart,
        diagnostic: const Diagnostic(
          code: 'TLS\u200B_001',
          title: 'CA not trusted\u202E',
          severity: Severity.warning,
          why: hostile,
          fix: FixAction.setEnv(
            key: 'CURL_CA\u200B_BUNDLE',
            value: hostilePath,
          ),
          docsUrl: 'https://example.invalid/#tls\u2066_001',
        ),
      ),
    );
    await settle();

    final SessionDiagnostic entry = container.read(diagnosticsProvider).single;
    // Jedes Feld, das Text traegt, nicht nur `why`.
    expect(entry.diagnostic.code, isNot(contains('\u200B')));
    expect(entry.diagnostic.code, contains(bodyReplacementChar));
    expect(entry.diagnostic.title, isNot(contains('\u202E')));
    expect(entry.diagnostic.title, contains(bodyReplacementChar));
    expect(entry.diagnostic.why, isNot(contains('\u202E')));
    expect(entry.diagnostic.why, contains(bodyReplacementChar));
    expect(entry.diagnostic.docsUrl, isNot(contains('\u2066')));
    final FixActionSetEnv fix = entry.diagnostic.fix! as FixActionSetEnv;
    expect(fix.key, isNot(contains('\u200B')));
    expect(fix.value, isNot(contains('\u202E')));
    // Ersetzt wird eins zu eins, gelöscht wird nichts: Die Länge bleibt.
    expect(fix.value.length, hostilePath.length);
  });

  test('the_list_stops_at_the_cap_and_counts_what_it_dropped', () async {
    // `TLS_001..003` entstoert der Daemon; `LLM_005`, `PROXY_002` und
    // `PROXY_005` nicht — sie entstehen je Anfrage, je widerspruechlicher
    // Zieladresse und je abgelehntem Uebergang, und der Agent loest sie selbst
    // aus. Ohne Grenze waere diese Liste ein Speicherleck, das er fuellt.
    // Rot, sobald der Ringpuffer faellt.
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(client);
    expect(container.read(diagnosticsProvider), isEmpty);
    await settle();

    // Literale, nicht `maxSessionDiagnostics`: Eine Zusicherung, die gegen
    // dieselbe Konstante rechnet, die sie prueft, bleibt gruen, wenn jemand
    // die Konstante auf 5000 setzt.
    const int cap = 200;
    const int sent = cap + 120;
    for (int i = 0; i < sent; i++) {
      client.emit(
        FlowEvent.diagnostic(
          at: testStart,
          diagnostic: Diagnostic(
            code: 'LLM_005',
            severity: Severity.warning,
            why: 'finding number $i',
          ),
        ),
      );
    }
    await settle();

    final List<SessionDiagnostic> found = container.read(diagnosticsProvider);
    expect(maxSessionDiagnostics, cap);
    expect(found, hasLength(cap));
    // Der aelteste faellt heraus, der juengste bleibt.
    expect(found.last.diagnostic.why, 'finding number ${sent - 1}');
    expect(found.first.diagnostic.why, 'finding number ${sent - cap}');
    // Und der Verlust wird gezaehlt, nicht verschwiegen (CONVENTIONS 4.13).
    expect(container.read(diagnosticsProvider.notifier).dropped, 120);
  });

  test('nothing is dropped while the list fits', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(client);
    expect(container.read(diagnosticsProvider), isEmpty);
    await settle();

    for (int i = 0; i < maxSessionDiagnostics; i++) {
      client.emit(
        FlowEvent.diagnostic(
          at: testStart,
          diagnostic: tls001(why: 'n$i'),
        ),
      );
    }
    await settle();

    expect(
      container.read(diagnosticsProvider),
      hasLength(maxSessionDiagnostics),
    );
    expect(container.read(diagnosticsProvider.notifier).dropped, 0);
  });

  group('every fix action passes the boundary', () {
    // Ueber alle Varianten, nicht ueber eine Auswahl: Der `switch` in
    // `sanitizeDiagnostic` ist erschoepfend und ohne Auffangzweig, also
    // bricht eine achte Variante den Bau — und diese Tabelle sagt, was fuer
    // die sieben von heute gilt.
    const String bad = 'evil\u202Emoc.rekcatta';
    final Map<String, FixAction> withText = <String, FixAction>{
      'setEnv': const FixAction.setEnv(key: 'K\u200BEY', value: bad),
      'changeSetting': const FixAction.changeSetting(
        key: 'a\u200Bb',
        value: bad,
      ),
      'copyCommand': const FixAction.copyCommand(command: bad),
      'openUrl': const FixAction.openUrl(url: 'https://$bad'),
      'remountReadOnly': const FixAction.remountReadOnly(path: '/work/$bad'),
      'addRule': const FixAction.addRule(
        rule: Rule(
          action: RuleAction.block,
          matcher: RuleMatcher(host: bad, path: '/$bad'),
          note: bad,
        ),
      ),
    };

    withText.forEach((String name, FixAction action) {
      test(name, () {
        final Diagnostic clean = sanitizeDiagnostic(
          Diagnostic(code: 'TLS_001', severity: Severity.warning, fix: action),
        );
        final String before = action.toString();
        final String after = clean.fix.toString();
        // Kein Zeichen, das die Leserichtung dreht, verlaesst diese Grenze —
        // in keiner Variante. Rot, sobald eine davon uebersprungen wird.
        expect(after, isNot(contains('\u202E')), reason: name);
        expect(after, isNot(contains('\u200B')), reason: name);
        // Ersetzt, nicht geloescht: Die Laenge bleibt. Ohne diese Zusicherung
        // bliebe ein `replaceAll(hostile, '')` unbemerkt.
        expect(after, contains(bodyReplacementChar), reason: name);
        expect(after.length, before.length, reason: name);
      });
    });

    test('installService carries no text and comes back unchanged', () {
      const FixAction action = FixAction.installService();
      final Diagnostic clean = sanitizeDiagnostic(
        const Diagnostic(
          code: 'TLS_001',
          severity: Severity.warning,
          fix: action,
        ),
      );
      expect(clean.fix, action);
    });
  });

  test('add_rule_keeps_every_field_it_does_not_touch', () async {
    // Die Grenze saeubert Text und erfindet nichts: Aktion, Ablauf und
    // Herkunft der vorgeschlagenen Regel bleiben, wie der Daemon sie schickte.
    final Diagnostic clean = sanitizeDiagnostic(
      Diagnostic(
        code: 'TLS_001',
        severity: Severity.warning,
        fix: FixAction.addRule(
          rule: Rule(
            action: RuleAction.allow,
            matcher: RuleMatcher(
              host: 'evil\u202Eexample.org',
              path: '/a\u200Bb',
              port: 8443,
            ),
            note: 'weil\u202Enicht',
            expires: const RuleExpiry.never(),
          ),
        ),
      ),
    );
    final FixActionAddRule fix = clean.fix! as FixActionAddRule;
    expect(fix.rule.action, RuleAction.allow);
    expect(fix.rule.expires, const RuleExpiry.never());
    expect(fix.rule.matcher.port, 8443);
    expect(fix.rule.matcher.host, isNot(contains('\u202E')));
    expect(fix.rule.matcher.host, contains(bodyReplacementChar));
    expect(fix.rule.matcher.path, isNot(contains('\u200B')));
    expect(fix.rule.note, isNot(contains('\u202E')));
  });
}
