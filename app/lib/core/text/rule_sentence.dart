/// Eine Regel als Satz: die Aktion, was sie trifft, wie lange sie hält.
///
/// Der eine Generator für beide Seiten. Die Aktionsleiste zeigt den Satz,
/// bevor eine Regel entsteht, der Regel-Bildschirm zeigt ihn danach; standen
/// dafür zwei Generatoren nebeneinander, lasen sich dieselbe Regel vorher und
/// nachher verschieden, und dann sagt mindestens einer der beiden Sätze etwas
/// über den Verkehr des Nutzers, das nicht stimmt (`backlog/CONVENTIONS.md`
/// 4.13). Deshalb steht der Generator in `core` und nicht in einem Feature:
/// ein Feature importiert kein anderes (ARCHITECTURE 5).
///
/// Nichts hier zeichnet und nichts hier entscheidet. Die Funktionen nehmen
/// eine [Rule] und die Sprache des Nutzers und geben Text; ein Unit-Test prüft
/// damit jede Kombination in beiden Sprachen ohne Widget-Baum.
library;

import '../../l10n/l10n.dart';
import '../domain/domain.dart';

/// Wie grob eine verbleibende Laufzeit geschrieben wird.
///
/// Der genaue Endzeitpunkt reist im Tooltip und im Semantics-Value; die Zeile
/// trägt den Abstand, weil man Regeln danach vergleicht
/// (`backlog/CONVENTIONS.md` 4.13: genau, wo es der Beleg ist, grob, wo es
/// Kontext ist).
enum ExpiryScale {
  /// Weniger als 90 Minuten: in Minuten gezählt.
  minutes,

  /// Weniger als 48 Stunden: in Stunden gezählt.
  hours,

  /// Alles darüber: in Tagen gezählt.
  days,
}

/// Das Wort für [action] in einer Regel.
///
/// Die Regel benennt die Politik, nicht die einzelne Handlung: Englisch behält
/// das Verb des Vertrags (`allow`), Deutsch die Nominalform (`Erlauben`). Das
/// Control, das über eine Anfrage entscheidet, sagt absichtlich etwas anderes
/// (`docs/UX.md` 4.6).
String ruleActionWord(RuleAction action, AppLocalizations l10n) =>
    switch (action) {
      RuleAction.allow => l10n.rulesActionAllow,
      RuleAction.block => l10n.rulesActionBlock,
      RuleAction.ask => l10n.rulesActionAsk,
      RuleAction.redact => l10n.rulesActionRedact,
    };

/// Was [rule] trifft, in einer Zeile: Methoden, Host, Pfad und was der Matcher
/// sonst festnagelt.
///
/// `GET,HEAD · **.npmjs.org · /**`. Ein Teil, den die Regel nicht nennt, fällt
/// weg: eine Regel ohne Pfad trifft jeden Pfad, und eine Zeile, die das zweimal
/// sagte, wäre länger, ohne mehr zu sagen. Die Methoden sind die eine Ausnahme;
/// eine fehlende Methodenliste wird als [AppLocalizations.rulesAnyMethod]
/// geschrieben, weil „welche Verben" die erste Frage an jede Regel ist.
String ruleMatchSummary(Rule rule, AppLocalizations l10n) {
  final RuleMatcher matcher = rule.matcher;
  final List<String> parts = <String>[
    matcher.methods.isEmpty
        ? l10n.rulesAnyMethod
        : matcher.methods.map((Method m) => m.token).join(','),
    matcher.host,
    if (matcher.path.isNotEmpty) matcher.path,
    if (matcher.scheme case final Scheme scheme) scheme.name,
    if (matcher.port != 0) ':${matcher.port}',
    if (matcher.upgrade == Upgrade.websocket) l10n.rulesUpgradeWebsocketShort,
  ];
  return parts.join(' · ');
}

/// Die ganze Regel als ein Satz: Aktion, Treffer und Laufzeit.
///
/// Die Vorschau unter dem Formular liest ihn, die Aktionsleiste vor dem
/// Anlegen, und die Bildschirmleserin auch.
String ruleSentence(
  Rule rule,
  AppLocalizations l10n, {
  required DateTime now,
}) =>
    '${ruleActionWord(rule.action, l10n)} · '
    '${ruleMatchSummary(rule, l10n)} · '
    '${ruleExpiryLabel(rule.expires, l10n, now: now)}';

/// Wie lange die Regel hält, in Worten.
String ruleExpiryLabel(
  RuleExpiry expiry,
  AppLocalizations l10n, {
  required DateTime now,
}) => switch (expiry) {
  RuleExpiryNever() => l10n.rulesExpiryAlways,
  RuleExpirySession() => l10n.rulesExpirySession,
  RuleExpiryAt(:final DateTime at) => _remaining(at, l10n, now),
};

/// Das genaue Ende einer Regel, für Tooltip und Semantics-Value. Leer für eine
/// Regel ohne Ende: eine leere Zeichenkette ist die Abwesenheit einer Tatsache,
/// und das Label darüber sagt bereits, welcher Fall vorliegt.
String ruleExpiryExact(RuleExpiry expiry, AppLocalizations l10n) =>
    switch (expiry) {
      RuleExpiryAt(:final DateTime at) => l10n.rulesExpiresAtExact(
        at.toLocal(),
        at.toLocal(),
      ),
      RuleExpiryNever() || RuleExpirySession() => '',
    };

/// In welcher Einheit [left] geschrieben wird.
ExpiryScale expiryScaleOf(Duration left) {
  if (left.inMinutes < 90) {
    return ExpiryScale.minutes;
  }
  return left.inHours < 48 ? ExpiryScale.hours : ExpiryScale.days;
}

String _remaining(DateTime at, AppLocalizations l10n, DateTime now) {
  final Duration left = at.difference(now);
  if (left <= Duration.zero) {
    return l10n.rulesExpired;
  }
  // Jede Einheit rundet auf, und aus demselben Grund: eine Regel, die noch 55
  // Minuten hält, hält „eine Stunde" und nicht „null"; eine, die noch eine
  // Stunde und 55 Minuten hält, hält zwei und nicht eine. Abrunden ließe die
  // Regel kurzlebiger aussehen, als sie ist, und diese Zahl wird gelesen, um
  // über eine Verlängerung zu entscheiden.
  return switch (expiryScaleOf(left)) {
    ExpiryScale.minutes => l10n.rulesExpiresInMinutes(
      (left.inSeconds / Duration.secondsPerMinute).ceil(),
    ),
    ExpiryScale.hours => l10n.rulesExpiresInHours(
      (left.inMinutes / Duration.minutesPerHour).ceil(),
    ),
    ExpiryScale.days => l10n.rulesExpiresInDays(
      (left.inHours / Duration.hoursPerDay).ceil(),
    ),
  };
}
