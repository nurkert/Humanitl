/// Das Abzeichen „WebSocket" und der Satz darüber, was eine Freigabe danach
/// durchlässt (HUM-058, Zeile 11).
///
/// Ein Widget für zwei Orte: die Karte der Warteschlange und den Editor, der
/// mit `E` aufgehen kann, bevor die Karte ihre Zeile zeigt. Beide sagen
/// dasselbe mit denselben Worten.
library;

import 'package:flutter/widgets.dart';

import '../../l10n/l10n.dart';
import 'ui.dart';

/// The notice for a request that opens a WebSocket connection to [host].
class WebSocketNotice extends StatelessWidget {
  /// Creates the notice.
  const WebSocketNotice({required this.host, super.key});

  /// The host the connection goes to.
  final String host;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // `Wrap` statt `Row`: bei doppelter Textgröße rutscht der Satz unter das
    // Abzeichen, statt abgeschnitten zu werden (`docs/UX.md` 6).
    return Wrap(
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: tokens.spacing.x2,
      runSpacing: tokens.spacing.x1,
      children: <Widget>[
        // Violett ist die Farbe des Kanals, der weiterläuft, ohne dass
        // jemand jede Nachricht sieht -- dieselbe wie die der Durchreiche zum
        // Modell (`BACKLOG.md` 5).
        HBadge(
          text: l10n.interceptWebSocketChip,
          color: tokens.state.passthroughLlm,
        ),
        Text(
          l10n.interceptWebSocketHint(host),
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        ),
      ],
    );
  }
}
