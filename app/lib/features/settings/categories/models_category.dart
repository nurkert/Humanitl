/// The category Models: the saved servers as cards, the detail view of one
/// of them, and who uses which server (HUM-405, in the overlay of HUM-404).
///
/// The page has two states. Without a server open it shows the cards, with
/// the card that adds one at the end, and under them the table "Who uses
/// which server". A click on a card replaces both with the detail view of
/// that server, and "All servers" goes back. Nothing here writes on its own;
/// every change is one RPC of `LlmServers` or `LlmKey` (ADR-018).
///
/// A jump of a `ChangeSetting` fix or of the search to the endpoint, the
/// model or the key opens the detail view of the default server, or of a new
/// one while none is saved, and then lands on the row (HUM-275).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../registry/setting_entry.dart';
import '../settings_scope.dart';
import '../widgets/servers/server_cards.dart';
import '../widgets/servers/server_detail.dart';
import '../widgets/servers/server_usage.dart';
import '../widgets/setting_row.dart';

/// Draws the whole category.
Widget buildModelsCategory(SettingsPageData data) => const ModelsCategory();

/// The settings that stand in the detail view of a server.
const Set<String> serverFormIds = <String>{
  FormKeys.llmEndpoint,
  FormKeys.llmModels,
  FormKeys.llmApiKeyRef,
};

/// The cards, the detail view and the table of assignments.
class ModelsCategory extends ConsumerStatefulWidget {
  /// Creates the category.
  const ModelsCategory({super.key});

  @override
  ConsumerState<ModelsCategory> createState() => _ModelsCategoryState();
}

/// Which detail view is open.
sealed class _Open {
  const _Open();
}

final class _Cards extends _Open {
  const _Cards();
}

final class _Server extends _Open {
  const _Server(this.name);
  final String name;
}

final class _Adding extends _Open {
  const _Adding();
}

class _ModelsCategoryState extends ConsumerState<ModelsCategory> {
  _Open _open = const _Cards();

  /// The jump that opened a detail view and still has to land.
  String? _landing;

  void _show(_Open open) => setState(() => _open = open);

  /// Opens the detail view a jump to [id] needs, once the list is known.
  void _follow(String id, SavedServers list) {
    if (_landing == id) {
      return;
    }
    _landing = id;
    final SavedServer? fallback = list.activeServer;
    final _Open target = fallback == null
        ? const _Adding()
        : _Server(fallback.name);
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (!mounted) {
        return;
      }
      setState(() => _open = target);
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        if (!mounted) {
          return;
        }
        SettingsScope.maybeOf(context)?.reveal(id);
        _landing = null;
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AsyncValue<SavedServers> servers = ref.watch(llmServersProvider);
    final SavedServers list = servers.value ?? SavedServers.none;
    final SettingsController? scope = SettingsScope.maybeOf(context);
    final String? wanted = scope?.wanted;
    final bool known = servers.hasValue || servers.hasError;
    if (wanted != null && serverFormIds.contains(wanted) && known) {
      _follow(wanted, list);
    }

    final _Open open = _open;
    final Widget body = switch (open) {
      _Server(:final String name) when list.named(name) != null => ServerDetail(
        key: ValueKey<String>('detail-$name'),
        server: list.named(name),
        list: list,
        onClose: () => _show(const _Cards()),
        onRenamed: (String newName) => _show(_Server(newName)),
      ),
      _Adding() => ServerDetail(
        key: const ValueKey<String>('detail-+'),
        list: list,
        onClose: () => _show(const _Cards()),
      ),
      _ => _overview(context, list),
    };
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x4),
      child: Semantics(
        container: true,
        label: l10n.settingsServersLabel,
        child: body,
      ),
    );
  }

  Widget _overview(BuildContext context, SavedServers list) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SettingsController? scope = SettingsScope.maybeOf(context);
    Widget anchored(String id, Widget child) => scope == null
        ? child
        : KeyedSubtree(
            key: scope.rowKey(id),
            child: Focus(focusNode: scope.anchor(id), child: child),
          );
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: SettingRowGrid.inset),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          anchored(
            SettingIds.llmServers,
            Column(
              key: const ValueKey<String>(
                'setting-row-${SettingIds.llmServers}',
              ),
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(
                  l10n.settingsServersIntro,
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
                SizedBox(height: tokens.spacing.x3),
                ServerCards(
                  servers: list,
                  onOpen: (String name) => _show(_Server(name)),
                  onAdd: () => _show(const _Adding()),
                ),
              ],
            ),
          ),
          if (list.servers.isNotEmpty) ...<Widget>[
            SizedBox(height: tokens.spacing.x6),
            anchored(
              SettingIds.llmServerUsage,
              KeyedSubtree(
                key: const ValueKey<String>(
                  'setting-row-${SettingIds.llmServerUsage}',
                ),
                child: ServerUsage(servers: list),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
