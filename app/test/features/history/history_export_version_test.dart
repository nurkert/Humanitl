// Der Creator-Block eines Exports nennt die Version dieses Baus, dieselbe
// wie der Dialog „Über Humanitl“, und keine fest eingetragene Zahl.
//
// Unter `flutter test` ist FLUTTER_BUILD_NAME die Version aus pubspec.yaml.
// Der Release-Workflow setzt sie dort auf die Version des Laufs
// (`packaging/release/stamp-version.sh`) und faehrt diese Datei mit
// `--dart-define=HUMANITL_EXPECTED_VERSION=<version>`; erst dort unterscheidet
// sich die Version von 0.0.0, und eine fest eingetragene Zahl fiele auf.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/about/about_dialog.dart';
import 'package:humanitl/features/history/providers/history_export.dart';

void main() {
  test('the export names the version of this build', () {
    expect(historyExportCreatorVersion, isNotEmpty);
    expect(historyExportCreatorVersion, appVersion);
    const String expected = String.fromEnvironment('HUMANITL_EXPECTED_VERSION');
    if (expected.isNotEmpty) {
      expect(historyExportCreatorVersion, expected);
    }
  });
}
