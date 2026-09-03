/// The five sections of the shell (BACKLOG.md 5, Informationsarchitektur).
library;

import '../../l10n/l10n.dart';

/// A rail entry. The order is the order of the rail and of `Ctrl+1..5`.
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
  audit;

  /// The digit of the `Ctrl+<digit>` shortcut.
  int get shortcutDigit => index + 1;

  /// The label in the current language.
  String label(AppLocalizations l10n) => switch (this) {
    Section.intercept => l10n.shellNavIntercept,
    Section.history => l10n.shellNavHistory,
    Section.rules => l10n.shellNavRules,
    Section.sandbox => l10n.shellNavSandbox,
    Section.audit => l10n.shellNavAudit,
  };
}
