// Der Rundlauf der Herkunft, gemessen statt von Hand nachgespielt: Eine Regel
// entsteht im Intercept-Bildschirm über „Remember", sie steht danach im Tab
// „Temporary", trägt dort das Abzeichen `rulesOriginFlow` mit der kurzen Id
// der Anfrage, und ein Klick darauf führt zurück in die Warteschlange, auf
// genau die Anfrage, aus der sie entstanden ist.
//
// Das ist die andere Hälfte von `rules_screen_test.dart` („the origin of a
// rule is a control that reaches the request"): Dort steht eine Regel mit
// gesetztem `createdFrom` schon im Fake, hier legt der Bildschirm sie selbst
// an, und die Übergabe wird bis zur Warteschlange verfolgt. Der Weg über die
// ganze Anwendung ist nötig, weil nicht das Abzeichen die Übergabe ausführt,
// sondern die Shell: Ein Feature greift nicht in ein anderes (ARCHITECTURE 5),
// das Abzeichen bittet nur (`backlog/CONVENTIONS.md` 4.16).

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_handoff.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/intercept_screen.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../../harness/fixed_now.dart';

/// Die Fensterbreite dieses Tests.
///
/// Das Abzeichen der Herkunft ist das Erste, was aus der Zeile weicht: unter
/// [ruleRowOriginBelow] Zeilenbreite steht es nicht mehr da. Die Liste bekommt
/// `rulesListFraction` der Fläche neben der 48 px breiten Leiste, also braucht
/// dieser Rundlauf ein breiteres Fenster als die übrigen Widget-Tests.
const Size roundTripWindow = Size(1600, 1000);

void main() {
  late AppLocalizations l10n;

  setUpAll(() async {
    l10n = await AppLocalizations.delegate.load(const Locale('en'));
  });

  testWidgets('a remembered decision becomes a rule that leads back to its '
      'request', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient.burst(count: 3);
    await pumpApp(
      tester,
      client: client,
      size: roundTripWindow,
      overrides: <Override>[
        // Die eine UI-Uhr steht still. Ohne das läuft ihr 250-ms-Ticker über
        // das Ende des Tests hinaus, und das Gerüst meldet einen offenen
        // Timer statt eines Ergebnisses.
        nowProvider.overrideWith(() => FixedNow(DateTime.now())),
      ],
    );
    // Das Skript hält drei Anfragen im Abstand von 100 ms; die Vorgabe liegt
    // über `HMotion.rearm`, also ist die Auswahl danach scharf.
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(HumanitlApp)),
    );
    final FlowId? selected = container.read(selectedFlowIdProvider);
    expect(selected, isNotNull, reason: 'the queue holds three requests');
    final FlowId flowId = selected!;

    // Der Entwurf, den die Aktionsleiste und die Tasten `2` und `Shift+2`
    // setzen: die Sitzung als Frist, der Host als Umfang. Nicht die
    // registrierbare Domäne -- ohne Antwort des Daemons zum Apex ist
    // `apexResolverProvider` leer, und der Umfang würde mit Grund abgelehnt
    // (`decision.dart`, `RefusalReason.apexUnknown`).
    final RememberDraft draft = container.read(rememberDraftProvider.notifier);
    draft.setDuration(RememberDuration.session);
    draft.setTarget(RememberTarget.host);
    expect(
      container.read(rememberDraftProvider),
      const RememberState(
        open: true,
        duration: RememberDuration.session,
        target: RememberTarget.host,
      ),
    );

    await container.read(interceptDecisionProvider.notifier).allow();
    await tester.pump();
    await tester.pump();

    // Eine Regel, aus dieser Anfrage, nur für diese Sitzung, auf den Host.
    expect(client.rules, hasLength(1));
    final Rule created = client.rules.single;
    expect(created.createdFrom, flowId);
    expect(created.expires, const RuleExpiry.session());
    expect(created.matcher.host, 'registry.npmjs.org');
    expect(created.matcher.methods, isEmpty);
    expect(created.matcher.path, isEmpty);

    // Der Regel-Bildschirm fragt neu, sobald er sichtbar wird; er erfährt von
    // einer Regel nur, wenn er selbst gefragt hat (`rules.dart`).
    container.read(navigationProvider.notifier).go(Section.rules);
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    // Der richtige Tab: Die Regel läuft mit der Sitzung ab, also steht sie
    // unter „Temporary" und nicht unter „Saved", wo die mitgelieferte Regel
    // des Fakes allein bleibt.
    expect(find.text(l10n.rulesTabSaved(1)), findsOneWidget);
    // Nicht nur die Zahl am Tab: Der offene Tab „Saved" zeigt genau eine
    // Zeile, die mitgelieferte, und das Abzeichen dieser Anfrage steht nicht
    // darin. Eine Liste, die beide Regeln zeigte und trotzdem richtig zählte,
    // bliebe sonst unbemerkt (Befund des Reviews).
    expect(find.byType(RuleRow), findsOneWidget);
    expect(
      find.byKey(ValueKey<String>('rule-origin-${flowId.value}')),
      findsNothing,
    );

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pump();
    expect(find.byType(RuleRow), findsOneWidget);

    // Das Abzeichen: der Schlüssel trägt die ganze Id, der Text das letzte
    // Segment. Nicht `from #n` -- es gibt keine laufende Nummer einer
    // Anfrage, und ein Abzeichen, das eine erfände, zeigte auf nichts
    // (`backlog/CONVENTIONS.md` 4.16).
    final Finder origin = find.byKey(
      ValueKey<String>('rule-origin-${flowId.value}'),
    );
    expect(origin, findsOneWidget);
    expect(
      find.descendant(
        of: origin,
        matching: find.text(l10n.rulesOriginFlow(flowId.value.split('-').last)),
      ),
      findsOneWidget,
    );
    expect(container.read(flowHandoffProvider), isNull);

    await tester.tap(origin);
    await tester.pump();
    await tester.pump();

    // Und der Klick landet in der Warteschlange, auf der Anfrage selbst: Das
    // Abzeichen bittet, die Shell führt aus und löscht die Bitte, damit sie
    // nicht bei jedem Neubau ein zweites Mal ausgeführt wird.
    expect(container.read(navigationProvider), Section.intercept);
    expect(container.read(selectedFlowIdProvider), flowId);
    expect(container.read(flowHandoffProvider), isNull);
    // Und der Bildschirm steht auch: Ein Zustand, den niemand zeichnet, ist
    // keine Landung (Befund des Reviews).
    expect(find.byType(InterceptScreen), findsOneWidget);

    // Die neue Auswahl schärft sich wieder; der Timer dafür läuft sonst über
    // das Ende des Tests hinaus.
    await tester.pump(HMotion.rearm);
    await tester.pump();
  });
}
