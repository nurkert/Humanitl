// Rauchtest für den erzeugten Dart-Code aus `proto/humanitl/v1/` (HUM-003).
//
// `lib/core/ipc/generated/` ist gitignored und entsteht mit `make proto`
// (`scripts/gen-proto.sh`). Dieser Test beweist, dass der Code existiert,
// kompiliert und Nachrichten unverändert durch `writeToBuffer`/`fromBuffer`
// bringt. Er hält keine Serialisierungsdetails fest, nur die Zusagen des
// Vertrags: Enums haben eine `_UNSPECIFIED`-Null, und jedes Ereignis des
// `oneof event` überlebt die Leitung.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/common.pbenum.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pb.dart';

void main() {
  test('Info survives a buffer roundtrip', () {
    final info = Info()
      ..daemonVersion = '0.0.0'
      ..protoMajor = 1
      ..protoMinor = 0
      ..capabilities.addAll(['sandbox.bwrap', 'proxy.h1'])
      ..sessionId = '018f0000-0000-7000-8000-000000000001';

    final decoded = Info.fromBuffer(info.writeToBuffer());

    expect(decoded, info);
    expect(decoded.protoMajor, 1);
    expect(decoded.capabilities, ['sandbox.bwrap', 'proxy.h1']);
  });

  test('FlowEvent.held survives a buffer roundtrip', () {
    final event = FlowEvent()
      ..held = (FlowEvent_Held()
        ..flowId = '018f0000-0000-7000-8000-000000000002'
        ..queueCount = 3);

    final decoded = FlowEvent.fromBuffer(event.writeToBuffer());

    expect(decoded, event);
    expect(decoded.whichEvent(), FlowEvent_Event.held);
    expect(decoded.held.flowId, '018f0000-0000-7000-8000-000000000002');
    expect(decoded.held.queueCount, 3);
  });

  test('FlowEvent.failed carries the upstream error', () {
    final event = FlowEvent()
      ..failed = (FlowEvent_Failed()
        ..flowId = '018f0000-0000-7000-8000-000000000003'
        ..error = UpstreamError.UPSTREAM_ERROR_PRIVATE_ADDRESS
        ..resolvedIp = '10.0.0.7');

    final decoded = FlowEvent.fromBuffer(event.writeToBuffer());

    expect(decoded.whichEvent(), FlowEvent_Event.failed);
    expect(decoded.failed.error, UpstreamError.UPSTREAM_ERROR_PRIVATE_ADDRESS);
    expect(decoded.failed.resolvedIp, '10.0.0.7');
  });

  test('DecideRequest.allowEdited carries the full body', () {
    // `EditedRequest` ist die eine Stelle, an der ein Body als Inhalt zum
    // Daemon reist; `HttpRequest` kennt nur den `BodyRef`.
    final request = DecideRequest()
      ..flowIds.add('018f0000-0000-7000-8000-000000000004')
      ..allowEdited = (EditedRequest()
        ..method = Method.METHOD_POST
        ..url = 'https://api.github.com/repos'
        ..headers.add(
          Header()
            ..name = 'content-type'
            ..value = [0x61, 0x2f, 0x62],
        )
        ..body = [0x00, 0xff, 0x7b]);

    final decoded = DecideRequest.fromBuffer(request.writeToBuffer());

    expect(decoded, request);
    expect(decoded.whichDecision(), DecideRequest_Decision.allowEdited);
    expect(decoded.allowEdited.body, [0x00, 0xff, 0x7b]);
    expect(decoded.allowEdited.url, 'https://api.github.com/repos');
    expect(decoded.allowEdited.method, Method.METHOD_POST);
  });

  test('FlowDetail.bodyPreview is text', () {
    // Hoechstens 4096 Zeichen, verlustbehaftetes UTF-8; nur im Detail, nie
    // in einem FlowEvent (docs/PROTOCOL.md 4).
    final detail = FlowDetail()..bodyPreview = '{"edited":false}';

    final decoded = FlowDetail.fromBuffer(detail.writeToBuffer());

    expect(decoded.bodyPreview, '{"edited":false}');
  });

  test('enums have an unspecified zero', () {
    expect(Method.valueOf(0), Method.METHOD_UNSPECIFIED);
    expect(Scheme.valueOf(0), Scheme.SCHEME_UNSPECIFIED);
    expect(FlowState.valueOf(0), FlowState.FLOW_STATE_UNSPECIFIED);
    expect(BlockReason.valueOf(0), BlockReason.BLOCK_REASON_UNSPECIFIED);
    expect(UpstreamError.valueOf(0), UpstreamError.UPSTREAM_ERROR_UNSPECIFIED);
    // Ein neuerer Daemon darf Werte schicken, die dieser Client nicht kennt.
    expect(BlockReason.valueOf(9999), isNull);
  });
}
