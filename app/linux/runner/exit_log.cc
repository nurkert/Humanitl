#include "exit_log.h"

#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

namespace {

// So groß wie ein Pfad unter Linux werden darf.
const size_t kPathCapacity = 4096;

// Muss `AppLog.maxBytes` in `lib/core/diagnostics/app_log.dart` entsprechen:
// Zwei Schreiber, eine Obergrenze, sonst ist die Aussage „nie mehr als 512
// KiB" nur halb wahr.
const long kMaxBytes = 256L * 1024L;

// Eine Zeile aus diesem Modul trägt nie mehr als einen Signalnamen oder eine
// Zahl; die Dart-Seite kürzt längere Zeilen selbst.
const size_t kLineCapacity = 256;

char g_path[kPathCapacity];
char g_dir[kPathCapacity];
char g_rotated_path[kPathCapacity + 8];

// Die drei Marken liest und schreibt ein Signal-Handler. `sig_atomic_t` ist
// der einzige Typ, für den der Standard das erlaubt; ein `bool` wäre hier
// eine Zusicherung, die niemand gibt.
volatile sig_atomic_t g_ready = 0;

// Ob die Zeile des Endes schon steht. Es gibt zwei Wege dorthin, und genau
// einer davon darf schreiben.
volatile sig_atomic_t g_stop_written = 0;

// Ob gerade jemand rotiert.
volatile sig_atomic_t g_rotating = 0;

// Der Ersatzstapel für den Fall, dass der Stapel selbst übergelaufen ist.
// Ohne ihn liefe der Handler für so einen `SIGSEGV` nie. Er gilt nur für den
// Faden, auf dem `humanitl_exit_log_init` läuft; die Grenze steht in
// `exit_log.h`.
char* g_alternate_stack = nullptr;

// Hängt `text` an und macht jedes Steuerzeichen zu einem Leerzeichen, damit
// aus einem Eintrag nie zwei Zeilen werden. Liefert die neue Länge.
size_t append_text(char* out, size_t out_len, size_t at, const char* text) {
  if (text == nullptr) {
    return at;
  }
  for (size_t i = 0; text[i] != '\0'; i++) {
    if (at + 1 >= out_len) {
      break;
    }
    char c = text[i];
    if (c == '\n' || c == '\r' || (c >= 0 && c < 0x20) || c == 0x7f) {
      c = ' ';
    }
    out[at++] = c;
  }
  out[at] = '\0';
  return at;
}

// Hängt `value` dezimal an, mindestens `width` Stellen, links mit Nullen.
size_t append_uint(char* out, size_t out_len, size_t at,
                   unsigned long long value, size_t width) {
  char digits[24];
  size_t n = 0;
  do {
    digits[n++] = static_cast<char>('0' + (value % 10));
    value /= 10;
  } while (value != 0 && n < sizeof(digits));
  while (n < width && n < sizeof(digits)) {
    digits[n++] = '0';
  }
  while (n > 0) {
    if (at + 1 >= out_len) {
      break;
    }
    out[at++] = digits[--n];
  }
  out[at] = '\0';
  return at;
}

// Kalendertag aus der Zahl der Tage seit 1970-01-01, ohne `gmtime`: Das ist
// keine Sparsamkeit, sondern die Bedingung dafür, dass die Zeile auch aus
// einem Signal-Handler heraus entstehen darf. Der Algorithmus ist der
// bekannte `civil_from_days` (Howard Hinnant), reine Ganzzahlarithmetik.
void civil_from_days(long long days, long long* year, unsigned* month,
                     unsigned* day) {
  days += 719468;
  const long long era = (days >= 0 ? days : days - 146096) / 146097;
  const unsigned long long day_of_era =
      static_cast<unsigned long long>(days - era * 146097);
  const unsigned long long year_of_era =
      (day_of_era - day_of_era / 1460 + day_of_era / 36524 -
       day_of_era / 146096) /
      365;
  const long long y = static_cast<long long>(year_of_era) + era * 400;
  const unsigned long long day_of_year =
      day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  const unsigned long long shifted_month = (5 * day_of_year + 2) / 153;
  const unsigned d =
      static_cast<unsigned>(day_of_year - (153 * shifted_month + 2) / 5 + 1);
  const unsigned m = static_cast<unsigned>(
      shifted_month < 10 ? shifted_month + 3 : shifted_month - 9);
  *year = y + (m <= 2 ? 1 : 0);
  *month = m;
  *day = d;
}

bool write_all(int fd, const char* data, size_t len) {
  size_t written = 0;
  while (written < len) {
    const ssize_t n = write(fd, data + written, len - written);
    if (n < 0) {
      if (errno == EINTR) {
        continue;
      }
      return false;
    }
    if (n == 0) {
      return false;
    }
    written += static_cast<size_t>(n);
  }
  return true;
}

// Hängt eine fertige Zeile an, und rotiert vorher, wenn sie die Obergrenze
// reißen würde. Beides ohne stdio, damit ein Signal-Handler denselben Weg
// nehmen kann wie ein geordnetes Ende.
// `line` trägt den Zeilenumbruch schon, und geschrieben wird er in **einem**
// `write`: Ein Anhängen unter `PIPE_BUF` ist atomar, zwei Aufrufe wären es
// nicht. Der Prozess hat mehrere Fäden -- Raster, IO, Dart-VM --, und zwei
// Abstürze zugleich schrieben sonst `zeile1zeile2\n\n`.
void emit(const char* line, size_t len) {
  if (!g_ready) {
    return;
  }
  struct stat info;
  if (stat(g_path, &info) == 0 &&
      info.st_size + static_cast<off_t>(len) > kMaxBytes) {
    // Nur einer rotiert. Ohne diese Marke benennte der zweite Faden gleich
    // hinterher eine Datei um, die nur noch seine eigene Zeile trägt.
    //
    // `rename` allein, ohne `unlink` davor: Es ersetzt das Ziel in einem
    // Schritt. Ein Löschen davor öffnete ein Fenster, in dem es keine
    // Rotation gibt -- und die Dart-Seite rotiert dieselbe Datei, ohne von
    // dieser Marke zu wissen.
    if (__sync_lock_test_and_set(&g_rotating, 1) == 0) {
      rename(g_path, g_rotated_path);
      __sync_lock_release(&g_rotating);
    }
  }
  const int fd = open(g_path, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0600);
  if (fd < 0) {
    return;
  }
  write_all(fd, line, len);
  close(fd);
}

// Hängt den Zeilenumbruch in denselben Puffer und schreibt einmal.
void emit_line(char* line, size_t len, size_t capacity) {
  if (capacity < 2) {
    return;
  }
  if (len + 1 >= capacity) {
    len = capacity - 2;
  }
  line[len] = '\n';
  emit(line, len + 1);
}

const char* signal_name(int signum) {
  switch (signum) {
    case SIGTERM:
      return "SIGTERM";
    case SIGHUP:
      return "SIGHUP";
    case SIGINT:
      return "SIGINT";
    case SIGQUIT:
      return "SIGQUIT";
    case SIGABRT:
      return "SIGABRT";
    case SIGSEGV:
      return "SIGSEGV";
    case SIGBUS:
      return "SIGBUS";
    case SIGFPE:
      return "SIGFPE";
    case SIGILL:
      return "SIGILL";
    default:
      return "SIGNAL";
  }
}

// Schreibt die Zeile und lässt den Prozess danach so sterben, wie er ohne
// diesen Handler gestorben wäre: `SA_RESETHAND` hat die Vorbelegung schon
// zurückgesetzt, `raise` liefert das Signal erneut. Damit bleibt der
// Speicherauszug erhalten und `pkill` verhält sich wie zuvor -- ein Handler,
// der `SIGTERM` verschluckte, machte aus einem Protokoll einen Fehler.
void handle_signal(int signum) {
  const int saved_errno = errno;
  struct timespec now;
  if (clock_gettime(CLOCK_REALTIME, &now) != 0) {
    now.tv_sec = 0;
    now.tv_nsec = 0;
  }
  char line[kLineCapacity];
  const size_t len = humanitl_exit_log_format(
      line, sizeof(line), static_cast<long long>(now.tv_sec),
      static_cast<long>(now.tv_nsec), static_cast<long>(getpid()), "signal",
      "name", signal_name(signum));
  emit_line(line, len, sizeof(line));
  errno = saved_errno;
  raise(signum);
  // `raise` allein genügt nicht: Während der Handler läuft, ist das Signal
  // blockiert (`sa_mask`), es bliebe also hängend und käme erst nach der
  // Rückkehr an -- die es hier nicht gibt. Das Freigeben stellt es sofort
  // zu, und weil `SA_RESETHAND` die Vorbelegung zurückgesetzt hat, endet der
  // Prozess genau so, wie er ohne diesen Handler geendet hätte.
  //
  // `pthread_sigmask` und nicht `sigprocmask`: In einem Prozess mit mehreren
  // Fäden -- und der Runner hat welche, sobald die Engine läuft -- ist die
  // Wirkung von `sigprocmask` ausdrücklich unbestimmt.
  sigset_t pending;
  sigemptyset(&pending);
  sigaddset(&pending, signum);
  pthread_sigmask(SIG_UNBLOCK, &pending, nullptr);
  // Nur erreichbar, wenn das Signal wider Erwarten nicht zugestellt wurde.
  _exit(128 + signum);
}

void install_handler(int signum) {
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_handler = handle_signal;
  sigfillset(&action.sa_mask);
  action.sa_flags = SA_RESETHAND | SA_ONSTACK;

  struct sigaction previous;
  memset(&previous, 0, sizeof(previous));
  if (sigaction(signum, nullptr, &previous) == 0 &&
      previous.sa_handler == SIG_IGN) {
    // Wer das Signal ignoriert, meint es so: `nohup` setzt `SIGHUP` auf
    // `SIG_IGN`, und ein Handler hier machte aus einem Prozess, der ein
    // abgehängtes Terminal überlebt, einen, der daran stirbt. Ein Protokoll
    // darf das Verhalten nicht ändern, über das es berichtet.
    return;
  }
  sigaction(signum, &action, nullptr);
}

// Legt jedes Verzeichnis auf dem Weg an. `EEXIST` ist kein Fehler; alles
// andere kostet nur das Protokoll und nie den Start.
void make_directories(char* directory) {
  for (size_t i = 1; directory[i] != '\0'; i++) {
    if (directory[i] != '/') {
      continue;
    }
    directory[i] = '\0';
    mkdir(directory, 0700);
    directory[i] = '/';
  }
  mkdir(directory, 0700);
}

// Der zweite Weg zur Zeile des Endes.
//
// Er ist kein Zierrat: Gemessen am 2026-09-13 beendet GTK den Prozess selbst
// mit `exit(1)`, wenn kein Bildschirm da ist („cannot open display"), und
// `g_application_run` kehrt dann nie zurück. Ein Ende, das eine fremde
// Bibliothek auslöst, ist für den Menschen davor dasselbe Rätsel wie jedes
// andere -- also bekommt es dieselbe Zeile. `on_exit` statt `atexit`, weil
// nur das den Rückgabewert mitliefert.
void write_stop_at_exit(int status, void* unused) {
  (void)unused;
  if (g_stop_written) {
    return;
  }
  humanitl_exit_log_stop(status);
}

}  // namespace

