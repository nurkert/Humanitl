// HUM-378: the header bar reports its height, so the traffic lights stay
// centred on it at every scale.
// HUM-383: in full screen the bar frees the room of the hidden traffic lights
// and makes room again while macOS reveals them.

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/traffic_lights.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ui/ui_scale.dart';
import 'package:humanitl/features/shell/widgets/window_frame.dart';

void main() {
  final List<Object?> sent = <Object?>[];
  final List<String> asked = <String>[];
  // What the Runner answers to `getTrafficLights`.
  String? current;

  setUp(() {
    sent.clear();
    asked.clear();
    current = null;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(windowChannel, (MethodCall call) async {
          asked.add(call.method);
          if (call.method == 'getTrafficLights') {
            return current;
          }
          sent.add((call.arguments as Map<Object?, Object?>)['height']);
          return null;
        });
  });

  Widget at(int scale) => ProviderScope(
    child: UiScaleScope(
      preferred: scale,
      limit: 300,
      effective: scale,
      child: const TrafficLightSync(child: SizedBox()),
    ),
  );

  testWidgets('macOS hears each new height once', (WidgetTester tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
    await tester.pumpWidget(at(100));
    await tester.pumpWidget(at(150));
    await tester.pumpWidget(at(150));
    expect(sent, <double>[40, 60]);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('Linux hears nothing', (WidgetTester tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
    await tester.pumpWidget(at(150));
    expect(sent, isEmpty);
    expect(asked, isEmpty);
    debugDefaultTargetPlatformOverride = null;
  });

  group('full screen (HUM-383)', () {
    late EdgeInsets padding;

    Widget bar({int scale = 100, bool reduced = false}) => ProviderScope(
      child: MediaQuery(
        data: MediaQueryData(disableAnimations: reduced),
        child: UiScaleScope(
          preferred: scale,
          limit: uiScaleMax,
          effective: scale,
          child: TrafficLightSync(
            child: Builder(
              builder: (BuildContext context) {
                padding = titleBarPadding(context, 12);
                return const SizedBox.shrink();
              },
            ),
          ),
        ),
      ),
    );

    // What the Runner sends when the traffic lights move.
    Future<void> report(WidgetTester tester, Object? state) =>
        tester.binding.defaultBinaryMessenger.handlePlatformMessage(
          windowChannel.name,
          windowChannel.codec.encodeMethodCall(
            MethodCall('trafficLights', state),
          ),
          (ByteData? _) {},
        );

    TrafficLightState state(WidgetTester tester) =>
        ProviderScope.containerOf(tester.element(find.byType(TrafficLightSync)))
            .read(trafficLightStateProvider);

    testWidgets('the room glides with all three states', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(bar());
      expect(state(tester), TrafficLightState.windowed);
      expect(padding.left, 12 + trafficLightInset);

      // Into full screen: the content glides left into the freed room.
      await report(tester, 'fullScreenHidden');
      expect(state(tester), TrafficLightState.fullScreenHidden);
      await tester.pump();
      await tester.pump(HMotion.arrive ~/ 2);
      expect(padding.left, greaterThan(12));
      expect(padding.left, lessThan(12 + trafficLightInset));
      // The entry curve is ahead of the linear middle.
      expect(
        padding.left,
        closeTo(12 + trafficLightInset * (1 - HMotion.enter.transform(0.5)), 1),
      );
      // Done within the duration of the token; the first frame only starts
      // the clock, so one more frame ends it.
      await tester.pump(HMotion.arrive ~/ 2);
      await tester.pump(const Duration(milliseconds: 17));
      expect(padding.left, 12);
      expect(tester.hasRunningAnimations, isFalse);
      expect(padding.right, 12);

      // The pointer reaches the top edge, macOS reveals the buttons.
      await report(tester, 'fullScreenRevealed');
      await tester.pump();
      await tester.pump(HMotion.arrive ~/ 2);
      expect(padding.left, greaterThan(12));
      await tester.pumpAndSettle();
      expect(padding.left, 12 + trafficLightInset);

      // And hides them again.
      await report(tester, 'fullScreenHidden');
      await tester.pumpAndSettle();
      expect(padding.left, 12);

      // Out of full screen: the layout of the window.
      await report(tester, 'windowed');
      await tester.pumpAndSettle();
      expect(state(tester), TrafficLightState.windowed);
      expect(padding.left, 12 + trafficLightInset);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

    testWidgets('the room stays 72 window pixels at every scale', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(bar(scale: 200));
      expect((padding.left - 12) * 2, closeTo(trafficLightInset, 1e-9));
      await report(tester, 'fullScreenHidden');
      await tester.pumpAndSettle();
      expect(padding.left, 12);
      await report(tester, 'fullScreenRevealed');
      await tester.pumpAndSettle();
      expect((padding.left - 12) * 2, closeTo(trafficLightInset, 1e-9));
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

    testWidgets('reduced motion jumps instead of gliding', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(bar(reduced: true));
      await report(tester, 'fullScreenHidden');
      await tester.pump();
      expect(padding.left, 12);
      expect(tester.hasRunningAnimations, isFalse);
      await report(tester, 'fullScreenRevealed');
      await tester.pump();
      expect(padding.left, 12 + trafficLightInset);
      expect(tester.hasRunningAnimations, isFalse);
      await report(tester, 'windowed');
      await tester.pump();
      expect(padding.left, 12 + trafficLightInset);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

    testWidgets('a window restored in full screen starts there', (
      WidgetTester tester,
    ) async {
      current = 'fullScreenHidden';
      await tester.pumpWidget(bar());
      // One frame, no glide: the answer stands at once.
      await tester.pump();
      expect(tester.hasRunningAnimations, isFalse);
      expect(asked, contains('getTrafficLights'));
      expect(state(tester), TrafficLightState.fullScreenHidden);
      expect(padding.left, 12);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

    testWidgets('an unknown word changes nothing', (WidgetTester tester) async {
      await tester.pumpWidget(bar());
      await report(tester, 'sideways');
      await report(tester, null);
      await tester.pumpAndSettle();
      expect(state(tester), TrafficLightState.windowed);
      expect(padding.left, 12 + trafficLightInset);
    }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

    testWidgets('Linux ignores the channel', (WidgetTester tester) async {
      await tester.pumpWidget(bar());
      await report(tester, 'fullScreenHidden');
      await tester.pumpAndSettle();
      expect(state(tester), TrafficLightState.windowed);
      expect(padding, const EdgeInsets.symmetric(horizontal: 12));
      expect(asked, isEmpty);
    }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
  });
}
