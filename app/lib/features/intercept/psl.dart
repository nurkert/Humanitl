/// The registrable domain of a host, from a short bundled table.
///
/// A stand-in for the public suffix list: HUM-031 replaces it with the domain
/// catalog, which carries the real list. Until then the table covers the
/// suffixes a development machine actually meets; everything else falls back
/// to the last two labels, which is right for every single-label suffix
/// (`com`, `dev`, `io`, `de`).
library;

/// Suffixes of two labels under which registrations happen.
const Set<String> multiLabelSuffixes = <String>{
  'co.uk',
  'org.uk',
  'me.uk',
  'ac.uk',
  'gov.uk',
  'co.jp',
  'or.jp',
  'ne.jp',
  'co.kr',
  'com.au',
  'net.au',
  'org.au',
  'com.br',
  'com.cn',
  'com.mx',
  'co.nz',
  'co.za',
  'co.in',
  'com.tr',
  'com.sg',
  'github.io',
  'gitlab.io',
  'pages.dev',
  'workers.dev',
  'vercel.app',
  'netlify.app',
  's3.amazonaws.com',
};

/// The registrable domain of [host]: `api.github.com` becomes `github.com`,
/// `foo.bar.co.uk` becomes `bar.co.uk`.
///
/// [isIpLiteral] hosts are returned unchanged; an address has no apex.
String registrableDomain(String host, {bool isIpLiteral = false}) {
  if (isIpLiteral || host.isEmpty) {
    return host;
  }
  final List<String> labels = host
      .split('.')
      .where((String label) => label.isNotEmpty)
      .toList();
  if (labels.length <= 2) {
    return labels.join('.');
  }
  final String lastTwo = labels.sublist(labels.length - 2).join('.');
  if (multiLabelSuffixes.contains(lastTwo)) {
    return labels.sublist(labels.length - 3).join('.');
  }
  return lastTwo;
}
