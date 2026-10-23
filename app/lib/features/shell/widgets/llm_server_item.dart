/// The LLM item of the status bar: the active saved model server, and a
/// quick switch between the saved ones (HUM-289).
///
/// It draws nothing while there is nothing to switch between (fewer than two
/// saved servers): the settings show the one server there is, and a status
/// bar item that never offers a choice would be read as one. A switch writes
/// the configuration through `LlmServers(activate)`; it applies to the next
/// session, a running session keeps its server, and the label of the item
/// for a screen reader says so.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// The status bar item with the active server and its quick switch.
class LlmServerItem extends ConsumerStatefulWidget {
  /// Creates the item.
  const LlmServerItem({super.key});

  @override
  ConsumerState<LlmServerItem> createState() => _LlmServerItemState();
}

class _LlmServerItemState extends ConsumerState<LlmServerItem> {
  final HContextMenuController _menu = HContextMenuController();
  final GlobalKey _anchor = GlobalKey();

  /// Opens the switch just above the item.
  void _open() {
    final RenderObject? box = _anchor.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.hasSize) {
      return;
    }
    _menu.open(box.localToGlobal(Offset.zero));
  }

  Future<void> _activate(String name) async {
    try {
      await ref.read(llmServersProvider.notifier).activate(name);
    } on DaemonException {
      // The settings show why; the item keeps showing the active server
      // the daemon reports, which did not change.
    }
  }

  @override
  Widget build(BuildContext context) {
    final SavedServers list =
        ref.watch(llmServersProvider).value ?? SavedServers.none;
    if (!list.canSwitch) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String active = list.active;
    return Padding(
      padding: EdgeInsets.only(right: tokens.spacing.x3),
      child: HContextMenu(
        controller: _menu,
        semanticsLabel: l10n.shellStatusLlmServerSwitch,
        itemsBuilder: () => <HMenuItem>[
          for (final SavedServer server in list.servers)
            HMenuItem(
              label: server.active
                  ? l10n.shellStatusLlmServerActive(server.name)
                  : server.name,
              enabled: !server.active,
              onSelected: () => unawaited(_activate(server.name)),
            ),
        ],
        child: HBadge(
          key: _anchor,
          text: active.isEmpty
              ? l10n.shellStatusLlmServer('-')
              : l10n.shellStatusLlmServer(active),
          color: tokens.state.passthroughLlm,
          semanticsLabel: l10n.shellStatusLlmServerSwitch,
          onTap: _open,
        ),
      ),
    );
  }
}
