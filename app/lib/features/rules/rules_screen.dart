/// Placeholder for the Rules section; the real screen arrives with a later
/// issue (HUM-019 Nicht-Ziel).
library;

import 'package:flutter/widgets.dart';

import '../../core/ui/section_placeholder.dart';
import '../../l10n/l10n.dart';

/// The Rules section.
class RulesScreen extends StatelessWidget {
  /// Creates the section.
  const RulesScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SectionPlaceholder(
      title: l10n.shellNavRules,
      hint: l10n.shellSectionPlaceholder,
    );
  }
}
