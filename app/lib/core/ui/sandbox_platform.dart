/// Which words fit the sandbox of this system (HUM-376).
library;

import 'package:flutter/foundation.dart';

/// Whether the sandbox of this system runs in real paths, without `/work`.
///
/// For texts that have no status at hand; `SandboxStatus.usesPolicy` is the
/// answer where there is one. The app and its daemon share a machine, and
/// Seatbelt is the backend of macOS.
bool get sandboxUsesRealPaths => defaultTargetPlatform == TargetPlatform.macOS;
