/// The note field of the action bar (HUM-072).
///
/// Hidden until `N`, closed by `Escape` and by every decision, never a
/// permanent row of the bar (`docs/UX.md` 5.4). While it stands it shows two
/// things the person cannot check anywhere else: how much of the 500
/// characters is left, and the line the agent will read in the body of the
/// `403` -- sanitised exactly as the daemon sanitises it, so the field never
/// shows one text while the agent receives another.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/ui/focus_ring.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../block_note.dart';
import '../providers/note.dart';

/// The single line note field, with its counter and its preview.
class NoteField extends ConsumerStatefulWidget {
  /// Creates the field.
  const NoteField({super.key});

  @override
  ConsumerState<NoteField> createState() => _NoteFieldState();
}

class _NoteFieldState extends ConsumerState<NoteField> {
  final FocusNode _focus = FocusNode(debugLabel: 'intercept-note');
  final TextEditingController _controller = TextEditingController();
  bool _focused = false;

  // Fields, not expressions in `build`: a map rebuilt every frame is a new
  // object for every descendant that depends on it (`docs/UX.md` 7).
  late final Map<ShortcutActivator, VoidCallback> _bindings =
      <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.escape): _close,
      };

  @override
  void initState() {
    super.initState();
    _focus
      ..addListener(_syncFocus)
      // The field opens because somebody asked for it, so it takes the
      // keyboard at once; without that, `N` would only draw a box.
      ..requestFocus();
    _controller.text = ref.read(blockNoteProvider).text;
  }

  @override
  void dispose() {
    _focus
      ..removeListener(_syncFocus)
      ..dispose();
    _controller.dispose();
    super.dispose();
  }

  void _syncFocus() {
    if (_focused != _focus.hasFocus) {
      setState(() => _focused = _focus.hasFocus);
    }
  }

  /// Closes the field and hands the keyboard back to the screen, which parks
  /// it on its own node again.
  void _close() {
    ref.read(blockNoteProvider.notifier).close();
    _focus.unfocus();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final NoteDraft draft = ref.watch(blockNoteProvider);
    final String sanitized = draft.sanitized;
    final TextStyle style = tokens.typography.ui13.tinted(tokens.colors.fg0);
    return CallbackShortcuts(
      bindings: _bindings,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Expanded(
                child: Semantics(
                  textField: true,
                  label: l10n.interceptNoteHint,
                  child: FocusRing(
                    visible: _focused,
                    radius: tokens.radii.control,
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        color: tokens.colors.bg2,
                        borderRadius: HRadius.controlRadius,
                        border: Border.all(color: tokens.colors.line),
                      ),
                      child: Padding(
                        padding: EdgeInsets.symmetric(
                          horizontal: tokens.spacing.x2,
                          vertical: tokens.spacing.x2 - 1,
                        ),
                        child: ConstrainedBox(
                          // A minimum, never a height: at twice the text
                          // scale the line is taller than the box would be
                          // (`docs/UX.md` 6).
                          constraints: BoxConstraints(
                            minHeight: tokens.typography.ui13.fontSize ?? 13,
                          ),
                          child: Stack(
                            alignment: Alignment.centerLeft,
                            children: <Widget>[
                              if (draft.text.isEmpty)
                                ExcludeSemantics(
                                  child: Text(
                                    l10n.interceptNoteHint,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: style.tinted(tokens.colors.fg1),
                                  ),
                                ),
                              EditableText(
                                key: const Key('intercept-note-input'),
                                controller: _controller,
                                focusNode: _focus,
                                style: style,
                                cursorColor: tokens.colors.accent,
                                backgroundCursorColor: tokens.colors.bg3,
                                selectionColor: HColorDerivation.fade(
                                  tokens.colors.accent,
                                  0.35,
                                ),
                                inputFormatters: <TextInputFormatter>[
                                  // 500 characters, counted in the field and
                                  // not only in the daemon: the counter must
                                  // be able to say "full" before the note
                                  // leaves (HUM-072).
                                  LengthLimitingTextInputFormatter(
                                    noteMaxChars,
                                  ),
                                ],
                                onChanged: ref
                                    .read(blockNoteProvider.notifier)
                                    .write,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              Text(
                l10n.interceptNoteCounter(
                  draft.text.runes.length,
                  noteMaxChars,
                ),
                key: const Key('intercept-note-counter'),
                style: tokens.typography.mono11.tinted(tokens.colors.fg1),
              ),
            ],
          ),
          SizedBox(height: tokens.spacing.x1),
          Row(
            children: <Widget>[
              Expanded(
                child: sanitized.isEmpty
                    ? ExcludeSemantics(
                        child: Text(
                          l10n.interceptNoteKeys,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tokens.typography.mono11.tinted(
                            tokens.colors.fg1,
                          ),
                        ),
                      )
                    : Text(
                        l10n.interceptNoteAgentReads(
                          '$noteBodyPrefix$sanitized',
                        ),
                        key: const Key('intercept-note-preview'),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: tokens.typography.mono11.tinted(
                          tokens.colors.fg1,
                        ),
                      ),
              ),
            ],
          ),
          if (noteLosesCharactersInHeader(sanitized))
            Text(
              l10n.interceptNoteAsciiOnly(noteHeaderName),
              key: const Key('intercept-note-ascii'),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.ui11.tinted(tokens.colors.fg1),
            ),
        ],
      ),
    );
  }
}