size_t humanitl_exit_log_format(char* out, size_t out_len, long long seconds,
                                long nanoseconds, long pid, const char* kind,
                                const char* key, const char* value) {
  if (out == nullptr || out_len == 0) {
    return 0;
  }
  out[0] = '\0';
  if (seconds < 0) {
    seconds = 0;
  }
  long long days = seconds / 86400;
  long long rest = seconds % 86400;
  if (rest < 0) {
    rest += 86400;
    days -= 1;
  }
  long long year = 0;
  unsigned month = 0;
  unsigned day = 0;
  civil_from_days(days, &year, &month, &day);

  size_t at = 0;
  at = append_uint(out, out_len, at, static_cast<unsigned long long>(year), 4);
  at = append_text(out, out_len, at, "-");
  at = append_uint(out, out_len, at, month, 2);
  at = append_text(out, out_len, at, "-");
  at = append_uint(out, out_len, at, day, 2);
  at = append_text(out, out_len, at, "T");
  at = append_uint(out, out_len, at,
                   static_cast<unsigned long long>(rest / 3600), 2);
  at = append_text(out, out_len, at, ":");
  at = append_uint(out, out_len, at,
                   static_cast<unsigned long long>((rest % 3600) / 60), 2);
  at = append_text(out, out_len, at, ":");
  at = append_uint(out, out_len, at, static_cast<unsigned long long>(rest % 60),
                   2);
  at = append_text(out, out_len, at, ".");
  at = append_uint(out, out_len, at,
                   static_cast<unsigned long long>(nanoseconds / 1000000), 3);
  at = append_text(out, out_len, at, "Z ");
  at = append_text(out, out_len, at, kind == nullptr ? "note" : kind);
  at = append_text(out, out_len, at, " pid=");
  at = append_uint(out, out_len, at, static_cast<unsigned long long>(pid), 1);
  if (key != nullptr && value != nullptr) {
    at = append_text(out, out_len, at, " ");
    at = append_text(out, out_len, at, key);
    at = append_text(out, out_len, at, "=");
    at = append_text(out, out_len, at, value);
  }
  return at;
}

