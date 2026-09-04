/// The control for a [FixAction]: a button for what the client can do today
/// (copy a command or a link), a chip naming the action for what HUM-044
/// wires up later.
///
/// Lives in `core/ui` because two screens show diagnostics -- the setup screen
/// and the action bar of the intercept screen -- and no feature imports
/// another one (ARCHITECTURE 5). A `Diagnostic` that carries a `FixAction`
/// and shows no action is a defect (`docs/UX.md` 4.4), so the control has to
/// be reachable from wherever a diagnostic is drawn.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../domain/domain.dart';
import '../../l10n/l10n.dart';
import 'ui.dart';

/// The control for a [FixAction].
class FixControl extends StatefulWidget {
  /// Creates the control for [fix]; renders nothing for null.
  const FixControl({required this.fix, super.key});

  /// The proposed fix.
  final FixAction? fix;

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
      FixActionSetEnv(:final key) => HBadge(text: l10n.setupFixSetEnv(key)),
      FixActionChangeSetting(:final key) => HBadge(
        text: l10n.setupFixChangeSetting(key),
      ),
      FixActionInstallService() => HBadge(text: l10n.setupFixInstallService),
      FixActionAddRule() => HBadge(text: l10n.setupFixAddRule),
      FixActionRemountReadOnly() => HBadge(text: l10n.setupFixRemountReadOnly),
    };
  }

  Widget _copyRow(
    HTokens tokens, {
    required String label,
    required String text,
    required TextStyle style,
  }) {
    return Row(
      children: <Widget>[
        HButton(
          key: const Key('setup-fix-copy'),
          onPressed: () => _copy(text),
          child: Text(label),
        ),
        SizedBox(width: tokens.spacing.x3),
        Expanded(
          child: Text(text, style: style, overflow: TextOverflow.ellipsis),
        ),
      ],
    );
  }
}
