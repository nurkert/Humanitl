// HUM-378: the header bar reports its height, so the traffic lights stay
// centred on it at every scale.

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/traffic_lights.dart';
import 'package:humanitl/core/ui/ui_scale.dart';

void main() {
  final List<Object?> sent = <Object?>[];

  setUp(() {
    sent.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(windowChannel, (MethodCall call) async {
          sent.add((call.arguments as Map<Object?, Object?>)['height']);
          return null;
        });
  });

  Widget at(int scale) => UiScaleScope(
    preferred: scale,
    limit: 300,
    effective: scale,
    child: const TrafficLightSync(child: SizedBox()),
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
    debugDefaultTargetPlatformOverride = null;
  });
}