const char* humanitl_exit_log_path(void) { return g_path; }

void humanitl_exit_log_note(const char* kind, const char* key,
                            const char* value) {
  struct timespec now;
  if (clock_gettime(CLOCK_REALTIME, &now) != 0) {
    now.tv_sec = 0;
    now.tv_nsec = 0;
  }
  char line[kLineCapacity];
  const size_t len = humanitl_exit_log_format(
      line, sizeof(line), static_cast<long long>(now.tv_sec),
      static_cast<long>(now.tv_nsec), static_cast<long>(getpid()), kind, key,
      value);
  emit_line(line, len, sizeof(line));
}

void humanitl_exit_log_stop(int status) {
  g_stop_written = 1;
  char number[24];
  number[0] = '\0';
  append_uint(number, sizeof(number), 0,
              static_cast<unsigned long long>(status < 0 ? 0 : status), 1);
  humanitl_exit_log_note("stop", "status", number);
}

void humanitl_exit_log_init(void) {
  const char* state = getenv("XDG_STATE_HOME");
  const char* home = getenv("HOME");
  size_t at = 0;
  g_path[0] = '\0';
  if (state != nullptr && state[0] != '\0') {
    at = append_text(g_path, sizeof(g_path), at, state);
  } else {
    at = append_text(g_path, sizeof(g_path), at,
                     (home != nullptr && home[0] != '\0') ? home : ".");
    at = append_text(g_path, sizeof(g_path), at, "/.local/state");
  }
  at = append_text(g_path, sizeof(g_path), at, "/humanitl");
  make_directories(g_path);
  g_dir[0] = '\0';
  append_text(g_dir, sizeof(g_dir), 0, g_path);
  // Auch wenn es das Verzeichnis schon gab: Es ist unseres, und was darin
  // steht, geht kein anderes Konto etwas an (`docs/SECURITY.md` 4 und 8).
  chmod(g_dir, 0700);
  at = append_text(g_path, sizeof(g_path), at, "/app.log");

  g_rotated_path[0] = '\0';
  size_t rotated_at = append_text(g_rotated_path, sizeof(g_rotated_path), 0,
                                  g_path);
  append_text(g_rotated_path, sizeof(g_rotated_path), rotated_at, ".1");

  // Die Datei hier anlegen und nicht erst beim ersten Schreiben: Danach steht
  // sie mit `0600` da, bevor die Dart-Seite ihre erste Zeile schreibt, und
  // eine Datei aus einer älteren Fassung bekommt dieselben Rechte.
  const int fd = open(g_path, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0600);
  if (fd >= 0) {
    close(fd);
  }
  chmod(g_path, 0600);

  if (g_alternate_stack == nullptr) {
    const size_t size = SIGSTKSZ < 32768 ? 32768 : static_cast<size_t>(SIGSTKSZ);
    g_alternate_stack = static_cast<char*>(malloc(size));
    if (g_alternate_stack != nullptr) {
      stack_t alternate;
      memset(&alternate, 0, sizeof(alternate));
      alternate.ss_sp = g_alternate_stack;
      alternate.ss_size = size;
      alternate.ss_flags = 0;
      sigaltstack(&alternate, nullptr);
    }
  }

  g_ready = 1;
  // Ein neuer Lauf hat sein Ende noch vor sich. Das steht hier, weil ein
  // `fork` die Marke des Elternprozesses mitbringt.
  g_stop_written = 0;
  g_rotating = 0;
  on_exit(write_stop_at_exit, nullptr);

  // Ende von außen. Genau die drei aus der Spezifikation, plus `SIGQUIT`, das
  // dieselbe Frage stellt.
  install_handler(SIGTERM);
  install_handler(SIGHUP);
  install_handler(SIGINT);
  install_handler(SIGQUIT);
  // Absturz im nativen Teil. Das ist die Schicht, auf die die Zeile
  // `object_ref: assertion '!object_already_finalized' failed` vom
  // 2026-09-07 zeigt, und die einzige, aus der Dart nichts mehr meldet.
  install_handler(SIGABRT);
  install_handler(SIGSEGV);
  install_handler(SIGBUS);
  install_handler(SIGFPE);
  install_handler(SIGILL);
}
