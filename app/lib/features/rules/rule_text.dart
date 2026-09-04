/// Warum ein Muster im Formular nicht gelesen werden kann.
///
/// Der Satz, den eine Regel ergibt, steht in `core/text/rule_sentence.dart`:
/// die Aktionsleiste zeigt ihn vor dem Anlegen, dieser Bildschirm danach, und
/// ein zweiter Generator hier hätte genau die beiden Sätze auseinanderlaufen
/// lassen. Hier bleibt, was nur ein Formular braucht: die Vorprüfung, die
/// sagt, was an einer Eingabe nicht stimmt, bevor die Rundreise läuft. Ob ein
/// Muster wirklich legal ist, bleibt die Antwort des Daemons (ADR-018,
/// `docs/UX.md` 4.4).
library;

import '../../core/domain/domain.dart';
import '../../l10n/l10n.dart';

/// Warum das Host-Muster im Formular nicht gelesen werden kann, in der Sprache
/// des Nutzers.
///
/// Null, solange das Muster entweder in Ordnung ist oder etwas, das nur die
/// Engine beurteilen kann. Der Daemon behält in beiden Fällen das letzte Wort:
/// dieser Text erscheint, während jemand tippt, sein eigener Befund, wenn
/// gespeichert wird (`docs/UX.md` 4.4).
String? hostProblemText(String pattern, AppLocalizations l10n) =>
    switch (hostPatternProblem(pattern)) {
      null => null,
      HostPatternProblem.empty => l10n.rulesHostEmpty,
      HostPatternProblem.wildcardInLabel => l10n.rulesHostWildcard,
      HostPatternProblem.emptyLabel => l10n.rulesHostEmptyLabel,
      HostPatternProblem.notAnAddress => l10n.rulesHostAddress,
      HostPatternProblem.notALabel => l10n.rulesHostLabel,
    };

/// Warum das Pfad-Muster im Formular nicht gelesen werden kann, oder null.
String? pathProblemText(String pattern, AppLocalizations l10n) =>
    pathPatternProblem(pattern) == null ? null : l10n.rulesPathRegex;

/// Wahr, solange der Entwurf es wert ist, gesendet zu werden: die Teile, die
/// die App beurteilen kann, sind in Ordnung. Alles Weitere ist Sache des
/// Daemons.
bool rulePassesPreCheck(Rule rule) =>
    hostPatternProblem(rule.matcher.host) == null &&
    pathPatternProblem(rule.matcher.path) == null;
