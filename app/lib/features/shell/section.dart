/// The sections of the shell (BACKLOG.md 5, Informationsarchitektur).
library;

import '../../l10n/l10n.dart';

/// A rail entry. The order is the order of the rail and of the `Ctrl+<digit>`
/// shortcuts.
///
/// [Section.setup] stands last on purpose. It is the newest entry and the one
/// a person needs least often once the machine is in order, and appending it
/// keeps every digit that was already learned: intercept is `Ctrl+1` today and
/// stays `Ctrl+1` (HUM-044).
enum Section {
  /// Held requests waiting for a decision.
  intercept,

  /// Every recorded flow.
  history,

  /// The ordered rule list.
  rules,

  /// The sandbox and its isolation checks.
  sandbox,

  /// The audit log.
  audit,

  /// The four checks that have to hold before an agent starts, and the button
  /// that starts it (HUM-044).
  setup;

  /// The digit of the `Ctrl+<digit>` shortcut.
  int get shortcutDigit => index + 1;

  /// The label in the current language.
  String label(AppLocalizations l10n) => switch (this) {
    Section.intercept => l10n.shellNavIntercept,
    Section.history => l10n.shellNavHistory,
    Section.rules => l10n.shellNavRules,
    Section.sandbox => l10n.shellNavSandbox,
    Section.audit => l10n.shellNavAudit,
    Section.setup => l10n.shellNavSetup,
  };
}
