/// Das Banner über einer eingefrorenen Warteschlange (HUM-044).
///
/// Es erscheint genau dann, wenn eine Verbindung, die stand, gebrochen ist.
/// Der Setup-Bildschirm übernimmt in diesem Fall nicht: Wer zwölf wartende
/// Anfragen auf dem Schirm hat, verlöre sonst den Bildschirm und erführe
/// nicht, was mit dem Agenten passiert ist (`docs/UX.md` 4.2, Fall 4).
///
/// Das Banner trägt die drei Dinge, die dieser Fall verlangt: den Grund aus
/// dem `Diagnostic`, die Folge für den Agenten und **eine** Aktion, „Erneut
/// verbinden". Kein Schließen: Der Zustand geht nicht weg, weil jemand das
/// Banner wegklickt, und ein Banner, das man schließen kann, ließe einen
/// stehengebliebenen Bildschirm ohne Erklärung zurück.
///
/// Solange eine Verbindung, die eben noch stand, in ihrer Frist ist
/// (`ConnectionFrozen.settling`, HUM-385), sagt das Banner ruhig, dass das
/// Fenster neu verbindet: ohne Warnfarbe, ohne den Satz des Transports und
/// ohne Knopf, denn der nächste Versuch läuft schon. Die Folge für die
/// Warteschlange steht trotzdem da; sie gilt auch für zwei Sekunden. Endet die
/// Frist ohne Antwort, wird daraus das Banner von oben, mit Grund und Knopf.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// Das Banner.
class FrozenBanner extends StatelessWidget {
  /// Zeigt [diagnostic] als Grund und ruft [onReconnect] für den einen Knopf.
  const FrozenBanner({
    required this.diagnostic,
    required this.onReconnect,
    this.settling = false,
    super.key,
  });

  /// True while the window takes a connection that stood a moment ago up
  /// again, within its grace (HUM-385).
  final bool settling;

  /// Warum die Verbindung weg ist. Der Text kommt aus dem Transport oder aus
  /// dem Daemon und wird hier nicht ersetzt (`docs/UX.md` 4.4).
  final Diagnostic diagnostic;

  /// Versucht die Verbindung erneut.
  final VoidCallback onReconnect;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Color tone = settling
        ? tokens.colors.fg1
        : tokens.stateTextColor(HFlowState.error);
    final String title = settling
        ? l10n.shellReconnectingTitle
        : l10n.shellFrozenTitle;
    return Semantics(
      container: true,
      label: '$title. ${l10n.shellFrozenConsequence}',
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: settling
              ? tokens.colors.bg1
              : HColorDerivation.tint(tokens.stateColor(HFlowState.error)),
          border: Border(bottom: BorderSide(color: tokens.colors.line)),
        ),
        child: Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x2,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              HGlyphIcon(
                settling ? HGlyph.hourglass : HGlyph.triangleAlert,
                size: HSize.glyph,
                color: tone,
              ),
              SizedBox(width: tokens.spacing.x2),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Text(
                      title,
                      key: Key(
                        settling
                            ? 'shell-reconnecting-title'
                            : 'shell-frozen-title',
                      ),
                      style: tokens.typography.ui13.medium.tinted(
                        tokens.colors.fg0,
                      ),
                    ),
                    Text(
                      l10n.shellFrozenConsequence,
                      key: const Key('shell-frozen-consequence'),
                      style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                    ),
                    if (!settling && diagnostic.why.isNotEmpty)
                      Text(
                        diagnostic.why,
                        key: const Key('shell-frozen-why'),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: tokens.typography.mono12.tinted(
                          tokens.colors.fg1,
                        ),
                      ),
                  ],
                ),
              ),
              if (!settling) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                HButton(
                  key: const Key('shell-frozen-reconnect'),
                  onPressed: onReconnect,
                  child: Text(l10n.shellFrozenReconnect),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
