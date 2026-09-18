#include "exit_log.h"
#include "my_application.h"

int main(int argc, char** argv) {
  // Vor allem anderen: Von hier an hinterlässt jedes Ende dieses Prozesses
  // eine Zeile (HUM-136).
  humanitl_exit_log_init();

  g_autoptr(MyApplication) app = my_application_new();
  const int status = g_application_run(G_APPLICATION(app), argc, argv);

  // Die Schleife ist zurückgekehrt, also war es ein geordnetes Ende. Ein
  // Absturz und ein Signal kommen hier nie an; die schreiben ihre Zeile im
  // Handler.
  humanitl_exit_log_stop(status);
  return status;
}
