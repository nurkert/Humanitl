/// Wie ein Befund auf diesem Bildschirm aussieht und heißt.
///
/// Drei Stellen zeigen Diagnosen -- der Streifen über der Liste, das Formular
/// und der Probelauf -- und alle drei zeigen sie gleich. Die beiden Funktionen
/// stehen deshalb hier und nicht in einem der drei Widgets, sonst importierte
/// eines das andere im Kreis.
library;

import 'package:flutter/widgets.dart' show Color;

import '../../core/domain/domain.dart';
import '../../core/ui/diagnostic_severity.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';

/// Das Wort für [severity]; ein Alias auf die eine Abbildung in
/// `core/ui/diagnostic_severity.dart` (HUM-068).
String ruleSeverityLabel(AppLocalizations l10n, Severity severity) =>
    severityLabel(l10n, severity);

/// Der Farbton für [severity]; derselbe Alias wie [ruleSeverityLabel].
Color ruleSeverityColor(HTokens tokens, Severity severity) =>
    severityColor(tokens, severity);
