// Bausteine der Rules-Tests: ein Client, der mitzählt, was der Screen fragt,
// die Regeln, mit denen er gefüttert wird, und ein Baum, der ohne die Shell
// steht (der Screen ist eine Sektion, aber er hängt an nichts aus ihr).

// `Flow` ist hier ein Domänentyp, nicht das Layout-Widget gleichen Namens.
import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/rules/providers/rules.dart';
import 'package:humanitl/features/rules/rules_screen.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Der Zeitpunkt, gegen den jeder Test rechnet.
final DateTime rulesTestNow = DateTime.utc(2026, 9, 3, 12);

/// Die Id der n-ten Testregel.
RuleId testRuleId(int n) =>
    RuleId('018f0040-0000-7000-8000-${n.toString().padLeft(12, '0')}');

/// Eine Regel mit den Feldern, die der Test braucht.
Rule testRule({
  required int n,
  RuleAction action = RuleAction.allow,
  String host = 'registry.npmjs.org',
  List<Method> methods = const <Method>[],
  String path = '',
  RuleExpiry expires = const RuleExpiry.never(),
  String? note,
  FlowId? createdFrom,
  bool bundled = false,
  bool disabled = false,
  bool stream = false,
  int position = 0,
}) => Rule(
  id: testRuleId(n),
  action: action,
  matcher: RuleMatcher(host: host, methods: methods, path: path),
  expires: expires,
  note: note,
  createdFrom: createdFrom,
  bundled: bundled,
  disabled: disabled,
  stream: stream,
  position: position,
  createdAt: rulesTestNow,
);

/// Ein aufgezeichneter Flow für den Probelauf.
Flow testFlow({
  required int n,
  Method method = Method.get,
  String host = 'registry.npmjs.org',
  String path = '/react',
}) => Flow(
  id: FlowId('018f0041-0000-7000-8000-${n.toString().padLeft(12, '0')}'),
  sessionId: FakeDaemonClient.defaultSession,
  receivedAt: rulesTestNow.add(Duration(seconds: n)),
  method: method,
  scheme: Scheme.https,
  authority: Authority(host: host, port: 443),
  path: path,
  state: FlowState.recorded,
);

/// Der Fake-Daemon, der mitschreibt, was der Screen von ihm wollte.
///
/// Er erbt die ganze Regel-Mechanik des ausgelieferten Fakes -- Gruppen,
/// Positionen, Prüfung, Probelauf -- und legt nur die Buchhaltung darüber,
/// damit ein Test „ein RPC, nicht drei" belegen kann.
class RulesTestClient extends FakeDaemonClient {
  /// Erzeugt einen Client ohne Skript; die Rules-Tests abonnieren nichts.
  RulesTestClient() : super(script: const <ScriptedEvent>[]);

  /// Jede Reihenfolge, die `Rules(reorder)` bekam, älteste zuerst.
  final List<List<RuleId>> reorders = <List<RuleId>>[];

  /// Jede Regel, die `Rules(add)` bekam.
  final List<Rule> added = <Rule>[];

  /// Jede Regel, die `Rules(update)` bekam.
  final List<Rule> updated = <Rule>[];

  /// Jede Id, die `Rules(remove)` bekam.
  final List<RuleId> removed = <RuleId>[];

  /// Jede Id, die `Rules(make_permanent)` bekam.
  final List<RuleId> permanent = <RuleId>[];

  /// Jedes `Rules(set_disabled)`, ältestes zuerst: Id und gewünschter Zustand.
  final List<(RuleId, bool)> setDisabledCalls = <(RuleId, bool)>[];

  /// Jede Regel, mit der ein Probelauf lief.
  final List<Rule> dryRuns = <Rule>[];

  /// Wie oft `Rules(list)` gefragt wurde.
  int listCalls = 0;

  /// Wie oft `Rules(reload)` gefragt wurde.
  int reloadCalls = 0;

  /// Was jede `Rules`-Antwort an Befunden trägt.
  ///
  /// Der Daemon füllt `RulesResponse.diagnostics` heute nur beim Reload; das
  /// Feld steht aber an jeder Antwort, und der Screen muss es überall lesen.
  List<Diagnostic> answerDiagnostics = const <Diagnostic>[];

  /// Was `Rules(dry_run)` statt einer Antwort wirft, wenn gesetzt.
  Diagnostic? dryRunFailure;

  /// Was `Rules(add)` statt einer Antwort wirft, wenn gesetzt.
  ///
  /// Die Prüfung des Fakes ist dieselbe wie die Vorprüfung des Formulars;
  /// damit ein Test den Fall „der Daemon lehnt ab, obwohl das Formular
  /// zufrieden war" zeigen kann, braucht er diesen Schalter.
  Diagnostic? addFailure;

  @override
  Future<RuleSet> listRules() async {
    listCalls++;
    return _reported(await super.listRules());
  }

  /// Hängt [answerDiagnostics] an eine Antwort, so wie der Daemon es täte.
  RuleSet _reported(RuleSet answered) => answerDiagnostics.isEmpty
      ? answered
      : RuleSet(rules: answered.rules, diagnostics: answerDiagnostics);

