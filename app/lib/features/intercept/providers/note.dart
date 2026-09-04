/// The note for the agent: the draft behind a block (HUM-072).
///
/// The note is temporary. It is hidden until `N`, it closes on `Escape` and on
/// every decision, and it never becomes a permanent row of the action bar: a
/// visible optional text field adds a focus stop, a frame and a character
/// counter to the bar and invites typing where the normal way is one key
/// (`docs/UX.md` 5.4).
///
/// One draft for the selection, not one per flow: the draft resets with every
/// new selection, so a note written for one request can never travel to the
/// next one unseen (`backlog/CONVENTIONS.md` 4.13). That is stronger than the
/// `blockNoteProvider(FlowId)` of the specification and keeps the same name.
library;

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../block_note.dart';
import 'flows.dart';

part 'note.g.dart';

/// What the note field holds, and whether it is shown at all.
@immutable
class NoteDraft {
  /// Creates a draft.
  const NoteDraft({this.open = false, this.text = ''});

  /// True while the field is on the screen.
  final bool open;

  /// What was typed, unsanitised; [sanitized] is what goes out.
  final String text;

  /// The note as the agent will read it in the body of the `403`.
  String get sanitized => sanitizeNote(text);

  /// The note that reaches `Decide`, or null when nothing is left of it.
  ///
  /// The daemon sanitises again -- it must, because the CLI and a second
  /// window reach the same RPC -- so this is the preview, not the guard.
  String? get outgoing => sanitized.isEmpty ? null : sanitized;

  /// A copy with the given fields replaced.
  NoteDraft copyWith({bool? open, String? text}) =>
      NoteDraft(open: open ?? this.open, text: text ?? this.text);

  @override
  bool operator ==(Object other) =>
      other is NoteDraft && other.open == open && other.text == text;

  @override
  int get hashCode => Object.hash(open, text);
}

/// The note the next block would carry.
@Riverpod(keepAlive: true)
class BlockNote extends _$BlockNote {
  @override
  NoteDraft build() {
    // Every new selection starts without a note.
    ref.watch(selectedFlowIdProvider);
    return const NoteDraft();
  }

  /// Shows the field. `N`, and the control beside Block.
  void open() => state = state.copyWith(open: true);

  /// Hides the field and forgets what stood in it.
  ///
  /// Closing forgets, because a note nobody can see must not change what the
  /// Block control does; the label would otherwise promise a note that is not
  /// on the screen.
  void close() => state = const NoteDraft();

  /// Takes what was typed, capped at [noteMaxChars] characters.
  void write(String text) {
    final List<int> runes = text.runes.toList();
    final String capped = runes.length <= noteMaxChars
        ? text
        : String.fromCharCodes(runes.sublist(0, noteMaxChars));
    state = state.copyWith(open: true, text: capped);
  }
}
