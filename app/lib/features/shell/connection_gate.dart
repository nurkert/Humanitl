/// Chooses between splash, setup screen and shell from
/// `connectionStateProvider` (HUM-019 Widget-Baum).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../setup/setup_screen.dart';
import 'providers/connection.dart';
import 'shell_screen.dart';
import 'widgets/splash.dart';

/// The gate.
class ConnectionGate extends ConsumerWidget {
  /// Creates the gate.
  const ConnectionGate({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final ConnectionStatus status = ref.watch(connectionStateProvider);
    return switch (status) {
      ConnectionConnecting() => const Splash(),
      ConnectionFailed(:final diagnostic) => SetupScreen(
        diagnostic: diagnostic,
        onRetry: ref.read(connectionStateProvider.notifier).retry,
      ),
      ConnectionConnected(:final info) => ShellScreen(info: info),
    };
  }
}
