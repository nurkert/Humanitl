// Die Wegwerf-Sandbox für Tests und Goldens (HUM-351): ein Client mit der
// Fähigkeit `sandbox.disposable`, dessen Änderungen warten, und die
// Kennungen des Szenarios.

import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import 'harness.dart';

/// Die Kennungen des Szenarios `disposable`, nach Pfad.
const String logoId = '6071829304a5b6c7';
const String readmeId = '0a1b2c3d4e5f6071';
const String notesId = '1b2c3d4e5f607182';
const String linkId = '2c3d4e5f60718293';
const String scriptId = '3d4e5f60718293a4';
const String mainId = '4e5f60718293a4b5';
const String utilId = '5f60718293a4b5c6';

/// Ein Client, der Wegwerf-Sandboxen kann und dessen Änderungen warten.
SandboxTestClient disposableClient({DisposableChanges? changes}) {
  final SandboxTestClient client = SandboxTestClient(
    info: FakeDaemonClient.disposableInfo,
  );
  client
    ..pendingChanges = changes ?? fakeDisposableChanges()
    ..disposableRunChanges = fakeDisposableChanges();
  return client;
}
