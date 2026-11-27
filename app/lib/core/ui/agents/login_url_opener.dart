/// How "Open in browser" of a login run opens a URL (HUM-521).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../file_opener.dart';

/// Opens a login URL on this machine; true when the desktop took it.
typedef LoginUrlOpener = Future<bool> Function(String url);

/// The opener of "Open in browser". Overridden in tests: a widget test must
/// never start a program on the person's desktop.
final Provider<LoginUrlOpener> loginUrlOpenerProvider =
    Provider<LoginUrlOpener>((Ref ref) => openWithDesktop);
