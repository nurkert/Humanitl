// HUM-164, Review: Ein Herzschlag, der auf `GetInfo` wartet, bekommt keinen
// zweiten neben sich. Seit dem Weckruf kann ein `GetInfo` bis zu zehn
// Sekunden dauern; ein Takt von zwei Sekunden legte sonst bis zu fünf
// Weckversuche übereinander.

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/proto_version.dart';

const DaemonInfo info = DaemonInfo(
  daemonVersion: 'hum164',
  protoMajor: ProtoVersion.major,
  protoMinor: ProtoVersion.minor,
);

/// Ein Client, dessen erstes `GetInfo` sofort antwortet und dessen weitere
/// hängen, bis der Test sie freigibt: ein Herzschlag, der auf einen
/// geweckten Daemon wartet.
class SlowAfterFirst implements DaemonClient {
  /// Wie oft `GetInfo` gerufen wurde.
  int calls = 0;

  /// Die offenen Antworten.
  final List<Completer<DaemonInfo>> pending = <Completer<DaemonInfo>>[];

  @override
  Future<DaemonInfo> getInfo() {
    calls++;
    if (calls == 1) {
      return Future<DaemonInfo>.value(info);
    }
    final Completer<DaemonInfo> answer = Completer<DaemonInfo>();
    pending.add(answer);
    return answer.future;
  }

  @override
  Future<void> close() async {}

  // Die übrigen Methoden der Schnittstelle ruft dieser Test nie.
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  test('a heartbeat that still waits gets no second one beside it', () async {
    final SlowAfterFirst client = SlowAfterFirst();
    final ProviderContainer container = ProviderContainer(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(
          const Duration(milliseconds: 20),
        ),
        connectionReconnectProvider.overrideWithValue(null),
      ],
    );
    addTearDown(() {
      for (final Completer<DaemonInfo> answer in client.pending) {
        answer.complete(info);
      }
      container.dispose();
    });
    final ProviderSubscription<ConnectionStatus> status = container.listen(
      connectionStateProvider,
      (_, _) {},
    );
    addTearDown(status.close);
    await container.read(daemonInfoProvider.future);
    expect(status.read(), isA<ConnectionConnected>());

    // Zehn Takte, und der erste Herzschlag wartet die ganze Zeit.
    await Future<void>.delayed(const Duration(milliseconds: 220));

    expect(
      client.calls,
      2,
      reason: 'one GetInfo to connect, then one heartbeat that still waits',
    );
  });
}
