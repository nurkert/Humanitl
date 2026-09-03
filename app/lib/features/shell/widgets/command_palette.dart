/// The command palette (`Ctrl+K`): a filter field over a short list of
/// commands, keyboard first. Built on `HModal`; the shell mounts it as an
/// overlay layer and owns its open state.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// One entry of the palette.
class PaletteCommand {
  /// Creates a command.
  const PaletteCommand({
    required this.id,
    required this.label,
    required this.run,
  });

  /// Stable identifier, for tests and keys.
  final String id;

  /// The label, localised.
  final String label;

  /// What happens; the palette closes first.
  final VoidCallback run;

  /// True when [query] selects this command: every word of the query occurs
  /// in the label, in any order.
  bool matches(String query) {
    final String haystack = label.toLowerCase();
    return query
        .toLowerCase()
        .split(RegExp(r'\s+'))
        .where((String word) => word.isNotEmpty)
        .every(haystack.contains);
  }
}

/// The palette.
class CommandPalette extends StatefulWidget {
  /// Creates the palette over [commands].
  const CommandPalette({
    required this.commands,
    required this.onClose,
    super.key,
  });

  /// The commands, in display order.
  final List<PaletteCommand> commands;

  /// Invoked when the palette should disappear.
  final VoidCallback onClose;

  /// How many matches are listed at most.
  static const int maxRows = 8;

  @override
  State<CommandPalette> createState() => _CommandPaletteState();
}

class _CommandPaletteState extends State<CommandPalette> {
  final TextEditingController _controller = TextEditingController();
  final FocusNode _focus = FocusNode(debugLabel: 'palette');
  int _selected = 0;

  List<PaletteCommand> get _matches => widget.commands
      .where((PaletteCommand c) => c.matches(_controller.text))
      .take(CommandPalette.maxRows)
      .toList(growable: false);

  @override
  void initState() {
    super.initState();
    // `autofocus` yields to whoever already holds the focus, and the shell
    // does; the palette exists for keyboard input and takes it. The request
    // is deferred until the node is attached.
    _focus.requestFocus();
  }

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _changed(String _) => setState(() => _selected = 0);

  void _move(int delta) {
    final int count = _matches.length;
    if (count == 0) {
      return;
    }
    setState(() => _selected = (_selected + delta) % count);
  }

  void _run([PaletteCommand? command]) {
    final List<PaletteCommand> matches = _matches;
    final PaletteCommand? chosen =
        command ??
        (matches.isEmpty ? null : matches[_selected % matches.length]);
    if (chosen == null) {
      return;
    }
    widget.onClose();
    chosen.run();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<PaletteCommand> matches = _matches;
    return HModal(
      title: Text(l10n.shellPaletteTitle),
      onDismiss: widget.onClose,
      scrimSemanticsLabel: l10n.shellPaletteClose,
      width: 480,
      child: CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.arrowDown): () => _move(1),
          const SingleActivator(LogicalKeyboardKey.arrowUp): () => _move(-1),
          const SingleActivator(LogicalKeyboardKey.enter): _run,
          const SingleActivator(LogicalKeyboardKey.numpadEnter): _run,
          const SingleActivator(LogicalKeyboardKey.escape): widget.onClose,
        },
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            DecoratedBox(
              decoration: BoxDecoration(
                color: tokens.colors.bg1,
                borderRadius: BorderRadius.circular(tokens.radii.control),
                border: Border.all(color: tokens.colors.lineStrong),
              ),
              child: Padding(
                padding: EdgeInsets.symmetric(
                  horizontal: tokens.spacing.x3,
                  vertical: tokens.spacing.x2,
                ),
                child: Stack(
                  alignment: Alignment.centerLeft,
                  children: <Widget>[
                    if (_controller.text.isEmpty)
                      ExcludeSemantics(
                        child: Text(
                          l10n.shellPaletteSearchHint,
                          style: tokens.typography.ui13.tinted(
                            tokens.colors.fg2,
                          ),
                        ),
                      ),
                    EditableText(
                      key: const Key('palette-input'),
                      controller: _controller,
                      focusNode: _focus,
                      autofocus: true,
                      style: tokens.typography.ui13.tinted(tokens.colors.fg0),
                      cursorColor: tokens.colors.accent,
                      backgroundCursorColor: tokens.colors.bg3,
                      selectionColor: HColorDerivation.fade(
                        tokens.colors.accent,
                        0.35,
                      ),
                      onChanged: _changed,
                      onSubmitted: (_) => _run(),
                    ),
                  ],
                ),
              ),
            ),
            SizedBox(height: tokens.spacing.x2),
            if (matches.isEmpty)
              Padding(
                padding: EdgeInsets.symmetric(
                  horizontal: tokens.spacing.x3,
                  vertical: tokens.spacing.x2,
                ),
                child: Text(
                  l10n.shellPaletteNoMatch,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg2),
                ),
              )
            else
              for (int i = 0; i < matches.length; i++)
                _PaletteRow(
                  key: Key('palette-${matches[i].id}'),
                  command: matches[i],
                  selected: i == _selected % matches.length,
                  onTap: () => _run(matches[i]),
                ),
          ],
        ),
      ),
    );
  }
}

class _PaletteRow extends StatelessWidget {
  const _PaletteRow({
    required this.command,
    required this.selected,
    required this.onTap,
    super.key,
  });

  final PaletteCommand command;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Semantics(
      button: true,
      selected: selected,
      label: command.label,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: onTap,
        child: Container(
          height: HSize.hitMin,
          padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
          decoration: BoxDecoration(
            color: selected ? tokens.colors.bg3 : const Color(0x00000000),
            borderRadius: BorderRadius.circular(tokens.radii.control),
          ),
          alignment: Alignment.centerLeft,
          child: Text(
            command.label,
            style: tokens.typography.ui13.tinted(
              selected ? tokens.colors.fg0 : tokens.colors.fg1,
            ),
          ),
        ),
      ),
    );
  }
}
