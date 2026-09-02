import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/main.dart';

void main() {
  testWidgets('app boots', (tester) async {
    await tester.pumpWidget(const HumanitlApp());
    expect(find.byType(HumanitlApp), findsOneWidget);
  });
}
