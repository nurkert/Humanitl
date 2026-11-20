// `SandboxBackendSegments` für sich (HUM-570): Eine Anforderung des Fokus,
// die vor dem Aufbau der Sperre gestellt, vom Fokus-Manager aber noch nicht
// vollzogen ist, gilt. Die Sperre fordert dann nicht den Knoten um die
// Segmente an, und das Entsperren gibt den Fokus nicht in die Wahl zurück.
// Im Bildschirm lässt sich diese Reihenfolge nicht herstellen: `pump` führt
// die vorgemerkten Wechsel vor jedem Bild aus. Hier stellt ein Elternteil die
// Anforderung in seinem eigenen Aufbau, vor dem der Segmente.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/sandbox_backend_choice.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Zwei Backends, beide verfügbar.
const SandboxBackendList twoBackends = SandboxBackendList(
  minIsolation: IsolationLevel.policy,
  autoBackend: 'first',
  backends: <SandboxBackendInfo>[
    SandboxBackendInfo(
      name: 'first',
      isolation: IsolationLevel.namespaces,
      available: true,
    ),
    SandboxBackendInfo(
      name: 'second',
      isolation: IsolationLevel.namespaces,
      available: true,
    ),
  ],
);

/// Die Segmente und ein zweites Bedienelement [other]. Sperrt [lock] die
/// Segmente, fordert der Aufbau dieses Elternteils zuerst den Fokus für
/// [other] an.
class _Harness extends StatefulWidget {
  const _Harness({required this.other});

  final FocusNode other;

  @override
  State<_Harness> createState() => _HarnessState();
}

class _HarnessState extends State<_Harness> {
  bool locked = false;
  bool requestOnLock = false;

  void lock({required bool requestOther}) => setState(() {
    locked = true;
    requestOnLock = requestOther;
  });

  void unlock() => setState(() => locked = false);

  @override
  Widget build(BuildContext context) {
    if (requestOnLock) {
      requestOnLock = false;
      widget.other.requestFocus();
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        SandboxBackendSegments(
          list: twoBackends,
          selected: 'second',
          onSelect: (_) {},
          enabled: !locked,
          segmentsKey: const Key('segments'),
        ),
        Focus(
          focusNode: widget.other,
          child: const SizedBox(width: 40, height: 20),
        ),
      ],
    );
  }
}

Widget host(Widget child) => WidgetsApp(
  color: HColors.bg0,
  debugShowCheckedModeBanner: false,
  locale: const Locale('en'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  onGenerateTitle: (BuildContext context) => 'segments',
  builder: (BuildContext context, Widget? _) => HTheme(
    tokens: HTokens.dark,
    child: Align(alignment: Alignment.topLeft, child: child),
  ),
);

/// Ob der Fokus auf einem Segment steht.
bool focusOnSegment() {
  final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
  return focused?.findAncestorWidgetOfExactType<HSegment<String>>() != null;
}

void main() {
  final TargetPlatformVariant linuxAndMac = TargetPlatformVariant(
    <TargetPlatform>{TargetPlatform.linux, TargetPlatform.macOS},
  );

  Future<FocusNode> pumpAndFocusSegment(WidgetTester tester) async {
    final FocusNode other = FocusNode(debugLabel: 'hum-570-other');
    addTearDown(other.dispose);
    await tester.pumpWidget(host(_Harness(other: other)));
    await tester.pumpAndSettle();
    Focus.of(tester.element(find.text('second'))).requestFocus();
    await tester.pump();
    expect(focusOnSegment(), isTrue);
    return other;
  }

  // Mutationsbeweis: die Sperre ohne den Blick auf den vorgemerkten Fokus
  // (`_nextFocusInChoice`) macht diesen Test rot.
  testWidgets('a focus request made before the locking rebuild wins', (
    WidgetTester tester,
  ) async {
    final FocusNode other = await pumpAndFocusSegment(tester);
    final _HarnessState harness = tester.state<_HarnessState>(
      find.byType(_Harness),
    );

    harness.lock(requestOther: true);
    await tester.pump();
    await tester.pump();
    expect(
      tester
          .widget<HSegmented<String>>(find.byKey(const Key('segments')))
          .enabled,
      isFalse,
    );
    expect(FocusManager.instance.primaryFocus, same(other));

    harness.unlock();
    await tester.pumpAndSettle();
    expect(FocusManager.instance.primaryFocus, same(other));
    expect(focusOnSegment(), isFalse);
  }, variant: linuxAndMac);

  // Gegenprobe ohne fremde Anforderung: Die Sperre fängt den Fokus auf, und
  // das Entsperren gibt ihn dem gewählten Segment zurück.
  testWidgets('without another request the lock catches and restores', (
    WidgetTester tester,
  ) async {
    final FocusNode other = await pumpAndFocusSegment(tester);
    final _HarnessState harness = tester.state<_HarnessState>(
      find.byType(_Harness),
    );

    harness.lock(requestOther: false);
    await tester.pump();
    await tester.pump();
    expect(
      FocusManager.instance.primaryFocus?.debugLabel,
      'sandbox-backend-choice',
    );
    expect(FocusManager.instance.primaryFocus, isNot(same(other)));

    harness.unlock();
    await tester.pumpAndSettle();
    expect(focusOnSegment(), isTrue);
  }, variant: linuxAndMac);
}
