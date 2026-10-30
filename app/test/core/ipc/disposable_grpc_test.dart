// Die Wegwerf-Sandbox über die Leitung (HUM-351): was der gRPC-Client für
// Start, Changes, ApplyChanges und DiscardChanges schickt und wie er die
// beiden neuen Ereignisse übersetzt.

import 'dart:io';

import 'package:fixnum/fixnum.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:grpc/grpc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/convert.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pb.dart' as pb;
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pbgrpc.dart'
    as pbgrpc;
import 'package:humanitl/core/ipc/grpc_daemon_client.dart';

/// Eine Änderung je Art, wie der Daemon sie schreibt.
pb.DisposableChange change(String id, pb.FileChangeKind kind) =>
    pb.DisposableChange(id: id, path: 'p/$id', kind: kind);

/// Ein Daemon, der jede Sandbox-Anfrage festhält und auf jede Operation so
/// antwortet wie `daemon/crates/ipc/src/sandbox.rs`.
class RecordingDaemon extends pbgrpc.HumanitlServiceBase {
  /// Jede Anfrage, älteste zuerst.
  final List<pb.SandboxRequest> requests = <pb.SandboxRequest>[];

  @override
  Stream<pb.SandboxEvent> sandbox(
    ServiceCall call,
    pb.SandboxRequest request,
  ) async* {
    requests.add(request);
    switch (request.whichOp()) {
      case pb.SandboxRequest_Op.start:
        yield pb.SandboxEvent(exit: pb.SandboxEvent_Exit(code: 0));
        yield pb.SandboxEvent(
          changes: pb.DisposableChanges(
            sandboxId: 'sb-1',
            workDir: '/home/u/p',
            truncated: true,
            changes: <pb.DisposableChange>[
              change('a', pb.FileChangeKind.FILE_CHANGE_KIND_ADDED),
              change('m', pb.FileChangeKind.FILE_CHANGE_KIND_MODIFIED)
                ..conflict = 'changed since the start'
                ..diff = '@@ -1 +1 @@\n-a\n+b\n'
                ..size = Int64(3),
              change('r', pb.FileChangeKind.FILE_CHANGE_KIND_REMOVED),
              change('s', pb.FileChangeKind.FILE_CHANGE_KIND_SYMLINK_ADDED)
                ..symlinkTarget = '../x'
                ..refused = 'leaves the project',
              change('x', pb.FileChangeKind.FILE_CHANGE_KIND_MODE_CHANGED)
                ..strippedBits = 2048
                ..binary = true
                ..sha256 = 'ab',
            ],
          ),
        );
      case pb.SandboxRequest_Op.applyChanges:
        yield pb.SandboxEvent(
          changesOutcome: pb.DisposableOutcome(
            sandboxId: 'sb-1',
            outcome: 'partial',
            applied: <String>['p/a'],
          ),
        );
      case pb.SandboxRequest_Op.discardChanges:
        yield pb.SandboxEvent(
          changesOutcome: pb.DisposableOutcome(
            sandboxId: 'sb-1',
            outcome: 'discarded',
            discarded: <String>['p/a', 'p/m'],
          ),
        );
      default:
        break;
    }
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  test('every wire kind has its domain kind, and a new one stays a row', () {
    expect(
      <DisposableChangeKind>[
        for (final pb.FileChangeKind kind in <pb.FileChangeKind>[
          pb.FileChangeKind.FILE_CHANGE_KIND_ADDED,
          pb.FileChangeKind.FILE_CHANGE_KIND_MODIFIED,
          pb.FileChangeKind.FILE_CHANGE_KIND_REMOVED,
          pb.FileChangeKind.FILE_CHANGE_KIND_SYMLINK_ADDED,
          pb.FileChangeKind.FILE_CHANGE_KIND_MODE_CHANGED,
          pb.FileChangeKind.FILE_CHANGE_KIND_UNSPECIFIED,
        ])
          change('k', kind).toDomain().kind,
      ],
      <DisposableChangeKind>[
        DisposableChangeKind.added,
        DisposableChangeKind.modified,
        DisposableChangeKind.removed,
        DisposableChangeKind.symlink,
        DisposableChangeKind.mode,
        DisposableChangeKind.unknown,
      ],
    );
    expect(
      DisposableOutcomeKind.fromWire('later'),
      DisposableOutcomeKind.unknown,
    );
  });

  group('over a socket', () {
    late RecordingDaemon daemon;
    late GrpcDaemonClient client;

    setUp(() async {
      final Directory dir = Directory.systemTemp.createTempSync('hum351-');
      addTearDown(() => dir.deleteSync(recursive: true));
      Process.runSync('chmod', <String>['700', dir.path]);
      final String socketPath = '${dir.path}/daemon.sock';
      final File token = File('${dir.path}/token')
        ..writeAsStringSync('secret\n');
      Process.runSync('chmod', <String>['600', token.path]);
      daemon = RecordingDaemon();
      final Server server = Server.create(services: <Service>[daemon]);
      await server.serve(
        address: InternetAddress(socketPath, type: InternetAddressType.unix),
        port: 0,
      );
      addTearDown(server.shutdown);
      client = GrpcDaemonClient(socketPath: socketPath, tokenPath: token.path);
      addTearDown(client.close);
    });

    test(
      'the start sends the mode, and the changes arrive on its stream',
      () async {
        final List<SandboxUpdate> updates = await client
            .startSandbox(sandboxMode: SandboxMode.disposable)
            .toList()
            .timeout(const Duration(seconds: 20));
        await client.startSandbox().toList().timeout(
          const Duration(seconds: 20),
        );

        expect(daemon.requests[0].start.sandboxMode, 'disposable');
        expect(
          daemon.requests[1].start.sandboxMode,
          '',
          reason: 'no choice leaves the field empty for sandbox.mode',
        );
        final DisposableChanges changes =
            (updates.last as SandboxUpdateChanges).changes;
        expect(changes.sandboxId, 'sb-1');
        expect(changes.workDir, '/home/u/p');
        expect(changes.truncated, isTrue);
        expect(
          changes.changes.map((DisposableChange c) => c.kind),
          <DisposableChangeKind>[
            DisposableChangeKind.added,
            DisposableChangeKind.modified,
            DisposableChangeKind.removed,
            DisposableChangeKind.symlink,
            DisposableChangeKind.mode,
          ],
        );
        final DisposableChange modified = changes.changes[1];
        expect(modified.conflict, 'changed since the start');
        expect(modified.diff, '@@ -1 +1 @@\n-a\n+b\n');
        expect(modified.size, 3);
        expect(changes.changes[3].refused, 'leaves the project');
        expect(changes.changes[3].symlinkTarget, '../x');
        expect(changes.changes[4].strippedBits, 2048);
        expect(changes.changes[4].binary, isTrue);
      },
    );

    test(
      'apply sends exactly the ids and the overwrite ids it was given',
      () async {
        final List<SandboxUpdate> updates = await client
            .applyChanges(
              sandboxId: 'sb-1',
              ids: <String>['m', 'a'],
              overwriteIds: <String>['m'],
            )
            .toList()
            .timeout(const Duration(seconds: 20));

        final pb.SandboxRequest_ApplyChanges sent =
            daemon.requests.single.applyChanges;
        expect(sent.sandboxId, 'sb-1');
        expect(sent.ids, <String>['m', 'a']);
        expect(sent.overwriteIds, <String>['m']);
        expect(
          sent.overwrite,
          isFalse,
          reason: 'the app never confirms every chosen id at once',
        );
        expect(
          updates.single,
          const SandboxUpdate.changesOutcome(
            DisposableOutcome(
              sandboxId: 'sb-1',
              outcome: DisposableOutcomeKind.partial,
              applied: <String>['p/a'],
            ),
          ),
        );
      },
    );

    test('discard names the sandbox and reads the outcome', () async {
      final List<SandboxUpdate> updates = await client
          .discardChanges(sandboxId: 'sb-1')
          .toList()
          .timeout(const Duration(seconds: 20));

      expect(daemon.requests.single.discardChanges.sandboxId, 'sb-1');
      final DisposableOutcome outcome =
          (updates.single as SandboxUpdateChangesOutcome).outcome;
      expect(outcome.outcome, DisposableOutcomeKind.discarded);
      expect(outcome.discarded, <String>['p/a', 'p/m']);
    });

    test('changes asks with the changes operation', () async {
      await client.sandboxChanges().toList().timeout(
        const Duration(seconds: 20),
      );
      expect(daemon.requests.single.whichOp(), pb.SandboxRequest_Op.changes);
    });
  });
}
