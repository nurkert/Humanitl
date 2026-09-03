/// Enumerations of the flow lifecycle, mirrors of `humanitl-core` and of the
/// enums in `proto/humanitl/v1/humanitl.proto`.
///
/// Unlike the wire enums these have no `unspecified` member: an absent value
/// is `null` on the optional fields that carry it, and `core/ipc/convert.dart`
/// is the one place that knows about the zero value.
library;

/// Where a flow is in its lifecycle (CONVENTIONS 3.2).
enum FlowState {
  /// The request arrived at the proxy.
  received,

  /// The detectors have run.
  analyzed,

  /// Waiting for a human decision.
  held,

  /// A decision was made.
  decided,

  /// The request went out to the upstream.
  forwarded,

  /// The response came back.
  responded,

  /// Persisted by the recorder.
  recorded,

  /// An allowed request did not reach its target (see [UpstreamError]).
  failed;

  /// True while the flow waits for a decision.
  bool get isHeld => this == FlowState.held;

  /// True once nothing about the flow can change any more.
  bool get isTerminal => this == FlowState.recorded;
}

/// What kind of decision was taken.
enum DecisionKind {
  /// Allowed unchanged.
  allow,

  /// Allowed after the human edited the request.
  allowEdited,

  /// Blocked.
  block,

  /// The hold budget ran out.
  timedOut,
}

/// Why a request was blocked. The HTTP status per reason is in CONVENTIONS 3.2.
enum BlockReason {
  /// The human said no.
  user,

  /// A rule said no.
  rule,

  /// Nobody decided in time.
  timeout,

  /// The body exceeded the hold cap.
  bodyCap,

  /// The TLS authority did not match the request.
  authorityMismatch,

  /// No route to the target.
  noRoute,

  /// The hold buffer is full.
  holdMemory,

  /// Too many flows are held.
  holdMaxFlows,

  /// The client went away while the request was held.
  clientTimeout,

  /// The target resolved to a private address without `allow_private`.
  privateAddress,

  /// A secret with a matching checksum was found in the request and
  /// `hold.hard_block_checksum_secrets` is on. Nobody was asked, so this is
  /// not `user`: an answer naming a human who never decided would be untrue
  /// (`backlog/CONVENTIONS.md` 4.13).
  secret,
}

/// Who decided.
enum DecisionSource {
  /// The human in the UI or on the terminal.
  user,

  /// A rule.
  rule,

  /// The hold budget.
  timeout,

  /// The LLM passthrough.
  passthrough,

  /// The daemon itself, always a refusal (caps, budgets).
  system,
}

/// Why an allowed request never reached its target (CONVENTIONS 4.10).
enum UpstreamError {
  /// Name resolution failed.
  dns,

  /// The TCP connection failed.
  connect,

  /// The TLS handshake failed.
  tls,

  /// The name resolved to a private address.
  privateAddress,

  /// The upstream did not answer in time.
  timeout,
}
