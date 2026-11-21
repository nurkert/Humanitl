/// Words of the start window that depend on a value (HUM-537).
library;

import '../../core/domain/domain.dart';
import '../../l10n/l10n.dart';

/// How long ago [at] was, seen from [now]: "just now", "5 min ago",
/// "3 h ago", "2 days ago"; "never started" without a start.
String startAge(AppLocalizations l10n, DateTime? at, DateTime now) {
  if (at == null) {
    return l10n.startNeverStarted;
  }
  final Duration ago = now.difference(at);
  if (ago.inMinutes < 1) {
    return l10n.startAgeJustNow;
  }
  if (ago.inHours < 1) {
    return l10n.startAgeMinutes(ago.inMinutes);
  }
  if (ago.inDays < 1) {
    return l10n.startAgeHours(ago.inHours);
  }
  return l10n.startAgeDays(ago.inDays);
}

/// The recent projects: every current row of the register that does not
/// run now, in the daemon's order. A row another folder replaced is
/// history; it stays reachable through the cleanup tab.
List<ProjectEntry> recentProjects(List<ProjectEntry> all) => <ProjectEntry>[
  for (final ProjectEntry entry in all)
    if (!entry.running && !entry.replaced) entry,
];

/// The projects with the running session, in the daemon's order.
List<ProjectEntry> runningProjects(List<ProjectEntry> all) => <ProjectEntry>[
  for (final ProjectEntry entry in all)
    if (entry.running) entry,
];
