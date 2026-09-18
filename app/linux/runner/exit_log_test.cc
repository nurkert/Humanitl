// Test des Protokolls, das der Runner schreibt (HUM-136).
//
// Er läuft nicht unter `flutter test`: Das hier ist C++ und kein Dart, und
// `flutter build linux` übersetzt nur das Ziel `runner`, nicht diese Datei.
// Ausgeführt wird er über `./run_exit_log_test.sh` in diesem Verzeichnis, das
// ihn übersetzt und startet.
//
// Geprüft wird, was von außen sichtbar ist: das Format einer Zeile, die
// Auflösung des Pfades, die Rotation an der Obergrenze und -- der eigentliche
// Punkt des Issues -- dass ein `SIGTERM` eine Zeile hinterlässt und den
// Prozess trotzdem so beendet, wie er ohne den Handler geendet hätte.

#include "exit_log.h"

#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include <string>

namespace {

int g_failures = 0;

// Vor jedem `fork`: Ein Kind, das `exit` ruft, leert die geerbte Kopie des
// Ausgabepuffers und schriebe jede bisherige Zeile ein zweites Mal.
pid_t fork_after_flush() {
  fflush(nullptr);
  return fork();
}

void check(bool condition, const char* what) {
  if (condition) {
    printf("ok   %s\n", what);
    return;
  }
  printf("FAIL %s\n", what);
  g_failures++;
}

std::string read_file(const std::string& path) {
  FILE* file = fopen(path.c_str(), "rb");
  if (file == nullptr) {
    return std::string();
  }
  std::string out;
  char buffer[4096];
  size_t n = 0;
  while ((n = fread(buffer, 1, sizeof(buffer), file)) > 0) {
    out.append(buffer, n);
  }
  fclose(file);
  return out;
}

long file_size(const std::string& path) {
  struct stat info;
  if (stat(path.c_str(), &info) != 0) {
    return -1;
  }
  return static_cast<long>(info.st_size);
}

// Das Format, ohne dass ein Prozess dafür sterben muss.
void test_format() {
  char line[256];
  // 1757756400 = 2025-09-13T09:40:00Z, nachgerechnet mit `date -u -d @...`.
  const size_t len = humanitl_exit_log_format(line, sizeof(line), 1757756400LL,
                                              123000000L, 4242L, "signal",
                                              "name", "SIGTERM");
  check(len == strlen(line), "format liefert die Laenge der Zeile");
  check(std::string(line) ==
            "2025-09-13T09:40:00.123Z signal pid=4242 name=SIGTERM",
        "format baut Zeitpunkt, Art, Prozesskennung und Feld");

  humanitl_exit_log_format(line, sizeof(line), 0LL, 0L, 1L, "start", nullptr,
                           nullptr);
  check(std::string(line) == "1970-01-01T00:00:00.000Z start pid=1",
        "format kommt ohne Feld aus und kennt den Beginn der Epoche");

  humanitl_exit_log_format(line, sizeof(line), 1LL, 0L, 1L, "note", "text",
                           "zwei\nzeilen");
  check(std::string(line).find('\n') == std::string::npos,
        "format macht aus einem Umbruch nie eine zweite Zeile");
}

void test_path(const std::string& state_home) {
  check(std::string(humanitl_exit_log_path()) ==
            state_home + "/humanitl/app.log",
        "der Pfad folgt XDG_STATE_HOME");
  struct stat info;
  check(stat((state_home + "/humanitl").c_str(), &info) == 0,
        "das Verzeichnis wird angelegt");
}

// Das Protokoll trägt Fehlertexte, also gehört es niemandem sonst
// (`docs/SECURITY.md` 4 und 8). Der Runner legt beides an, bevor die
// Dart-Seite ihre erste Zeile schreibt.
void test_modes(const std::string& state_home, const std::string& path) {
  struct stat directory;
  struct stat file;
  check(stat((state_home + "/humanitl").c_str(), &directory) == 0 &&
            (directory.st_mode & 0777) == 0700,
        "das Verzeichnis steht auf 0700");
  check(stat(path.c_str(), &file) == 0 && (file.st_mode & 0777) == 0600,
        "die Datei steht auf 0600");
}

void test_stop(const std::string& path) {
  humanitl_exit_log_stop(0);
  const std::string content = read_file(path);
  check(content.find(" stop pid=") != std::string::npos,
        "das geordnete Ende schreibt eine stop-Zeile");
  check(content.find("status=0") != std::string::npos,
        "die stop-Zeile nennt den Rueckgabewert");
}

// Die Obergrenze gilt auch für den Runner: Wer 256 KiB überschreitet,
// rotiert, statt weiterzuwachsen.
void test_rotation(const std::string& path) {
  FILE* file = fopen(path.c_str(), "wb");
  if (file == nullptr) {
    check(false, "die Testdatei liess sich nicht fuellen");
    return;
  }
  const std::string filler(1024, 'x');
  for (int i = 0; i < 256; i++) {
    fwrite(filler.data(), 1, filler.size(), file);
  }
  fclose(file);

  humanitl_exit_log_note("note", "why", "rotation");
  check(file_size(path + ".1") == 256L * 1024L,
        "die alte Datei steht vollstaendig unter app.log.1");
  check(file_size(path) > 0 && file_size(path) < 256L * 1024L,
        "die neue Datei ist kleiner als die Obergrenze");
  check(read_file(path).find("why=rotation") != std::string::npos,
        "die Zeile, die rotieren liess, steht in der neuen Datei");
}

// Der Kern des Issues: ein Ende von außen hinterlässt eine Zeile, und der
// Prozess stirbt trotzdem an dem Signal, das ihn getroffen hat.
void test_signal(const std::string& path) {
  unlink(path.c_str());
  const pid_t child = fork_after_flush();
  if (child == 0) {
    humanitl_exit_log_init();
    raise(SIGTERM);
    _exit(0);
  }
  int status = 0;
  waitpid(child, &status, 0);
  check(WIFSIGNALED(status) && WTERMSIG(status) == SIGTERM,
        "der Prozess stirbt weiterhin an SIGTERM, nicht an exit(0)");

  const std::string content = read_file(path);
  check(content.find(" signal pid=") != std::string::npos,
        "SIGTERM hinterlaesst eine signal-Zeile");
  check(content.find("name=SIGTERM") != std::string::npos,
        "die Zeile nennt das Signal");
  char expected[64];
  snprintf(expected, sizeof(expected), "signal pid=%ld",
           static_cast<long>(child));
  check(content.find(expected) != std::string::npos,
        "die Zeile nennt die Prozesskennung des Kindes");
}

// Ein Ende, das eine fremde Bibliothek auslöst, ohne dass die Schleife
// zurückkehrt: GTK ruft `exit(1)`, wenn kein Bildschirm da ist. Auch dieser
// Weg hinterlässt seine Zeile, und nur eine.
void test_exit_without_stop(const std::string& path) {
  unlink(path.c_str());
  const pid_t child = fork_after_flush();
  if (child == 0) {
    humanitl_exit_log_init();
    exit(3);
  }
  int status = 0;
  waitpid(child, &status, 0);
  check(WIFEXITED(status) && WEXITSTATUS(status) == 3,
        "der Rueckgabewert bleibt derselbe");

  const std::string content = read_file(path);
  check(content.find("status=3") != std::string::npos,
        "ein exit aus einer Bibliothek hinterlaesst die stop-Zeile");
  size_t lines = 0;
  for (size_t i = 0; i < content.size(); i++) {
    if (content[i] == '\n') {
      lines++;
    }
  }
  check(lines == 1, "und genau eine Zeile, nicht zwei");
}

// Und andersherum: Wer die Zeile des Endes schon geschrieben hat, bekommt vom
// Netz keine zweite.
void test_stop_is_written_once(const std::string& path) {
  unlink(path.c_str());
  const pid_t child = fork_after_flush();
  if (child == 0) {
    humanitl_exit_log_init();
    humanitl_exit_log_stop(0);
    exit(0);
  }
  int status = 0;
  waitpid(child, &status, 0);

  const std::string content = read_file(path);
  size_t stops = 0;
  for (size_t i = content.find(" stop pid="); i != std::string::npos;
       i = content.find(" stop pid=", i + 1)) {
    stops++;
  }
  check(stops == 1, "nach einem geordneten Ende steht genau eine stop-Zeile");
}

// Derselbe Weg für einen Absturz im nativen Teil.
void test_crash(const std::string& path) {
  unlink(path.c_str());
  const pid_t child = fork_after_flush();
  if (child == 0) {
    humanitl_exit_log_init();
    abort();
  }
  int status = 0;
  waitpid(child, &status, 0);
  check(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT,
        "ein abort bleibt ein abort");
  check(read_file(path).find("name=SIGABRT") != std::string::npos,
        "der Absturz hinterlaesst seine Zeile");
}

}  // namespace

int main() {
  char templ[] = "/tmp/humanitl-exit-log-test-XXXXXX";
  const char* directory = mkdtemp(templ);
  if (directory == nullptr) {
    printf("FAIL kein temporaeres Verzeichnis\n");
    return 1;
  }
  setenv("XDG_STATE_HOME", directory, 1);
  const std::string path = std::string(directory) + "/humanitl/app.log";

  test_format();
  humanitl_exit_log_init();
  test_path(directory);
  test_modes(directory, path);
  test_stop(path);
  test_rotation(path);
  test_signal(path);
  test_exit_without_stop(path);
  test_stop_is_written_once(path);
  test_crash(path);

  if (g_failures == 0) {
    printf("\nalle Pruefungen gruen\n");
    return 0;
  }
  printf("\n%d rot\n", g_failures);
  return 1;
}
