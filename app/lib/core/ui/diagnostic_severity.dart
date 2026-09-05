/// Wie ein `Severity` auf dem Schirm heißt und welchen Ton er bekommt.
///
/// Zwei Zeilen, die überall gleich aussehen müssen: Jede Karte, die ein
/// `Diagnostic` zeigt — der Setup-Bildschirm, die Sandbox, die Mitteilung des
/// Trays, der Streifen über der Warteschlange — schreibt denselben Schweregrad
/// mit demselben Wort und derselben Farbe. Kopierte Übersetzungstabellen
/// laufen auseinander, sobald eine davon einen Grad ergänzt; deshalb steht sie
/// hier einmal, neben `HDiagnosticCard` und `FixControl`, statt in jedem
/// Aufrufer.
library;

import 'package:flutter/widgets.dart';

import '../domain/domain.dart';
import '../../l10n/l10n.dart';
import 'ui.dart';

/// Das übersetzte Wort für [severity].
String severityLabel(AppLocalizations l10n, Severity severity) =>
    switch (severity) {
      Severity.info => l10n.diagSeverityInfo,
      Severity.warning => l10n.diagSeverityWarning,
      Severity.error => l10n.diagSeverityError,
      Severity.blocking => l10n.diagSeverityBlocking,
    };

/// Der Farbton für [severity].
///
/// Nie das Rot des Blockierens: Rot bedeutet in diesem Programm „diese Anfrage
/// ist nicht hinausgegangen", und ein Befund ist keine Entscheidung.
Color severityColor(HTokens tokens, Severity severity) => switch (severity) {
  Severity.info => tokens.colors.accent,
  Severity.warning => tokens.state.held,
  Severity.error || Severity.blocking => tokens.state.error,
};
