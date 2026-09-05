/// Translation between the generated wire types (`generated/humanitl/v1`) and
/// the domain mirror (`core/domain`). The only file outside `generated/` that
/// knows a proto field by name.
///
/// Enums are mapped by position: every wire enum starts with `_UNSPECIFIED =
/// 0` and lists its members in the same order as the domain enum, so member
/// `n` of the wire enum is member `n - 1` of the domain enum. `test/core/ipc/
/// convert_test.dart` checks that both sides agree name by name, which is
/// what keeps this cheap trick honest. An unknown value from a newer daemon
/// becomes `null` on optional fields; required fields fall back to a value
/// that is wrong in the least harmful way and is documented at the site.
library;

import 'dart:typed_data';

import 'package:fixnum/fixnum.dart';
import 'package:protobuf/well_known_types/google/protobuf/duration.pb.dart'
    as wkt;
import 'package:protobuf/well_known_types/google/protobuf/empty.pb.dart' as wkt;
import 'package:protobuf/well_known_types/google/protobuf/timestamp.pb.dart'
    as wkt;

import '../domain/domain.dart';
import 'generated/humanitl/v1/common.pbenum.dart' as pb;
import 'generated/humanitl/v1/humanitl.pb.dart' as pb;
import 'generated/humanitl/v1/rules.pb.dart' as pb;

/// Member `wire - 1` of [values], or null for `0` and unknown values.
T? enumFromWire<T>(List<T> values, int wire) =>
    wire >= 1 && wire <= values.length ? values[wire - 1] : null;

/// The wire number of [member]: its index plus one.
int enumToWire<T extends Enum>(T member) => member.index + 1;

DateTime _dateTime(wkt.Timestamp timestamp) =>
    timestamp.toDateTime(toLocal: true);

wkt.Timestamp _timestamp(DateTime dateTime) =>
    wkt.Timestamp.fromDateTime(dateTime.toUtc());

Duration _duration(wkt.Duration duration) => Duration(
  seconds: duration.seconds.toInt(),
  microseconds: duration.nanos ~/ 1000,
);

/// `Info` to [DaemonInfo].
extension InfoToDomain on pb.Info {
  /// The domain form.
  DaemonInfo toDomain() => DaemonInfo(
    daemonVersion: daemonVersion,
    protoMajor: protoMajor,
    protoMinor: protoMinor,
    capabilities: List<String>.unmodifiable(capabilities),
    sessionId: sessionId,
  );
}

/// `Severity` to [Severity].
extension SeverityToDomain on pb.Severity {
  /// The domain form; unknown values count as [Severity.error].
  Severity toDomain() => enumFromWire(Severity.values, value) ?? Severity.error;
}

/// `FixAction` to [FixAction].
extension FixActionToDomain on pb.FixAction {
  /// The domain form, or null when no action is set.
  FixAction? toDomain() => switch (whichAction()) {
    pb.FixAction_Action.setEnv => FixAction.setEnv(
      key: setEnv.key,
      value: setEnv.value,
    ),
    pb.FixAction_Action.addRule => FixAction.addRule(rule: addRule.toDomain()),
    pb.FixAction_Action.installService => const FixAction.installService(),
    pb.FixAction_Action.changeSetting => FixAction.changeSetting(
      key: changeSetting.key,
      value: changeSetting.value,
    ),
    pb.FixAction_Action.copyCommand => FixAction.copyCommand(
      command: copyCommand,
    ),
    pb.FixAction_Action.openUrl => FixAction.openUrl(url: openUrl),
    pb.FixAction_Action.remountReadOnly => FixAction.remountReadOnly(
      path: remountReadOnly,
    ),
    pb.FixAction_Action.notSet => null,
  };
}

/// `Diagnostic` to [Diagnostic].
extension DiagnosticToDomain on pb.Diagnostic {
  /// The domain form.
  Diagnostic toDomain() => Diagnostic(
    code: code,
    severity: severity.toDomain(),
    title: title,
    why: why,
    fix: hasFix() ? fix.toDomain() : null,
    docsUrl: docsUrl.isEmpty ? null : docsUrl,
  );
}

/// `Authority` to [Authority].
extension AuthorityToDomain on pb.Authority {
  /// The domain form.
  Authority toDomain() => Authority(
    host: host,
    port: port,
    isIpLiteral: isIpLiteral,
    displayHost: displayHost,
  );
}

/// `Header` to [Header].
extension HeaderToDomain on pb.Header {
  /// The domain form.
  Header toDomain() => Header(name: name, value: List<int>.unmodifiable(value));
}

