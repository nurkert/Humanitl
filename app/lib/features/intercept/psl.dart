/// The registrable domain of a host, from a short bundled table.
///
/// A stand-in for the public suffix list: HUM-031 replaces it with the domain
/// catalog, which carries the real list. Until then the table covers the
/// suffixes a development machine actually meets; everything else falls back
/// to the last two labels, which is right for every single-label suffix
/// (`com`, `dev`, `io`, `de`).
///
/// **This answer never becomes a rule.** It groups the queue, and that is all:
/// a group is a way of reading the list, and a wrong guess costs a line in the
/// wrong place. A rule takes its domain from the daemon's catalog
/// (`selectedApexProvider`); a guessed domain in a rule would be a claim
/// nobody checked (`backlog/CONVENTIONS.md` 4.13).
library;

/// Suffixes of two or three labels under which registrations happen.
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

/// How many labels the suffixes of [multiLabelSuffixes] have, longest first.
///
/// Derived from the table instead of written down: a new three-label entry
/// beside a two-label one would otherwise be shadowed by the shorter match,
/// and the grouping would change without anybody noticing.
List<int> suffixLengths(Set<String> suffixes) =>
    <int>{for (final String suffix in suffixes) suffix.split('.').length}
        .toList()
      ..sort((int a, int b) => b.compareTo(a));

/// The registrable domain of [host]: `api.github.com` becomes `github.com`,
/// `foo.bar.co.uk` becomes `bar.co.uk`.
///
/// [isIpLiteral] hosts are returned unchanged; an address has no apex.
/// [suffixes] is the table to read; tests pass their own.
String registrableDomain(
  String host, {
  bool isIpLiteral = false,
  Set<String> suffixes = multiLabelSuffixes,
}) {
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
  // The longest suffix wins, so a three-label entry is tried before a
  // two-label one: without that order `s3.amazonaws.com` never matches and two
  // strangers' buckets group under `amazonaws.com`.
  for (final int length in suffixLengths(suffixes)) {
    if (labels.length <= length) {
      continue;
    }
    final String suffix = labels.sublist(labels.length - length).join('.');
    if (suffixes.contains(suffix)) {
      return labels.sublist(labels.length - length - 1).join('.');
    }
  }
  return labels.sublist(labels.length - 2).join('.');
}
