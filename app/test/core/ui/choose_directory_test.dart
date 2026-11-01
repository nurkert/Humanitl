// HUM-379: on macOS the folder dialog comes from the Runner, which allows new
// folders.

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('macOS asks the Runner and passes the title on', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
    MethodCall? seen;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(folderChannel, (MethodCall call) async {
          seen = call;
          return '/Users/me/new';
        });
    final String? chosen = await chooseDirectory(dialogTitle: 'Where');
    debugDefaultTargetPlatformOverride = null;
    expect(chosen, '/Users/me/new');
    expect(seen?.method, 'chooseFolder');
    expect((seen?.arguments as Map<Object?, Object?>)['title'], 'Where');
  });
}
