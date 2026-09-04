/// One recorded flow as a `curl` command line, for a single flow only.
///
/// The body is not inlined. A recorded body can be megabytes and can carry
/// bytes no shell quotes safely, so it goes into a file next to the command
/// and the command reads it with `--data-binary @<file>`. Whoever exports is
/// told about both files (`backlog/sprint-2.md`, HUM-032).
library;

import '../../../core/domain/domain.dart';
import 'export_entry.dart';

/// The name of the file the command reads the body from.
const String curlBodyFileName = 'request.body';

/// [entry] as a `curl` invocation.
///
/// Header order is the recorded one. `--data-binary` is used rather than
/// `--data`, because `--data` strips newlines and would send something other
/// than what was recorded.
String encodeCurl(
  HistoryExportEntry entry, {
  String bodyFile = curlBodyFileName,
}) {
  final Flow flow = entry.flow;
  final HttpRequest? request =
      entry.detail.editedRequest ?? entry.detail.request;
  final String pathAndQuery = request?.pathAndQuery ?? flow.path;
  final String url =
      '${flow.scheme.name}://${flow.authority.display(flow.scheme)}$pathAndQuery';
  final StringBuffer out = StringBuffer()
    ..write('curl -X ')
    ..write(flow.methodLabel)
    ..write(' ')
    ..write(shellQuote(url));
  for (final Header header in request?.headers ?? const <Header>[]) {
    out
      ..write(' \\\n  -H ')
      ..write(shellQuote('${header.name}: ${header.text}'));
  }
  final int bodySize = entry.requestBody?.length ?? 0;
  if (bodySize > 0) {
    out
      ..write(' \\\n  --data-binary @')
      ..write(shellQuote(bodyFile));
  }
  return out.toString();
}

/// [value] in single quotes, safe for `sh`.
///
/// A single quote cannot appear inside single quotes, so it is closed,
/// escaped and reopened -- the standard `'\''` dance. Nothing else needs
/// escaping inside single quotes, which is why they are used and not double
/// ones.
String shellQuote(String value) => "'${value.replaceAll("'", r"'\''")}'";
