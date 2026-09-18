// Die zweite Hälfte der Antwort auf „warum ist das Fenster fort" (HUM-136).
//
// Die Dart-Seite schreibt Start und Ausnahmen in
// `$XDG_STATE_HOME/humanitl/app.log`. Was sie nicht schreiben kann, steht
// hier: ein Ende durch ein Signal und ein Absturz im nativen Teil. Beide
// treffen einen Prozess, in dem `runZonedGuarded` nichts mehr fängt, und
// genau diese beiden Fälle waren am 2026-09-07 nicht zu unterscheiden.
//
// Alles in dieser Datei, was ein Signal-Handler anfasst, ist
// async-signal-safe: nur `clock_gettime`, `stat`, `rename`, `open`, `write`,
// `close`, `raise`, `pthread_sigmask` und Arithmetik auf dem Stapel.
// Keine Allokation, kein `snprintf`, kein `localtime`. Ein Absturzprotokoll,
// das seinerseits im Handler abstürzt, ist kein Protokoll.
//
// # Was das Protokoll nicht abdeckt, und warum es trotzdem so gebaut ist
//
// **Ein übergelaufener Stapel auf einem anderen Faden als dem Haupt-Faden
// hinterlässt keine Zeile.** `sigaltstack` gilt je Faden, und
// `humanitl_exit_log_init` läuft vor der Engine, also auf genau einem Faden.
// Die Fäden, die Flutter danach selbst startet -- Raster, IO, Dart-VM --
// bekommen keinen Ersatzstapel; der Runner kann sie nicht erreichen, weil er
// sie nicht erzeugt. Für sie gilt: Jedes Signal wird notiert, außer einem
// `SIGSEGV` aus einem übergelaufenen Stapel, denn dafür fehlt der Platz, auf
// dem der Handler liefe. Ein gewöhnlicher `SIGSEGV`, ein `SIGABRT` aus GLib
// und jedes Signal von außen werden auf jedem Faden geschrieben -- der
// Handler gehört dem Prozess, nicht dem Faden.
//
// **Der Handler läuft, wenn die Engine ihn nicht überschreibt.** Was nach
// `humanitl_exit_log_init` eine eigene Vorbelegung setzt, gewinnt. Das ist
// richtig so: Wer ein Signal wirklich behandelt, weiß mehr darüber als ein
// Protokoll.

#ifndef RUNNER_EXIT_LOG_H_
#define RUNNER_EXIT_LOG_H_

#include <stddef.h>

// Ermittelt den Pfad des Protokolls, legt sein Verzeichnis an und hängt die
// Signal-Handler ein. Wird einmal aufgerufen, vor `g_application_run`.
void humanitl_exit_log_init(void);

// Schreibt die Zeile des geordneten Endes, nachdem die GTK-Schleife
// zurückgekehrt ist.
//
// Diese Zeile schreibt der Runner und nicht Dart: Sie ist das Letzte, was in
// einem geordneten Ende überhaupt noch läuft.
void humanitl_exit_log_stop(int status);

// Schreibt eine Zeile mit höchstens einem Feld. `key` und `value` dürfen
// `nullptr` sein.
void humanitl_exit_log_note(const char* kind, const char* key,
                            const char* value);

// Der Pfad des Protokolls, oder ein leerer String vor `init`.
const char* humanitl_exit_log_path(void);

// Baut eine Zeile in `out` und liefert ihre Länge ohne den Zeilenumbruch.
//
// Steht im Header, weil der Test des Formats sonst den Prozess beenden
// müsste, um eine Zeile zu sehen (`exit_log_test.cc`).
size_t humanitl_exit_log_format(char* out, size_t out_len, long long seconds,
                                long nanoseconds, long pid, const char* kind,
                                const char* key, const char* value);

#endif  // RUNNER_EXIT_LOG_H_