  @override
  Future<RuleSet> addRule(Rule rule) async {
    added.add(rule);
    final Diagnostic? failure = addFailure;
    if (failure != null) {
      throw DaemonException(failure);
    }
    return _reported(await super.addRule(rule));
  }

  @override
  Future<RuleSet> updateRule(Rule rule) {
    updated.add(rule);
    return super.updateRule(rule);
  }

  @override
  Future<void> removeRule(RuleId id) {
    removed.add(id);
    return super.removeRule(id);
  }

  @override
  Future<RuleSet> reorderRules(List<RuleId> order) {
    reorders.add(List<RuleId>.of(order));
    return super.reorderRules(order);
  }

  @override
  Future<RuleSet> makeRulePermanent(RuleId id) {
    permanent.add(id);
    return super.makeRulePermanent(id);
  }

  @override
  Future<RuleSet> reloadRules() {
    reloadCalls++;
    return super.reloadRules();
  }

  @override
  Future<RuleSet> setRuleDisabled(RuleId id, {required bool disabled}) {
    setDisabledCalls.add((id, disabled));
    return super.setRuleDisabled(id, disabled: disabled);
  }

  @override
  Future<DryRun> dryRunRule(Rule rule, {int limit = dryRunScanDefault}) {
    dryRuns.add(rule);
    final Diagnostic? failure = dryRunFailure;
    if (failure != null) {
      throw DaemonException(failure);
    }
    return super.dryRunRule(rule, limit: limit);
  }
}

/// Baut den Rules-Screen über [client] und pumpt, bis der Regelsatz steht.
Future<void> pumpRules(
  WidgetTester tester, {
  required RulesTestClient client,
  HThemeMode mode = HThemeMode.dark,
  TextScaler textScaler = TextScaler.noScaling,
  bool reducedMotion = false,
  Size size = const Size(1280, 800),
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    rulesUnderTest(
      client: client,
      mode: mode,
      textScaler: textScaler,
      reducedMotion: reducedMotion,
    ),
  );
  // Ein Frame für `Rules(list)`, einer für die Antwort, einer für den Aufbau.
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

/// Der Screen in einem Baum ohne Shell.
Widget rulesUnderTest({
  required RulesTestClient client,
  HThemeMode mode = HThemeMode.dark,
  TextScaler textScaler = TextScaler.noScaling,
  bool reducedMotion = false,
  List<Override> overrides = const <Override>[],
}) {
  final HTokens tokens = mode.resolve(Brightness.dark);
  return ProviderScope(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(client),
      ...overrides,
    ],
    child: WidgetsApp(
      color: tokens.colors.bg0,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      builder: (BuildContext context, Widget? _) => MediaQuery(
        data: MediaQueryData(
          textScaler: textScaler,
          disableAnimations: reducedMotion,
        ),
        child: HTheme(
          tokens: tokens,
          child: ColoredBox(
            color: tokens.colors.bg0,
            // Wie `app.dart`: kein Navigator, ein Overlay für alles, was
            // schwebt.
            child: Overlay(
              initialEntries: <OverlayEntry>[
                OverlayEntry(
                  builder: (BuildContext context) => const RulesScreen(),
                ),
              ],
            ),
          ),
        ),
      ),
    ),
  );
}

/// Fährt mit dem Zeiger auf [finder] und lässt ihn dort stehen.
///
/// Zeilenaktionen erscheinen bei Hover und bei Fokus; ein Test, der klickt,
/// ohne vorher zu fahren, prüft einen Weg, den es nicht gibt.
Future<TestGesture> hoverOver(WidgetTester tester, Finder finder) async {
  final TestGesture gesture = await tester.createGesture(
    kind: PointerDeviceKind.mouse,
  );
  await gesture.addPointer(location: Offset.zero);
  addTearDown(gesture.removePointer);
  await gesture.moveTo(tester.getCenter(finder));
  await tester.pumpAndSettle();
  return gesture;
}

/// Setzt den Fokus auf die Regel-Zeile mit dem Index [index].
void focusRow(WidgetTester tester, int index) {
  final FocusableActionDetector detector = tester
      .widget<FocusableActionDetector>(
        find
            .descendant(
              of: find.byType(RuleRow).at(index),
              matching: find.byType(FocusableActionDetector),
            )
            .first,
      );
  detector.focusNode!.requestFocus();
}

/// Legt [rule] auf demselben Weg an, den der Editor nimmt: über den Notifier,
/// nicht am Client vorbei. Der Screen erfährt von einer Regel nur, wenn er
/// selbst gefragt hat -- der Regelsatz kommt mit jeder Antwort vollständig
/// zurück.
Future<void> addRuleThroughApp(WidgetTester tester, Rule rule) async {
  final ProviderContainer container = ProviderScope.containerOf(
    tester.element(find.byType(RuleRow).first),
  );
  await container.read(rulesProvider.notifier).add(rule);
}
