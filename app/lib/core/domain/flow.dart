/// Flows as the queue and the history see them: the row (`Flow`), the detail
/// (`FlowDetail`), a page of rows and the filter that selects them. Mirrors
/// of `FlowSummary`, `FlowDetail`, `FlowPage` and `ListFlowsRequest`.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'diagnostic.dart';
import 'flow_state.dart';
import 'http.dart';
import 'ids.dart';

part 'flow.freezed.dart';

/// How sure a finding is.
enum FindingTier {
  /// The value passed a checksum; this is a secret.
  checksum,

  /// A pattern matched.
  regex,

  /// A term the user listed.
  userTerm,
}

/// Where in the request a finding sits.
enum FindingLocation {
  /// In a header; see `headerName`.
  header,

  /// In the query string.
  query,

  /// In the body.
  body,
}

/// A possible secret in the request.
@freezed
abstract class Finding with _$Finding {
  /// Creates a finding.
  const factory Finding({
    required String kind,
    required FindingLocation location,
    @Default('') String headerName,
    required int spanStart,
    required int spanEnd,
    required FindingTier tier,
    @Default(<int>[]) List<int> valueHash,
    @Default('') String displayPrefix,
    @Default(false) bool resolved,
  }) = _Finding;
}

/// What the catalog knows about the target domain (HUM-031).
@freezed
abstract class DomainInfo with _$DomainInfo {
  /// Creates domain information.
  const factory DomainInfo({
    @Default('') String apex,
    @Default('') String catalogId,
    @Default(0) int trancoRank,
    DateTime? firstSeen,
    @Default(0) int seenCount,
  }) = _DomainInfo;

  const DomainInfo._();

  /// True when the catalog has an entry for the domain.
  bool get inCatalog => catalogId.isNotEmpty;
}

/// One flow as a row: everything the queue and the history list need.
@freezed
abstract class Flow with _$Flow {
  /// Creates a flow row.
  const factory Flow({
    required FlowId id,
    required SessionId sessionId,
    required DateTime receivedAt,
    required Method method,
    @Default('') String methodRaw,
    required Scheme scheme,
    required Authority authority,
    required String path,
    required FlowState state,
    DecisionKind? decision,
    DecisionSource? decisionSource,
    BlockReason? blockReason,
    RuleId? ruleId,
    @Default(0) int status,
    @Default(0) int requestSize,
    @Default(0) int responseSize,
    Duration? duration,
    @Default(0) int findingCount,
    @Default(false) bool edited,
    @Default(false) bool passthrough,

    /// True for a request to the reserved name `humanitl.internal` that the
    /// proxy answered itself (HUM-073, HUM-103).
    ///
    /// It stands next to [decision], not inside it: nobody decided about a
    /// meta request, it went nowhere, and [decision] stays null. No count over
    /// decisions ever includes one; the filter term `meta:true` finds them and
    /// `meta:false` excludes them.
    @Default(false) bool meta,
    DateTime? deadline,
    @Default('') String originTool,
    UpstreamError? upstreamError,

    /// When the daemon started holding the flow (the `Held` event); client
    /// side, drives the countdown ring together with [deadline].
    DateTime? heldAt,

    /// When the client learned of the decision; client side, keeps the row
    /// in the queue for a moment so the outcome can be seen.
    DateTime? decidedAt,
  }) = _Flow;

  const Flow._();

  /// The full URL of the request, as the card shows it.
  String get url => '${scheme.name}://${authority.display(scheme)}$path';

  /// Time left until [deadline] at [now], never negative; zero without a
  /// deadline.
  Duration remainingAt(DateTime now) {
    final DateTime? deadline = this.deadline;
    if (deadline == null) {
      return Duration.zero;
    }
    final Duration left = deadline.difference(now);
    return left.isNegative ? Duration.zero : left;
  }

  /// The whole hold budget: from [heldAt] (or [receivedAt]) to [deadline];
  /// zero without a deadline.
  Duration get holdBudget {
    final DateTime? deadline = this.deadline;
    if (deadline == null) {
      return Duration.zero;
    }
    final Duration budget = deadline.difference(heldAt ?? receivedAt);
    return budget.isNegative ? Duration.zero : budget;
  }

  /// How long the flow has been waiting at [now]; zero before it was held.
  Duration heldFor(DateTime now) {
    final DateTime? since = heldAt;
    if (since == null) {
      return Duration.zero;
    }
    final Duration waited = now.difference(since);
    return waited.isNegative ? Duration.zero : waited;
  }

  /// True once the flow was decided, by whoever or whatever.
  bool get isDecided => decision != null;

  /// The method token to show.
  String get methodLabel => method.display(methodRaw);

  /// The host to show.
  String get host => authority.shownHost;

  /// True while the flow waits for a decision.
  bool get isHeld => state.isHeld;
}

/// Everything the detail pane shows about one flow.
@freezed
abstract class FlowDetail with _$FlowDetail {
  /// Creates a flow detail.
  const factory FlowDetail({
    required Flow summary,
    HttpRequest? request,
    HttpRequest? editedRequest,
    HttpResponseHead? response,
    BodyRef? responseBody,
    @Default(<Finding>[]) List<Finding> findings,
    @Default(<Diagnostic>[]) List<Diagnostic> diagnostics,
    DomainInfo? domain,
    @Default('') String bodyPreview,
  }) = _FlowDetail;
}

/// One page of the flow history.
@freezed
abstract class FlowPage with _$FlowPage {
  /// Creates a page.
  const factory FlowPage({
    @Default(<Flow>[]) List<Flow> flows,
    @Default('') String nextCursor,
    @Default(0) int total,

    /// True when [total] is only a lower bound.
    ///
    /// The recorder stops counting at its ceiling so that a long history does
    /// not block a query; from there on `total` means "at least this many"
    /// and the surface has to say so (`backlog/CONVENTIONS.md` 4.13 and
    /// 4.14). The daemon fills the flag from `FlowPage::capped`, so nobody
    /// has to guess it from the value.
    @Default(false) bool capped,
  }) = _FlowPage;

  const FlowPage._();

  /// True when another page follows.
  bool get hasMore => nextCursor.isNotEmpty;

  /// [total] as text, with a `+` where it is only a lower bound.
  ///
  /// The counterpart of `FlowPage::total_text` in the recorder. The digits
  /// are grouped by the caller, which knows the language; this is the shape.
  String totalText(String grouped) => capped ? '$grouped+' : grouped;
}

/// What `ListFlows` should return.
@freezed
abstract class FlowFilter with _$FlowFilter {
  /// Creates a filter. [query] uses the history syntax, for example
  /// `host:github.com state:blocked`.
  const factory FlowFilter({
    @Default('') String query,
    FlowId? since,
    @Default('') String orderBy,
    @Default(false) bool includePassthrough,
  }) = _FlowFilter;

  /// Everything, newest first.
  static const FlowFilter all = FlowFilter();
}