/// `BodyRef` to [BodyRef].
extension BodyRefToDomain on pb.BodyRef {
  /// The domain form.
  BodyRef toDomain() => BodyRef(
    sha256: List<int>.unmodifiable(sha256),
    size: size.toInt(),
    truncated: truncated,
    contentType: contentType,
  );
}

/// [BodyRef] to `BodyRef`.
extension BodyRefToProto on BodyRef {
  /// The wire form.
  pb.BodyRef toProto() => pb.BodyRef()
    ..sha256 = sha256
    ..size = Int64(size)
    ..truncated = truncated
    ..contentType = contentType;
}

/// `HttpRequest` to [HttpRequest].
extension HttpRequestToDomain on pb.HttpRequest {
  /// The domain form. An unknown method is [Method.other] with the raw
  /// token kept; an unknown scheme counts as [Scheme.https].
  HttpRequest toDomain() => HttpRequest(
    method: enumFromWire(Method.values, method.value) ?? Method.other,
    methodRaw: methodRaw,
    scheme: enumFromWire(Scheme.values, scheme.value) ?? Scheme.https,
    authority: authority.toDomain(),
    pathAndQuery: pathAndQuery,
    headers: List<Header>.unmodifiable(headers.map((h) => h.toDomain())),
    body: body.toDomain(),
    version: version,
  );
}

/// `HttpResponseHead` to [HttpResponseHead].
extension ResponseHeadToDomain on pb.HttpResponseHead {
  /// The domain form.
  HttpResponseHead toDomain() => HttpResponseHead(
    status: status,
    headers: List<Header>.unmodifiable(headers.map((h) => h.toDomain())),
    version: version,
  );
}

/// `Finding` to [Finding].
extension FindingToDomain on pb.Finding {
  /// The domain form. Unknown locations count as the body, unknown tiers as
  /// [FindingTier.regex].
  Finding toDomain() => Finding(
    kind: kind,
    location:
        enumFromWire(FindingLocation.values, location.value) ??
        FindingLocation.body,
    headerName: headerName,
    spanStart: spanStart.toInt(),
    spanEnd: spanEnd.toInt(),
    tier: enumFromWire(FindingTier.values, tier.value) ?? FindingTier.regex,
    valueHash: List<int>.unmodifiable(valueHash),
    displayPrefix: displayPrefix,
    resolved: resolved,
  );
}

/// `DomainInfo` to [DomainInfo].
extension DomainInfoToDomain on pb.DomainInfo {
  /// The domain form.
  DomainInfo toDomain() => DomainInfo(
    apex: apex,
    catalogId: catalogId,
    trancoRank: trancoRank,
    firstSeen: hasFirstSeen() ? _dateTime(firstSeen) : null,
    seenCount: seenCount,
  );
}

/// `FlowSummary` to [Flow].
extension FlowSummaryToDomain on pb.FlowSummary {
  /// The domain form. An unknown state counts as [FlowState.received], which
  /// is the one state that never triggers an action in the UI.
  Flow toDomain() => Flow(
    id: FlowId(flowId),
    sessionId: SessionId(sessionId),
    receivedAt: hasReceivedAt()
        ? _dateTime(receivedAt)
        : DateTime.fromMillisecondsSinceEpoch(0),
    method: enumFromWire(Method.values, method.value) ?? Method.other,
    methodRaw: methodRaw,
    scheme: enumFromWire(Scheme.values, scheme.value) ?? Scheme.https,
    authority: authority.toDomain(),
    path: path,
    state: enumFromWire(FlowState.values, state.value) ?? FlowState.received,
    decision: enumFromWire(DecisionKind.values, decision.value),
    decisionSource: enumFromWire(DecisionSource.values, decisionSource.value),
    blockReason: enumFromWire(BlockReason.values, blockReason.value),
    ruleId: ruleId.isEmpty ? null : RuleId(ruleId),
    status: status,
    requestSize: requestSize.toInt(),
    responseSize: responseSize.toInt(),
    duration: hasDuration() ? _duration(duration) : null,
    findingCount: findingCount,
    edited: edited,
    passthrough: passthrough,
    meta: meta,
    deadline: hasDeadline() ? _dateTime(deadline) : null,
    originTool: originTool,
    upstreamError: enumFromWire(UpstreamError.values, upstreamError.value),
  );
}

