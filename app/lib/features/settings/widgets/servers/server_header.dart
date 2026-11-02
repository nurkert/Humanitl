/// The head of a saved model server's detail view: "All servers" on a
/// narrow page, the name with the default mark, and who uses the server
/// (HUM-405, HUM-414).
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../setting_row.dart';

/// The head over the form of [server], or of a new server.
class ServerHeader extends StatelessWidget {
  /// Creates the head.
  const ServerHeader({
    required this.server,
    required this.list,
    required this.removedElsewhere,
    this.onBack,
    super.key,
  });

  /// The server shown; null for a new one.
  final SavedServer? server;

  /// Every saved server with the profiles and the project that use them.
  final SavedServers list;

  /// True for a draft of a server removed elsewhere; nobody uses it now.
  final bool removedElsewhere;

  /// Back to the list, on a narrow page.
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer? server = this.server;
    final VoidCallback? back = onBack;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: SettingRowGrid.inset),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (back != null) ...<Widget>[
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: HButton(
                key: const Key('settings-server-back'),
                variant: HButtonVariant.ghost,
                onPressed: back,
                leading: RotatedBox(
                  quarterTurns: 2,
                  child: HGlyphIcon(
                    HGlyph.chevronRight,
                    size: 12,
                    color: tokens.colors.fg1,
                  ),
                ),
                child: Text(l10n.settingsServerBack),
              ),
            ),
            SizedBox(height: tokens.spacing.x2),
          ],
          Row(
            children: <Widget>[
              Flexible(
                child: Semantics(
                  header: true,
                  child: Text(
                    server?.name ?? l10n.settingsServerNewTitle,
                    key: const Key('settings-server-title'),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.ui16.semibold.tinted(
                      tokens.colors.fg0,
                    ),
                  ),
                ),
              ),
              if (server != null && server.active) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                HGlyphIcon(HGlyph.star, size: 12, color: tokens.colors.accent),
                SizedBox(width: tokens.spacing.x1),
                Text(
                  l10n.settingsServerDefault,
                  key: const Key('settings-server-detail-default'),
                  style: tokens.typography.ui12.medium.tinted(
                    tokens.colors.accentText,
                  ),
                ),
              ],
            ],
          ),
          if (server != null && !removedElsewhere) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              _usedBy(l10n, server),
              key: const Key('settings-server-used-by'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ],
        ],
      ),
    );
  }

  /// "Used by: …": the default, the profiles and the project that pick
  /// [server], from what the daemon listed.
  String _usedBy(AppLocalizations l10n, SavedServer server) {
    final List<String> users = <String>[
      if (server.active) l10n.settingsServerUsedByDefault,
      ...serverUsers(l10n, list, server),
    ];
    return users.isEmpty
        ? l10n.settingsServerUsedByNobody
        : l10n.settingsServerUsedBy(users.join(', '));
  }
}

/// The profiles and the project of [list] that pick [server], in words.
List<String> serverUsers(
  AppLocalizations l10n,
  SavedServers list,
  SavedServer server,
) {
  final ProjectUse? project = list.project;
  return <String>[
    for (final ProfileUse profile in list.profiles)
      if (profile.server == server.name)
        l10n.settingsServerUsedByProfile(profile.name),
    if (project != null && project.chosen == server.name)
      l10n.settingsServerUsedByProject(_lastSegment(project.dir)),
  ];
}

/// The last part of a path, or the path itself.
String _lastSegment(String path) {
  final List<String> parts = path
      .split('/')
      .where((String part) => part.isNotEmpty)
      .toList();
  return parts.isEmpty ? path : parts.last;
}
