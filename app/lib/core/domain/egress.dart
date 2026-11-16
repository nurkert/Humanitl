/// The decision proxy of the person behind Humanitl (HUM-511, ADR-0022).
///
/// Plain value types: both are read-only views of an answer of the daemon.
/// Neither carries the credentials; the daemon never sends them back.
library;

import 'diagnostic.dart';

/// What `Egress` is asked to do.
enum EgressOp {
  /// Read the state.
  status,

  /// Open a tunnel through the proxy to a fixed target and the TLS
  /// handshake in it; sends no request.
  check,

  /// Store `USER:PASSWORD` in the keyring and set `egress.proxy_auth_ref`.
  setAuth,

  /// Remove the credentials and `egress.proxy_auth_ref`.
  removeAuth,
}

/// Who resolves the target's name (`egress.proxy_resolve`).
abstract final class EgressResolve {
  /// The decision proxy, after its own decision. The default once
  /// `egress.proxy` is set.
  static const String proxy = 'proxy';

  /// Humanitl, after its decision, with the check for private addresses.
  static const String humanitl = 'humanitl';

  /// Both, in the order the form shows them.
  static const List<String> all = <String>[proxy, humanitl];
}

/// A way out for the agent's traffic, mirror of `EgressRoute`.
final class EgressRoute {
  /// Creates a route.
  const EgressRoute({this.proxy = '', this.resolve = EgressResolve.humanitl});

  /// Direct, Humanitl resolves.
  static const EgressRoute direct = EgressRoute();

  /// `host:port` of the decision proxy; empty means direct.
  final String proxy;

  /// Who resolves: [EgressResolve.proxy] or [EgressResolve.humanitl].
  final String resolve;

  /// Whether requests leave directly.
  bool get isDirect => proxy.isEmpty;

  /// Whether the decision proxy resolves the names.
  bool get proxyResolves => !isDirect && resolve == EgressResolve.proxy;

  @override
  bool operator ==(Object other) =>
      other is EgressRoute && other.proxy == proxy && other.resolve == resolve;

  @override
  int get hashCode => Object.hash(proxy, resolve);
}

/// The state of the decision proxy, mirror of `EgressStatus`.
final class EgressStatus {
  /// Creates a state.
  const EgressStatus({
    this.configured = EgressRoute.direct,
    this.active = EgressRoute.direct,
    this.proxyCa = '',
    this.authSet = false,
    this.authRef = '',
    this.checkTarget = '',
    this.reachable = false,
    this.diagnostics = const <Diagnostic>[],
  });

  /// Nothing configured, nothing to say.
  static const EgressStatus none = EgressStatus();

  /// The route `config.toml` names now.
  final EgressRoute configured;

  /// The route the running daemon takes since its start.
  final EgressRoute active;

  /// `egress.proxy_ca`; empty without.
  final String proxyCa;

  /// Whether credentials are stored.
  final bool authSet;

  /// `egress.proxy_auth_ref`; empty without.
  final String authRef;

  /// After a check: the fixed target, `example.com:443`; empty otherwise.
  final String checkTarget;

  /// After a check: whether tunnel and TLS handshake succeeded.
  final bool reachable;

  /// What the daemon had to say: the finding of a failed check first.
  final List<Diagnostic> diagnostics;

  /// Whether a check ran.
  bool get checked => checkTarget.isNotEmpty;

  /// Whether the file names another way than the running daemon takes; it
  /// takes effect after a restart.
  bool get restartPending => configured != active;
}
