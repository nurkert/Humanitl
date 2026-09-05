/// The control for a [FixAction]: a button for what the client can do today
/// (copy a command, a link or an `export` line), a chip naming the action for
/// what HUM-044 and HUM-069 wire up later.
///
/// Lives in `core/ui` because several screens show diagnostics -- the setup
/// screen, the sandbox, the tray notice, the action bar and the diagnostic
/// strip of the intercept screen -- and no feature imports another one
/// (ARCHITECTURE 5). A `Diagnostic` that carries a `FixAction` and shows no
/// action is a defect (`docs/UX.md` 4.4), so the control has to be reachable
/// from wherever a diagnostic is drawn.
///
/// The line stays on the honest side of that rule: it never offers a button
/// for something the client cannot do. Writing an environment variable into
/// the configuration needs `SetConfig`, which answers `unimplemented` until
/// HUM-069; copying the `export` line is the part that works today.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../domain/domain.dart';
import '../../l10n/l10n.dart';
import 'shell_command.dart';
import 'ui.dart';

/// The control for a [FixAction].
class FixControl extends StatefulWidget {
  /// Creates the control for [fix]; renders nothing for null.
  const FixControl({required this.fix, this.copyKey, super.key});

  /// The proposed fix.
  final FixAction? fix;

  /// Key of the copy button, for a screen that draws more than one of these.
  ///
  /// Two cards in the same strip would otherwise both answer to
  /// `setup-fix-copy`, and a test that taps that key would not know which one
  /// it hit. Null keeps the shared key the single-card screens use.
  final Key? copyKey;

  @override
  State<FixControl> createState() => _FixControlState();
}

class _FixControlState extends State<FixControl> {
  bool _copied = false;

  Future<void> _copy(String text) async {
    await Clipboard.setData(ClipboardData(text: text));
    if (!mounted) {
      return;
    }
    setState(() => _copied = true);
    // Kein Literal: jede Dauer kommt aus `HMotion` (`docs/UX.md` 2.1).
    await Future<void>.delayed(HMotion.copyFeedback);
    if (mounted) {
      setState(() => _copied = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TextStyle mono = tokens.typography.mono12.tinted(tokens.colors.fg1);
    return switch (widget.fix) {
      null => const SizedBox.shrink(),
      FixActionCopyCommand(:final command) => _copyRow(
        tokens,
        label: _copied ? l10n.setupFixCopied : l10n.setupFixCopyCommand,
        text: command,
        style: mono,
      ),
      FixActionOpenUrl(:final url) => _copyRow(
        tokens,
        label: _copied ? l10n.setupFixCopied : l10n.setupFixCopyLink,
        text: url,
        // Der Akzent ist eine Fläche und misst hell 3,42:1 bis 3,74:1; ein
        // Wort darauf braucht 4,5:1 (`docs/UX.md` 6).
        style: tokens.typography.mono12.tinted(tokens.colors.accentText),
      ),
      // Das Abzeichen benennt, was zu tun ist; die Zeile darunter tut den
      // einen Teil davon, den dieser Client heute wirklich ausführen kann.
      // Ein Knopf, der in die Konfiguration schriebe, gehört hier nicht hin:
      // `SetConfig` antwortet bis HUM-069 `unimplemented`, und ein Control,
      // das etwas verspricht, was nicht geschieht, ist schlimmer als keines
      // (`docs/UX.md` 4.4, `backlog/CONVENTIONS.md` 4.13).
      //
      // Der Befehl wird nie interpoliert, sondern gebaut: `shell_command.dart`
      // quotiert den Wert und verweigert die Zeile, wo sie nicht beweisbar
      // genau eine Zuweisung wäre. Dann steht dort der Grund und kein Knopf.
      FixActionSetEnv(:final key, :final value) => _setEnv(
        tokens,
        l10n,
        key: key,
        value: value,
        style: mono,
      ),
      FixActionChangeSetting(:final key) => HBadge(
        text: l10n.setupFixChangeSetting(key),
      ),
      FixActionInstallService() => HBadge(text: l10n.setupFixInstallService),
      FixActionAddRule() => HBadge(text: l10n.setupFixAddRule),
      FixActionRemountReadOnly() => HBadge(text: l10n.setupFixRemountReadOnly),
    };
  }

  /// Der Knopf und daneben, was er kopiert.
  ///
  /// [reflow] wählt zwischen zwei Anordnungen. Ohne ihn steht beides in einer
  /// Zeile und der Text wird gekürzt, wenn er nicht passt; das ist die Form,
  /// die der Setup-Bildschirm, die Sandbox und die Mitteilung des Trays auf
  /// ihren breiten Karten haben. Mit ihm rutscht der Text unter den Knopf,
  /// statt zu überlaufen, und wird nie gekürzt.
  ///
  /// Der Unterschied ist die Breite, die das Control bekommt: In der
  /// Warteschlange sind es ab `HSize.paneMinQueue` 280 px, und dort läuft eine
  /// Zeile aus Knopf und Befehl über den Rand (`docs/UX.md` 6). Was jemand
  /// kopieren soll, soll er auch lesen können, also bricht die schmale Form
  /// um, statt eine Ellipse zu setzen.
  Widget _copyRow(
    HTokens tokens, {
    required String label,
    required String text,
    required TextStyle style,
    bool reflow = false,
  }) {
    final Widget button = HButton(
      key: widget.copyKey ?? const Key('setup-fix-copy'),
      onPressed: () => _copy(text),
      child: Text(label),
    );
    if (reflow) {
      return Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: tokens.spacing.x3,
        runSpacing: tokens.spacing.x1,
        children: <Widget>[
          button,
          Text(text, style: style),
        ],
      );
    }
    return Row(
      children: <Widget>[
        button,
        SizedBox(width: tokens.spacing.x3),
        Expanded(
          child: Text(text, style: style, overflow: TextOverflow.ellipsis),
        ),
      ],
    );
  }

  /// Das Abzeichen für `SetEnv` und darunter entweder die Kopierzeile oder
  /// der Grund, warum es keine gibt.
  ///
  /// Angezeigt und kopiert wird dieselbe Zeichenkette. Wer die eine säuberte
  /// und die andere roh ließe, hätte den Fehler nur verschoben.
  Widget _setEnv(
    HTokens tokens,
    AppLocalizations l10n, {
    required String key,
    required String value,
    required TextStyle style,
  }) {
    final String? command = exportCommand(key: key, value: value);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        HBadge(text: l10n.setupFixSetEnv(key)),
        SizedBox(height: tokens.spacing.x2),
        if (command == null)
          Text(
            switch (exportRefusal(key: key, value: value)) {
              ExportRefusal.value => l10n.setupFixSetEnvMultiline(key),
              // `null` steht hier nur, weil der Typ es zulässt: Ohne Grund
              // gäbe es einen Befehl, und dieser Zweig liefe nicht.
              ExportRefusal.key || null => l10n.setupFixSetEnvBadKey(key),
            },
            key: const Key('setup-fix-no-command'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          )
        else
          _copyRow(
            tokens,
            label: _copied ? l10n.setupFixCopied : l10n.setupFixCopyExport,
            text: command,
            style: style,
            // Diese Zeile steht in der Warteschlange, also im schmalsten
            // Pane des Programms.
            reflow: true,
          ),
      ],
    );
  }
}
