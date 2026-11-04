/// Die Tastatur des Editors (HUM-047).
///
/// Vier Intents, und sie leben hier statt in `core/shortcuts`, weil sie nur
/// gelten, solange der Editor den mittleren Pane hält: Ein `Ctrl+Enter`, das
/// überall im Programm „editierte Fassung senden" hieße, überschriebe das
/// `Ctrl+Enter` der Warteschlange, das blockt (HUM-072) — und „senden" und
/// „blocken" auf derselben Taste ist genau die Verwechslung, die
/// `docs/UX.md` 5.4 verbietet.
///
/// Der Bildschirm hängt sie deshalb in ein eigenes `Shortcuts` **innerhalb**
/// des Editors. Flutter löst von der Fokusstelle nach oben auf, also gewinnt
/// die innere Bindung, solange der Fokus im Editor steht, und die äußere,
/// sobald er ihn verlässt.
library;

import 'package:flutter/widgets.dart';

import '../../core/shortcuts/intents.dart' show withCommandOnMac;
import '../../core/shortcuts/shortcut_catalog.dart';

/// Die Auswahl pseudonymisieren. `Ctrl+R`.
class PseudonymizeSelectionIntent extends Intent {
  /// Baut den Intent.
  const PseudonymizeSelectionIntent();
}

/// Die bearbeitete Fassung senden. `Ctrl+Enter`.
class SendEditedIntent extends Intent {
  /// Baut den Intent.
  const SendEditedIntent();
}

/// Zurück zur Karte; der Entwurf bleibt. `Esc`.
class CloseEditorIntent extends Intent {
  /// Baut den Intent.
  const CloseEditorIntent();
}

/// Zum nächsten offenen Fund springen. `F3`.
class NextFindingIntent extends Intent {
  /// Baut den Intent.
  const NextFindingIntent();
}

/// Die Bindungen des Editors.
///
/// `Esc` steht hier und nicht im Bildschirm darüber: Es schließt den Editor,
/// und solange keiner offen ist, gibt es nichts zu schließen. Die Aktion dazu
/// ist abgeschaltet, wenn ein Popover offen ist — dann gehört `Esc` dem
/// Popover (`docs/UX.md` 5.2).
Map<ShortcutActivator, Intent> editorShortcuts() => withCommandOnMac(
  AppShortcut.bind<Intent>(<AppShortcut, Intent>{
    AppShortcut.editorPseudonymizeSelection:
        const PseudonymizeSelectionIntent(),
    AppShortcut.editorSend: const SendEditedIntent(),
    AppShortcut.editorClose: const CloseEditorIntent(),
    AppShortcut.editorNextFinding: const NextFindingIntent(),
  }),
);
