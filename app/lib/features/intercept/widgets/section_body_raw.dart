/// The body section of the request card: the raw preview, selectable, never
/// interpreted.
///
/// Version 1 shows what `FlowDetail.body_preview` carries and nothing else; a
/// JSON tree and a diff arrive with HUM-030. Long lines are wrapped and the
/// text is capped, because a `SelectableText` with one unwrapped line of a
/// megabyte lays out for seconds (HUM-020 Fallstricke).
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;

import '../../../core/domain/domain.dart';
import '../../../core/ui/h_collapsible.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../format.dart';
import 'selectable_mono_text.dart';

/// How much of a body the card renders.
const int bodyPreviewLimit = 64 * 1024;

/// The collapsible body section.
class SectionBodyRaw extends StatelessWidget {
  /// Creates the section for [preview]; [body] describes what the daemon
  /// holds, which can be more than the preview shows.
  const SectionBodyRaw({required this.preview, this.body, super.key});

  /// The body as text, already lossy UTF-8 from the daemon.
  final String preview;

  /// Size, type and digest of the whole body, when the detail carries it.
  final BodyRef? body;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final BodyRef? body = this.body;
    final int size = body?.size ?? preview.length;
    final String contentType = body == null || body.contentType.isEmpty
        ? l10n.interceptContentTypeUnknown
        : body.contentType;
    final bool truncated =
        preview.length > bodyPreviewLimit ||
        (body != null && (body.truncated || body.size > preview.length));
    final String text = preview.length > bodyPreviewLimit
        ? preview.substring(0, bodyPreviewLimit)
        : preview;
    return HCollapsible(
      title: l10n.interceptSectionBody(formatBytes(size), contentType),
      child: text.isEmpty
          ? Text(
              l10n.interceptBodyEmpty,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                if (truncated) ...<Widget>[
                  Text(
                    l10n.interceptBodyTruncated,
                    style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                  ),
                  SizedBox(height: tokens.spacing.x1),
                ],
                SelectableMonoText(
                  text: text,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                ),
              ],
            ),
    );
  }
}
