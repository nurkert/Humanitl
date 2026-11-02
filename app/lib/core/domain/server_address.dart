/// What an address of a model server says by itself, read from the text and
/// nothing else (HUM-414): where it points, the one origin a key goes to,
/// whether two addresses share it, how it is shown, and a name for a new
/// server. The settings and the question before a start share these.
///
/// None of this goes to the network or asks the daemon; it is a reading
/// aid beside the field. The daemon decides what is private and where a key
/// goes (`humanitl_config::servers::same_origin`, HUM-069).
library;

/// Where an address points.
enum ServerPlaceKind {
  /// Loopback: this machine.
  thisMachine,

  /// A private or link-local address, or a name of the local network.
  localNetwork,

  /// Anything else.
  remote,
}

/// Where an address points, and whether traffic to it goes unencrypted
/// beyond the local network.

final class ServerPlace {
  /// A place of [kind] for [host].
  const ServerPlace({
    required this.kind,
    required this.host,
    required this.plain,
  });

  /// Where it points.
  final ServerPlaceKind kind;

  /// The host as it stands in the address.
  final String host;

  /// True for `http`.
  final bool plain;

  /// True when prompts and key would cross the internet unencrypted: `http`
  /// to a host outside this machine and the local network.
  bool get plainOverInternet => plain && kind == ServerPlaceKind.remote;
}

/// Where [endpoint] points, or null when it names no host yet.
ServerPlace? serverPlace(String endpoint) {
  final Uri? uri = Uri.tryParse(endpoint.trim());
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) {
    return null;
  }
  final String host = uri.host.toLowerCase();
  return ServerPlace(
    kind: _kindOf(host),
    host: uri.host,
    plain: uri.scheme.toLowerCase() == 'http',
  );
}

ServerPlaceKind _kindOf(String host) {
  if (host == 'localhost' ||
      host.endsWith('.localhost') ||
      host == '::1' ||
      host == '[::1]' ||
      host.startsWith('127.')) {
    return ServerPlaceKind.thisMachine;
  }
  final List<int>? v4 = _ipv4(host);
  if (v4 != null) {
    final int a = v4[0];
    final int b = v4[1];
    final bool private =
        a == 10 ||
        (a == 172 && b >= 16 && b <= 31) ||
        (a == 192 && b == 168) ||
        (a == 169 && b == 254) ||
        (a == 100 && b >= 64 && b <= 127);
    return private ? ServerPlaceKind.localNetwork : ServerPlaceKind.remote;
  }
  final String bare = host.replaceAll('[', '').replaceAll(']', '');
  if (bare.contains(':')) {
    final bool private =
        bare.startsWith('fc') ||
        bare.startsWith('fd') ||
        bare.startsWith('fe80');
    return private ? ServerPlaceKind.localNetwork : ServerPlaceKind.remote;
  }
  if (!host.contains('.') ||
      host.endsWith('.local') ||
      host.endsWith('.lan') ||
      host.endsWith('.home.arpa') ||
      host.endsWith('.internal')) {
    return ServerPlaceKind.localNetwork;
  }
  return ServerPlaceKind.remote;
}

List<int>? _ipv4(String host) {
  final List<String> parts = host.split('.');
  if (parts.length != 4) {
    return null;
  }
  final List<int> numbers = <int>[];
  for (final String part in parts) {
    final int? n = int.tryParse(part);
    if (n == null || n < 0 || n > 255) {
      return null;
    }
    numbers.add(n);
  }
  return numbers;
}

/// The origin of [endpoint] (scheme, host and port, the default port left
/// out), or empty when it names no host.
String serverOrigin(String endpoint) {
  final Uri? uri = Uri.tryParse(endpoint.trim());
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) {
    return '';
  }
  return Uri(
    scheme: uri.scheme.toLowerCase(),
    host: uri.host.toLowerCase(),
    port: uri.hasPort ? uri.port : null,
  ).toString();
}

/// The labels a host often starts with that say nothing about the server.
const Set<String> _noise = <String>{'api', 'www', 'llm', 'ai', 'inference'};

/// A name for a new server from its address: `local` for this machine, the
/// telling part of a host name, the address with dashes for an IP. Empty
/// when the address names no host. Only a proposal; the daemon checks the
/// name on Save (1 to 32 of `a-z`, `0-9`, `-`, `_`).
String proposeServerName(String endpoint) {
  final ServerPlace? place = serverPlace(endpoint);
  if (place == null) {
    return '';
  }
  if (place.kind == ServerPlaceKind.thisMachine) {
    return 'local';
  }
  final String host = place.host.toLowerCase();
  final String raw;
  if (_ipv4(host) != null || host.contains(':')) {
    raw = host;
  } else {
    final List<String> labels = host.split('.');
    final List<String> telling = <String>[
      for (int i = 0; i < labels.length - (labels.length > 1 ? 1 : 0); i++)
        if (!_noise.contains(labels[i])) labels[i],
    ];
    raw = telling.isEmpty ? labels.first : telling.first;
  }
  final String name = raw
      .replaceAll(RegExp('[^a-z0-9_-]+'), '-')
      .replaceAll(RegExp('-+'), '-')
      .replaceAll(RegExp(r'^-|-$'), '');
  return name.length > 32 ? name.substring(0, 32) : name;
}

/// Whether two addresses share their origin: scheme, host and port with
/// the default ports of `http` and `https`. Only to warn before Save; the
/// daemon decides (`humanitl_config::servers::same_origin`).
bool sameOrigin(String a, String b) {
  final Uri? left = Uri.tryParse(a);
  final Uri? right = Uri.tryParse(b);
  if (left == null || right == null || left.host.isEmpty || !left.hasScheme) {
    return false;
  }
  return left.scheme.toLowerCase() == right.scheme.toLowerCase() &&
      left.host.toLowerCase() == right.host.toLowerCase() &&
      left.port == right.port;
}

/// The endpoint without user, password, query and fragment.
String displayEndpoint(String endpoint) {
  final Uri? uri = Uri.tryParse(endpoint);
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) {
    return endpoint;
  }
  return Uri(
    scheme: uri.scheme,
    host: uri.host,
    port: uri.hasPort ? uri.port : null,
    path: uri.path,
  ).toString();
}

/// The host and port of [endpoint], without scheme, user or path.
String serverHost(String endpoint) {
  final Uri? uri = Uri.tryParse(endpoint);
  if (uri == null || uri.host.isEmpty) {
    return endpoint;
  }
  // Ohne eigene Angabe der Standardport des Schemas (80, 443): Die Zeile
  // nennt immer, wohin genau es geht.
  return uri.port == 0 ? uri.host : '${uri.host}:${uri.port}';
}