/// `FlowDetail` to [FlowDetail].
extension FlowDetailToDomain on pb.FlowDetail {
  /// The domain form.
  FlowDetail toDomain() => FlowDetail(
    summary: summary.toDomain(),
    request: hasRequest() ? request.toDomain() : null,
    editedRequest: hasEditedRequest() ? editedRequest.toDomain() : null,
    response: hasResponse() ? response.toDomain() : null,
    responseBody: hasResponseBody() ? responseBody.toDomain() : null,
    findings: List<Finding>.unmodifiable(findings.map((f) => f.toDomain())),
    diagnostics: List<Diagnostic>.unmodifiable(
      diagnostics.map((d) => d.toDomain()),
    ),
    domain: hasDomain() ? domain.toDomain() : null,
    bodyPreview: bodyPreview,
  );
}

/// `FlowPage` to [FlowPage].
extension FlowPageToDomain on pb.FlowPage {
  /// The domain form.
  FlowPage toDomain() => FlowPage(
    flows: List<Flow>.unmodifiable(flows.map((f) => f.toDomain())),
    nextCursor: nextCursor,
    total: total.toInt(),
    capped: capped,
  );
}

/// [FlowFilter] to `ListFlowsRequest`.
extension FlowFilterToProto on FlowFilter {
  /// The wire form with paging.
  pb.ListFlowsRequest toProto({required int limit, String? cursor}) =>
      pb.ListFlowsRequest()
        ..filter = query
        ..sinceFlowId = since?.value ?? ''
        ..cursor = cursor ?? ''
        ..limit = limit
        ..orderBy = orderBy
        ..includePassthrough = includePassthrough;
}

/// `FlowEvent` to [FlowEvent].
extension FlowEventToDomain on pb.FlowEvent {
  /// The domain form, or null for an event this client does not know.
  FlowEvent? toDomain() {
    final DateTime when = hasAt()
        ? _dateTime(at)
        : DateTime.fromMillisecondsSinceEpoch(0);
    return switch (whichEvent()) {
      pb.FlowEvent_Event.received => FlowEvent.received(
        at: when,
        flow: received.summary.toDomain(),
        domain: received.hasDomain() ? received.domain.toDomain() : null,
      ),
      pb.FlowEvent_Event.analyzed => FlowEvent.analyzed(
        at: when,
        flowId: FlowId(analyzed.flowId),
        findings: List<Finding>.unmodifiable(
          analyzed.findings.map((f) => f.toDomain()),
        ),
      ),
      pb.FlowEvent_Event.held => FlowEvent.held(
        at: when,
        flowId: FlowId(held.flowId),
        deadline: held.hasDeadline() ? _dateTime(held.deadline) : when,
        queueBytes: held.queueBytes.toInt(),
        queueCount: held.queueCount,
      ),
      pb.FlowEvent_Event.decided => FlowEvent.decided(
        at: when,
        flowId: FlowId(decided.flowId),
        // An unknown kind is treated as a block: the safe reading of a
        // decision this client cannot interpret.
        kind:
            enumFromWire(DecisionKind.values, decided.kind.value) ??
            DecisionKind.block,
        source: enumFromWire(DecisionSource.values, decided.source.value),
        blockReason: enumFromWire(
          BlockReason.values,
          decided.blockReason.value,
        ),
        ruleId: decided.ruleId.isEmpty ? null : RuleId(decided.ruleId),
        note: decided.note,
      ),
      pb.FlowEvent_Event.forwarded => FlowEvent.forwarded(
        at: when,
        flowId: FlowId(forwarded.flowId),
      ),
      pb.FlowEvent_Event.responseHeaders => FlowEvent.responseHeaders(
        at: when,
        flowId: FlowId(responseHeaders.flowId),
        head: responseHeaders.head.toDomain(),
        streaming: responseHeaders.streaming,
      ),
      pb.FlowEvent_Event.responseChunk => FlowEvent.responseChunk(
        at: when,
        flowId: FlowId(responseChunk.flowId),
        bytesSoFar: responseChunk.bytesSoFar.toInt(),
      ),
      pb.FlowEvent_Event.recorded => FlowEvent.recorded(
        at: when,
        flowId: FlowId(recorded.flowId),
      ),
      pb.FlowEvent_Event.timedOut => FlowEvent.timedOut(
        at: when,
        flowId: FlowId(timedOut.flowId),
      ),
      pb.FlowEvent_Event.lagged => FlowEvent.lagged(
        at: when,
        dropped: lagged.dropped.toInt(),
      ),
      pb.FlowEvent_Event.diagnostic => FlowEvent.diagnostic(
        at: when,
        diagnostic: diagnostic.toDomain(),
      ),
      // Ein Befund, der zu genau einem Fluss gehört (Feld 16). Die Kennung
      // reist mit: Sie ist der einzige Weg, einen Befund an die Anfrage zu
      // hängen, die gerade gescheitert ist, und ein Befund, der irgendwo
      // allgemein steht, erklärt diese Anfrage nicht.
      //
      // Ein leeres Feld bleibt null. Der ganze Sinn der Kennung ist die
      // Unterscheidung „gehört zu einem Fluss" von „gehört zur Sitzung", und
      // die läuft über null; ein `FlowId('')` wäre ein Fluss, den es nicht
      // gibt, und eine Ansicht spränge später in leere Details.
      pb.FlowEvent_Event.flowDiagnostic => FlowEvent.diagnostic(
        at: when,
        diagnostic: flowDiagnostic.diagnostic.toDomain(),
        flowId: flowDiagnostic.flowId.isEmpty
            ? null
            : FlowId(flowDiagnostic.flowId),
      ),
      pb.FlowEvent_Event.rulesChanged => FlowEvent.rulesChanged(
        at: when,
        revision: rulesChanged.revision.toInt(),
      ),
      pb.FlowEvent_Event.agentAsk => FlowEvent.agentAsk(
        at: when,
        askId: agentAsk.askId,
        text: agentAsk.text,
        suggestedHost: agentAsk.suggestedHost,
        suggestedPath: agentAsk.suggestedPath,
      ),
      pb.FlowEvent_Event.failed => FlowEvent.failed(
        at: when,
        flowId: FlowId(failed.flowId),
        // An unknown error counts as a connect failure.
        error:
            enumFromWire(UpstreamError.values, failed.error.value) ??
            UpstreamError.connect,
        resolvedIp: failed.resolvedIp,
      ),
      pb.FlowEvent_Event.notSet => null,
    };
  }
}

