/// Wie ein Befund auf diesem Bildschirm aussieht und heißt.
///
/// Drei Stellen zeigen Diagnosen -- der Streifen über der Liste, das Formular
/// und der Probelauf -- und alle drei zeigen sie gleich. Die beiden Funktionen
/// stehen deshalb hier und nicht in einem der drei Widgets, sonst importierte
/// eines das andere im Kreis.
library;

import 'package:flutter/widgets.dart' show Color;

import '../../core/domain/domain.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';

/// The label of a severity, in the person's language.
String ruleSeverityLabel(AppLocalizations l10n, Severity severity) =>
    switch (severity) {
      Severity.info => l10n.diagSeverityInfo,
      Severity.warning => l10n.diagSeverityWarning,
      Severity.error => l10n.diagSeverityError,
      Severity.blocking => l10n.diagSeverityBlocking,
    };

/// The hue of a severity. Never the blocked red: red means blocked
/// (`docs/UX.md` 3.3, rule 6).
Color ruleSeverityColor(HTokens tokens, Severity severity) =>
    switch (severity) {
      Severity.info => tokens.colors.accent,
      Severity.warning => tokens.state.held,
      Severity.error || Severity.blocking => tokens.state.error,
    };
