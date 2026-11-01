/// Was der Daemon zuletzt über Updates gesagt hat (HUM-382), geteilt von der
/// Statusleiste und den Einstellungen.
library;

import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import 'client_providers.dart';
import 'daemon_client.dart';

/// The AppImage this app runs from (`$APPIMAGE`), empty otherwise. The
/// daemon checks the path itself before it replaces anything. Overridden in
/// tests.
final Provider<String> updateAppImageProvider = Provider<String>(
  (Ref ref) => Platform.environment['APPIMAGE'] ?? '',
);

/// The last update status the daemon reported, or null before the first.
final NotifierProvider<UpdateStatusNotifier, UpdateStatus?>
updateStatusProvider = NotifierProvider<UpdateStatusNotifier, UpdateStatus?>(
  UpdateStatusNotifier.new,
);

/// The notifier behind [updateStatusProvider].
class UpdateStatusNotifier extends Notifier<UpdateStatus?> {
  @override
  UpdateStatus? build() => null;

  /// Asks the daemon; [force] makes it ask GitHub now. Throws the
  /// [DaemonException] of a failed check and keeps the last status then.
  Future<UpdateStatus> check({bool force = false}) async {
    final UpdateStatus status = await ref
        .read(daemonClientProvider)
        .checkUpdate(force: force, appimage: ref.read(updateAppImageProvider));
    state = status;
    return status;
  }

  /// [check] without force, silent on failure: the automatic question must
  /// never put an error in front of anybody.
  Future<void> checkQuietly() async {
    try {
      await check();
    } on DaemonException {
      // Nothing to show; the next question comes in an hour.
    }
  }
}
