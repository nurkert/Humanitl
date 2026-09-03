/// Placeholder for the History section; the real screen arrives with a later
/// issue (HUM-019 Nicht-Ziel).
library;

import 'package:flutter/widgets.dart';

import '../../core/ui/section_placeholder.dart';
import '../../l10n/l10n.dart';

/// The History section.
class HistoryScreen extends StatelessWidget {
  /// Creates the section.
  const HistoryScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SectionPlaceholder(
      title: l10n.shellNavHistory,
      hint: l10n.shellSectionPlaceholder,
    );
  }
}
