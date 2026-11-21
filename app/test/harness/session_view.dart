// Die App so, wie sie in der Ansicht einer Sitzung steht (HUM-537).
//
// Seit HUM-537 öffnet die App ins Startfenster und zeigt die Shell erst,
// wenn jemand dort eine Sitzung angehängt, ein Projekt gestartet oder einen
// Ordner gewählt hat. Die Tests der Shell und ihrer Bereiche handeln von der
// Shell, nicht vom Weg dorthin; jedes Gerüst, das die ganze App baut, legt
// deshalb `inSessionView` vor die eigenen Overrides. Wer das Startfenster
// prüft, lässt es weg (`pumpApp(..., startWindow: true)`).

import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/session_view.dart';

/// Die Sitzungsansicht des Projekts in [path]; ohne Angabe der Ordner, den
/// der Fake als `sandbox.work_dir` führt.
/// Mit [projectId] ist es ein Projekt des Registers.
Override inSessionView({
  String path = FakeDaemonClient.defaultWorkDir,
  String projectId = '',
}) => sessionViewProvider.overrideWith(
  () =>
      SessionViewNotifier(SessionView(projectPath: path, projectId: projectId)),
);
