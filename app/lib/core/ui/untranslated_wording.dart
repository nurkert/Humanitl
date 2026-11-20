/// A sentence that came in English, from the daemon or the platform, folded
/// behind the hint that it is untranslated (HUM-569).
///
/// Der Daemon schreibt den Satz eines Befunds (`why`) Englisch, und die
/// Plattform ihre Fehlertexte auch. In einer Oberfläche in einer anderen
/// Sprache steht im Grund-Platz deshalb der übersetzte Grundsatz
/// ([DiagnosticText.cause]), und der Satz, der Pfad, Host oder Fehlertext
/// nennt, bleibt hier erreichbar: zugeklappt, unter einer Kopfzeile, die sagt,
/// dass er unübersetzt ist. So steht in der deutschen Oberfläche kein
/// englischer Satz ohne Hinweis, und keine Angabe des Daemons geht verloren.
library;

import 'package:flutter/widgets.dart';

import '../../l10n/l10n.dart';
import '../domain/domain.dart';
import 'h_collapsible.dart';
import 'hover_label.dart';
import 'ui.dart';

/// The folded sentence.
class UntranslatedWording extends StatelessWidget {
  /// Creates the fold around [sentence].
  const UntranslatedWording({required this.sentence, super.key});

  /// The fold of [text], or `null` when its cause already is the daemon's
  /// sentence (or there is none).
  static Widget? of(DiagnosticText text, {Key? key}) {
    final String? original = text.original;
    return original == null
        ? null
        : UntranslatedWording(sentence: original, key: key);
  }

  /// The fold for a card of [diagnostic] whose cause slot shows
  /// `DiagnosticL10n.resolve(diagnostic, l10n).cause`, or `null`.
  static Widget? forDiagnostic(
    Diagnostic diagnostic,
    AppLocalizations l10n, {
    Key? key,
  }) => of(DiagnosticL10n.resolve(diagnostic, l10n), key: key);

  /// Where a raw [sentence] goes that a card used to show as its mono
  /// detail line: still there in an interface that speaks
  /// [daemonLanguage], folded otherwise. Either half is `null`.
  static (String?, Widget?) split(
    AppLocalizations l10n,
    String? sentence, {
    Key? key,
  }) => sentence == null || sentence.isEmpty || speaksDaemonLanguage(l10n)
      ? (sentence, null)
      : (null, UntranslatedWording(sentence: sentence, key: key));

  /// The sentence, as it came.
  final String sentence;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return HCollapsible(
      title: context.l10n.diagnosticOriginalWording,
      initiallyOpen: false,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          borderRadius: BorderRadius.circular(tokens.radii.control),
          border: Border.all(color: tokens.colors.line),
        ),
        child: Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x2,
          ),
          child: SizedBox(
            width: double.infinity,
            child: Text(
              sentence,
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            ),
          ),
        ),
      ),
    );
  }
}

/// The cause of [diagnostic] as one line of text, with the daemon's sentence
/// folded under it where the line is a translation (HUM-569).
///
/// For the places that show a finding inline, under a control, and not as a
/// card (`docs/UX.md` 4.4).
class DiagnosticCause extends StatelessWidget {
  /// Creates the line of [diagnostic].
  const DiagnosticCause({
    required this.diagnostic,
    required this.style,
    this.textKey,
    this.compact = false,
    super.key,
  });

  /// The finding.
  final Diagnostic diagnostic;

  /// The style of the line.
  final TextStyle style;

  /// The key of the line itself, for a test that reads it.
  final Key? textKey;

  /// For a dense list of short reasons (one per sandbox backend): a fold
  /// under every line would push the list apart, and the generic cause of a
  /// code is a card's explanation, longer and less exact than the measured
  /// sentence (for example, that the program of a sandbox backend was not
  /// found on PATH). The line keeps the daemon's sentence behind the hint
  /// that it is untranslated, and the translated
  /// generic cause goes into the hover label and the screen-reader text.
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final DiagnosticText text = DiagnosticL10n.resolve(
      diagnostic,
      context.l10n,
    );
    final Widget line = Text(text.cause, key: textKey, style: style);
    final Widget? original = UntranslatedWording.of(text);
    if (original == null) {
      return line;
    }
    if (compact) {
      return HoverLabel(
        label: text.cause,
        child: Semantics(
          tooltip: text.cause,
          child: Text(
            context.l10n.diagnosticUntranslated(text.original ?? ''),
            key: textKey,
            style: style,
          ),
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[line, original],
    );
  }
}