/// `RuleMatcher` to [RuleMatcher].
extension RuleMatcherToDomain on pb.RuleMatcher {
  /// The domain form. Unknown methods are dropped from the list.
  RuleMatcher toDomain() => RuleMatcher(
    host: host,
    methods: List<Method>.unmodifiable(
      methods.map((m) => enumFromWire(Method.values, m.value)).nonNulls,
    ),
    path: path,
    scheme: enumFromWire(Scheme.values, scheme.value),
    port: port,
    upgrade: enumFromWire(Upgrade.values, upgrade.value),
  );
}

/// [RuleMatcher] to `RuleMatcher`.
extension RuleMatcherToProto on RuleMatcher {
  /// The wire form.
  pb.RuleMatcher toProto() {
    final pb.RuleMatcher out = pb.RuleMatcher()
      ..host = host
      ..path = path
      ..port = port;
    out.methods.addAll(methods.map((m) => pb.Method.valueOf(enumToWire(m))!));
    final Scheme? scheme = this.scheme;
    if (scheme != null) {
      out.scheme = pb.Scheme.valueOf(enumToWire(scheme))!;
    }
    final Upgrade? upgrade = this.upgrade;
    if (upgrade != null) {
      out.upgrade = pb.Upgrade.valueOf(enumToWire(upgrade))!;
    }
    return out;
  }
}

/// `RuleExpiry` to [RuleExpiry].
extension RuleExpiryToDomain on pb.RuleExpiry {
  /// The domain form; an unset expiry counts as the session.
  RuleExpiry toDomain() => switch (whichExpiry()) {
    pb.RuleExpiry_Expiry.never => const RuleExpiry.never(),
    pb.RuleExpiry_Expiry.session => const RuleExpiry.session(),
    pb.RuleExpiry_Expiry.at => RuleExpiry.at(at: _dateTime(at)),
    pb.RuleExpiry_Expiry.notSet => const RuleExpiry.session(),
  };
}

/// [RuleExpiry] to `RuleExpiry`.
extension RuleExpiryToProto on RuleExpiry {
  /// The wire form.
  pb.RuleExpiry toProto() => switch (this) {
    RuleExpiryNever() => pb.RuleExpiry()..never = wkt.Empty(),
    RuleExpirySession() => pb.RuleExpiry()..session = wkt.Empty(),
    RuleExpiryAt(:final at) => pb.RuleExpiry()..at = _timestamp(at),
  };
}

