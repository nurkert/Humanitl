/// Ein Befund des Daemons, so bereinigt, wie eine Ansicht ihn zeigen darf.
///
/// Stand bis HUM-269 in `features/intercept/providers/diagnostics.dart`. Die
/// Störung der Aufzeichnung (HUM-250) hält ihren Befund bereinigt, und seit
/// auch der Editor an ihrer Pause hält, lebt sie im Kern; ein Kern, der ein
/// Feature importiert, kehrte die Abhängigkeit um (`docs/ARCHITECTURE.md` 5).
library;

import '../body/body_span.dart';
import '../domain/domain.dart';

/// [diagnostic], so wie eine Ansicht ihn zeigen darf.
///
/// Der Satz des Daemons und der Vorschlag darin tragen Material von außen: den
/// Hostnamen, mit dem der Handschlag scheiterte, den Fehlertext der
/// Zertifikatsprüfung, den Pfad einer Umgebungsvariablen. Steuerzeichen und
/// Richtungsumkehrungen darin würden nicht nur die Karte verschieben, sondern
/// auch die Zeile, die jemand von ihr in eine Shell kopiert. Ersetzt wird
/// jedes Zeichen einzeln, gelöscht wird keines.
///
/// `FixAction.addRule` bleibt unverändert: Aus ihr zeichnet die Oberfläche
/// heute nur ein festes Wort und keinen Text des Absenders.
Diagnostic sanitizeDiagnostic(Diagnostic diagnostic) {
  final String? docsUrl = diagnostic.docsUrl;
  return diagnostic.copyWith(
    code: sanitizeBodyText(diagnostic.code),
    title: sanitizeBodyText(diagnostic.title),
    why: sanitizeBodyText(diagnostic.why),
    docsUrl: docsUrl == null ? null : sanitizeBodyText(docsUrl),
    fix: switch (diagnostic.fix) {
      null => null,
      FixActionSetEnv(:final String key, :final String value) =>
        FixAction.setEnv(
          key: sanitizeBodyText(key),
          value: sanitizeBodyText(value),
        ),
      FixActionChangeSetting(:final String key, :final String value) =>
        FixAction.changeSetting(
          key: sanitizeBodyText(key),
          value: sanitizeBodyText(value),
        ),
      FixActionCopyCommand(:final String command) => FixAction.copyCommand(
        command: sanitizeBodyText(command),
      ),
      FixActionOpenUrl(:final String url) => FixAction.openUrl(
        url: sanitizeBodyText(url),
      ),
      FixActionRemountReadOnly(:final String path) => FixAction.remountReadOnly(
        path: sanitizeBodyText(path),
      ),
      // Eine vorgeschlagene Regel trägt Hostmuster, Pfad und Notiz, und alle
      // drei stammen aus derselben Leitung wie der Satz darüber. Sie hier
      // auszulassen risse ein Loch in genau die Grenze, die diese Funktion
      // ist.
      FixActionAddRule(:final Rule rule) => FixAction.addRule(
        rule: _sanitizeRule(rule),
      ),
      // Trägt keinen Text des Absenders.
      FixActionInstallService() => diagnostic.fix,
    },
  );
}

/// [rule], so wie eine Ansicht sie zeigen darf.
///
/// Nur die Felder, die Text tragen. Zahlen, Zeitpunkte und Aufzählungen können
/// keine Richtung drehen und kein Zeichen verstecken.
Rule _sanitizeRule(Rule rule) => rule.copyWith(
  matcher: rule.matcher.copyWith(
    host: sanitizeBodyText(rule.matcher.host),
    path: sanitizeBodyText(rule.matcher.path),
  ),
  note: rule.note == null ? null : sanitizeBodyText(rule.note ?? ''),
);