/// `Rule` to [Rule].
extension RuleToDomain on pb.Rule {
  /// The domain form. An unknown action counts as `ask`, the safe default.
  Rule toDomain() => Rule(
    id: ruleId.isEmpty ? null : RuleId(ruleId),
    action: enumFromWire(RuleAction.values, action.value) ?? RuleAction.ask,
    matcher: matcher.toDomain(),
    expires: expires.toDomain(),
    stream: stream,
    createdFrom: createdFromFlowId.isEmpty ? null : FlowId(createdFromFlowId),
    bundled: bundled,
    disabled: disabled,
    note: note.isEmpty ? null : note,
    createdAt: hasCreatedAt() ? _dateTime(createdAt) : null,
    position: position,
    hitCount: hitCount.toInt(),
    allowPrivate: allowPrivate,
  );
}

/// [Rule] to `Rule`.
extension RuleToProto on Rule {
  /// The wire form.
  pb.Rule toProto() {
    final pb.Rule out = pb.Rule()
      ..ruleId = id?.value ?? ''
      ..action = pb.RuleAction.valueOf(enumToWire(action))!
      ..matcher = matcher.toProto()
      ..expires = expires.toProto()
      ..stream = stream
      ..createdFromFlowId = createdFrom?.value ?? ''
      ..bundled = bundled
      // Symmetric like `bundled`: the daemon ignores the field in a request,
      // but a rule that goes through `toProto()` on its way to a dry run or
      // back into the editor must not lose what it stands for (HUM-105).
      ..disabled = disabled
      ..note = note ?? ''
      ..position = position
      ..hitCount = Int64(hitCount)
      ..allowPrivate = allowPrivate;
    final DateTime? created = createdAt;
    if (created != null) {
      out.createdAt = _timestamp(created);
    }
    return out;
  }
}

/// [EditedRequest] to `EditedRequest`.
extension EditedRequestToProto on EditedRequest {
  /// The wire form.
  pb.EditedRequest toProto() {
    final pb.EditedRequest out = pb.EditedRequest()
      ..method = pb.Method.valueOf(enumToWire(method))!
      ..methodRaw = methodRaw
      ..url = url
      ..body = body;
    out.headers.addAll(
      headers.map(
        (h) => pb.Header()
          ..name = h.name
          ..value = h.value,
      ),
    );
    return out;
  }
}

/// [Decision] to `DecideRequest`.
extension DecisionToProto on Decision {
  /// The wire form for [flowId], with the optional rule to create first.
  pb.DecideRequest toProto(FlowId flowId, {Rule? remember}) {
    final pb.DecideRequest out = pb.DecideRequest()..flowIds.add(flowId.value);
    switch (this) {
      case DecisionAllow():
        out.allow = wkt.Empty();
      case DecisionAllowEdited(:final request):
        out.allowEdited = request.toProto();
      case DecisionBlock(:final note):
        out.block = pb.DecideRequest_Block()..note = note ?? '';
      case DecisionTimedOut():
        // A client never sends a timeout; the daemon's clock does. Sending
        // one is a programming error, not a wire case.
        throw ArgumentError.value(this, 'decision', 'not sendable');
    }
    if (remember != null) {
      out.remember = remember.toProto();
    }
    return out;
  }
}

/// `SandboxState` to [SandboxState].
extension SandboxStateToDomain on pb.SandboxState {
  /// The domain form; an unknown value counts as [SandboxState.stopped].
  ///
  /// Stopped is the least harmful guess: it never claims a sandbox is up, and
  /// it leaves the start control enabled rather than a stop that stops
  /// nothing.
  SandboxState toDomain() =>
      enumFromWire(SandboxState.values, value) ?? SandboxState.stopped;
}

/// `MountMode` to [MountMode].
extension MountModeToDomain on pb.MountMode {
  /// The domain form; an unknown value counts as [MountMode.ro].
  ///
  /// A mount from a newer daemon is still a mount and belongs in the table;
  /// read-only is the narrower of the two bind modes, so an unknown one is
  /// never shown as writable.
  MountMode toDomain() => enumFromWire(MountMode.values, value) ?? MountMode.ro;
}

/// `ValueOrigin` to [ValueOrigin].
extension ValueOriginToDomain on pb.ValueOrigin {
  /// The domain form; an unknown origin counts as [ValueOrigin.profile].
  ValueOrigin toDomain() =>
      enumFromWire(ValueOrigin.values, value) ?? ValueOrigin.profile;
}

/// `Mount` to [MountEntry].
extension MountToDomain on pb.Mount {
  /// The domain form.
  MountEntry toDomain() => MountEntry(
    dst: dst,
    src: src,
    mode: mode.toDomain(),
    origin: origin.toDomain(),
    linkTarget: linkTarget,
  );
}

/// `EnvVar` to [EnvEntry].
extension EnvVarToDomain on pb.EnvVar {
  /// The domain form. A withheld value arrives empty and stays that way.
  EnvEntry toDomain() => EnvEntry(
    key: key,
    value: value,
    origin: origin.toDomain(),
    withheld: withheld,
  );
}

/// `IsolationCheck` to [IsolationCheck].
extension IsolationCheckToDomain on pb.IsolationCheck {
  /// The domain form, or null for an unknown guarantee.
  ///
  /// Null and not a default: a guarantee this build does not know is not one
  /// of the three it draws, and putting it on one of their lines would attach
  /// the wrong evidence to the wrong sentence.
  IsolationCheck? toDomain() => enumFromWire(IsolationCheck.values, value);
}

/// `CheckResult` to [IsolationCheckResult].
extension CheckResultToDomain on pb.CheckResult {
  /// The domain form, or null when the guarantee is not one of the three.
  IsolationCheckResult? toDomain() {
    // `check_1`: the generated name of field `check`, which the protobuf
    // runtime renames because `GeneratedMessage` already has a `check`.
    final IsolationCheck? which = check_1.toDomain();
    if (which == null) {
      return null;
    }
    return IsolationCheckResult(
      check: which,
      passed: passed,
      evidence: evidence,
      diagnostic: hasDiagnostic() ? diagnostic.toDomain() : null,
    );
  }
}

/// `SandboxEvent.Status` to [SandboxStatus].
extension SandboxStatusToDomain on pb.SandboxEvent_Status {
  /// The domain form, without diagnostics: those arrive as their own events
  /// and are collected by the provider that reads the stream.
  SandboxStatus toDomain() => SandboxStatus(
    state: state.toDomain(),
    sessionId: sessionId.isEmpty ? null : SessionId(sessionId),
    sandboxId: sandboxId.isEmpty ? null : SandboxId(sandboxId),
    startedAt: hasStartedAt() ? _dateTime(startedAt) : null,
    profile: profile,
    backend: backend,
    llmEndpoint: llmEndpoint,
    workDirHost: workDir.isEmpty ? null : workDir,
    workMode: WorkMode.fromWire(workMode),
    mounts: List<MountEntry>.unmodifiable(
      mounts.map((pb.Mount mount) => mount.toDomain()),
    ),
    env: List<EnvEntry>.unmodifiable(
      env.map((pb.EnvVar entry) => entry.toDomain()),
    ),
    argvPreview: argvPreview,
    agentRunning: agentRunning,
  );
}

/// [TerminalCommand] to `TerminalInput` (HUM-042).
extension TerminalCommandToProto on TerminalCommand {
  /// The wire form of one message to the terminal.
  pb.TerminalInput toProto() => switch (this) {
    final TerminalOpen open => pb.TerminalInput(
      open: pb.TerminalInput_Open(
        sandboxId: open.sandboxId,
        cols: open.cols,
        rows: open.rows,
        readOnly: open.readOnly,
      ),
    ),
    final TerminalKeys keys => pb.TerminalInput(data: keys.bytes),
    final TerminalResize resize => pb.TerminalInput(
      resize: pb.TerminalInput_Resize(cols: resize.cols, rows: resize.rows),
    ),
    TerminalDetach() => pb.TerminalInput(close: wkt.Empty()),
  };
}

/// `TerminalOutput` to [TerminalFrame] (HUM-042).
extension TerminalOutputToDomain on pb.TerminalOutput {
  /// The domain form, or null for a variant this version does not know: a
  /// newer daemon may send one, and a terminal that went blank at the first
  /// unknown frame would lose a session over a field it does not need.
  TerminalFrame? toDomain() => switch (whichOutput()) {
    pb.TerminalOutput_Output.data => TerminalOutput(Uint8List.fromList(data)),
    pb.TerminalOutput_Output.resize => TerminalGeometry(
      cols: resize.cols,
      rows: resize.rows,
    ),
    pb.TerminalOutput_Output.diagnostic => TerminalFinding(
      diagnostic.toDomain(),
    ),
    pb.TerminalOutput_Output.exit => TerminalExit(exit.code),
    pb.TerminalOutput_Output.notSet => null,
  };
}
