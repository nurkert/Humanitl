/// Der Katalog der Tastenkürzel der Anwendung (HUM-420).
///
/// Jedes Tastenkürzel der Anwendung, das über einen Aktivator gebunden ist
/// oder einen Aktivator des Katalogs fragt, steht hier genau einmal: mit einer
/// festen Kennung, ihrem Bereich, ihren Aktivatoren und ihrem Namen in der
/// Sprache der Oberfläche. Die Bindungen der Bereiche (`shellShortcuts`,
/// `interceptShortcuts`, `historyShortcuts` ...) lesen ihre Aktivatoren von
/// hier, und die Kategorie „Tastenkürzel“ der Einstellungen zeigt dieselben
/// Einträge. Was die Übersicht nennt, ist deshalb immer, was gebunden ist.
///
/// Welche Absicht (`Intent`) eine Taste auslöst, steht weiter dort, wo sie
/// gebunden wird: Die Absichten mancher Bereiche gehören ihrem Feature, und
/// der Kern importiert kein Feature (ARCHITECTURE 5). Der Katalog sagt nur,
/// welche Tasten zu welcher Handlung gehören.
///
/// **Für HUM-421.** Frei belegbare Kürzel setzen hier an: Die Kennung
/// ([AppShortcut.name]) ist der stabile Schlüssel einer gespeicherten
/// Belegung, [AppShortcut.activators] die Vorgabe, und
/// [AppShortcut.scope] der Raum, in dem zwei Belegungen sich nicht in die
/// Quere kommen dürfen. Ein Name einer Kennung ändert sich deshalb nie.
/// Heute lesen die Bindungen [AppShortcut.activators] direkt; HUM-421 braucht
/// dafür einen Auflöser, der eine gespeicherte Belegung über die Vorgabe legt
/// (etwa `activatorsOf(shortcut)`), und jede Bindung liest dann ihn. Er ist
/// hier mit Absicht noch nicht gebaut.
///
/// Nicht im Katalog stehen die allgemeinen Tasten der Bausteine aus
/// `humanitl_ui` (`Esc` schließt Modal, Blatt und Kontextmenü, die Pfeile
/// gehen durch ein Bedienelement hindurch, `Enter` und `Leertaste` lösen eine
/// Pille aus): Sie gelten für jedes solche Element und gehören keiner
/// Handlung der Anwendung. Der Test führt sie als begründete Ausnahmen.
///
/// Ein Test hält den Katalog vollständig: Außerhalb dieser Datei und der
/// benannten Konstanten in `intents.dart` baut kein Code der Anwendung einen
/// Aktivator (`test/core/shortcuts/shortcut_catalog_test.dart`).
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../l10n/l10n.dart';
import 'intents.dart';

/// Wo ein Kürzel gilt; zugleich die Gruppen der Übersicht, in dieser
/// Reihenfolge.
enum ShortcutScope {
  /// In jedem Bereich der Shell.
  global,

  /// Das Terminal der Sandbox, wo es auch steht.
  terminal,

  /// Die Warteschlange des Bereichs Intercept, solange sie den Fokus hat.
  queue,

  /// Die Pause über Funden vor dem Senden (HUM-049).
  findingsPause,

  /// Der Editor einer angehaltenen Anfrage (HUM-047).
  editor,

  /// Der Bereich Verlauf.
  history,

  /// Die Liste der Regeln.
  rules,

  /// Das Overlay der Einstellungen.
  settings,

  /// Die Listen der Einstellungen: Kategorien und Modellserver.
  lists,

  /// Die Befehlspalette.
  palette,

  /// Dialoge, Rückfragen und einzelne Bedienelemente mit Fokus.
  controls;

  /// Der Name der Gruppe in der Übersicht.
  String title(AppLocalizations l10n) => switch (this) {
    ShortcutScope.global => l10n.shortcutScopeGlobal,
    ShortcutScope.terminal => l10n.shortcutScopeTerminal,
    ShortcutScope.queue => l10n.shortcutScopeQueue,
    ShortcutScope.findingsPause => l10n.shortcutScopeFindingsPause,
    ShortcutScope.editor => l10n.shortcutScopeEditor,
    ShortcutScope.history => l10n.shortcutScopeHistory,
    ShortcutScope.rules => l10n.shortcutScopeRules,
    ShortcutScope.settings => l10n.shortcutScopeSettings,
    ShortcutScope.lists => l10n.shortcutScopeLists,
    ShortcutScope.palette => l10n.shortcutScopePalette,
    ShortcutScope.controls => l10n.shortcutScopeControls,
  };
}

/// Ein Kürzel der Anwendung.
///
/// Die Reihenfolge innerhalb eines Bereichs ist die der Übersicht. Hat ein
/// Eintrag mehrere Aktivatoren, ist ihre Reihenfolge Teil der Bindung, wo
/// sie zählt: [navigate] führt die Bereiche der Reihe nach,
/// [rememberDuration] und [rememberTarget] die Segmente, [dividerBack] und
/// [dividerForward] erst die senkrechte, dann die waagerechte Achse.
enum AppShortcut {
  // --- Überall (HUM-019, HUM-069, HUM-254, HUM-356) -------------------------

  /// `Ctrl+1` bis `Ctrl+6`, ein Bereich je Ziffer, in der Reihenfolge von
  /// `Section`. Die Liste wächst nur am Ende ([navigationKeys]).
  navigate(ShortcutScope.global, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.digit1, control: true),
    SingleActivator(LogicalKeyboardKey.digit2, control: true),
    SingleActivator(LogicalKeyboardKey.digit3, control: true),
    SingleActivator(LogicalKeyboardKey.digit4, control: true),
    SingleActivator(LogicalKeyboardKey.digit5, control: true),
    SingleActivator(LogicalKeyboardKey.digit6, control: true),
  ], range: true),

  /// `Ctrl+K`.
  palette(ShortcutScope.global, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyK, control: true),
  ]),

  /// `Ctrl+,`.
  settings(ShortcutScope.global, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.comma, control: true),
  ]),

  /// `Ctrl+Shift+0`.
  toggleRail(ShortcutScope.global, <ShortcutActivator>[railToggleActivator]),

  /// `Ctrl` + `+`, auf macOS auch `Cmd`.
  scaleIn(ShortcutScope.global, <ShortcutActivator>[
    UiScaleActivator(UiScaleStep.zoomIn),
  ]),

  /// `Ctrl` + `-`.
  scaleOut(ShortcutScope.global, <ShortcutActivator>[
    UiScaleActivator(UiScaleStep.zoomOut),
  ]),

  /// `Ctrl+0`.
  scaleReset(ShortcutScope.global, <ShortcutActivator>[
    UiScaleActivator(UiScaleStep.reset),
  ]),

  // --- Terminal (HUM-298 bis HUM-314, HUM-353) ------------------------------

  /// `Ctrl+Shift+Enter`, auch mit dem `Enter` des Ziffernblocks.
  terminalFullWindow(ShortcutScope.terminal, <ShortcutActivator>[
    terminalFullWindowActivator,
    terminalFullWindowNumpadActivator,
  ]),

  /// `Ctrl+Shift+P`.
  terminalFloat(ShortcutScope.terminal, <ShortcutActivator>[
    terminalFloatActivator,
  ]),

  /// `Ctrl` mit der Taste links der `1`.
  terminalFocus(ShortcutScope.terminal, <ShortcutActivator>[
    TerminalFocusActivator(),
  ]),

  /// `Ctrl+Alt` + `+`.
  terminalZoomIn(ShortcutScope.terminal, <ShortcutActivator>[
    TerminalZoomActivator(TerminalZoom.zoomIn),
  ]),

  /// `Ctrl+Alt` + `-`.
  terminalZoomOut(ShortcutScope.terminal, <ShortcutActivator>[
    TerminalZoomActivator(TerminalZoom.zoomOut),
  ]),

  /// `Ctrl+Alt+0`.
  terminalZoomReset(ShortcutScope.terminal, <ShortcutActivator>[
    TerminalZoomActivator(TerminalZoom.reset),
  ]),

  /// `Ctrl+Shift+T`.
  terminalNewShell(ShortcutScope.terminal, <ShortcutActivator>[
    openShellActivator,
  ]),

  /// `Ctrl+PageUp`.
  terminalPreviousTab(ShortcutScope.terminal, <ShortcutActivator>[
    previousTerminalTabActivator,
  ]),

  /// `Ctrl+PageDown`.
  terminalNextTab(ShortcutScope.terminal, <ShortcutActivator>[
    nextTerminalTabActivator,
  ]),

  /// `Ctrl+Shift+C`, auf Apple-Plattformen `Cmd+C`.
  terminalCopy(ShortcutScope.terminal, <ShortcutActivator>[
    TerminalClipboardActivator(TerminalClipboard.copy),
  ]),

  /// Das schlichte `Ctrl+C`, solange sichtbarer Text markiert ist (HUM-353);
  /// nicht auf Apple-Plattformen, dort ist es immer die Unterbrechung.
  /// `TerminalPane` fragt es selbst und erst nach der Auswahl.
  terminalCopySelection(ShortcutScope.terminal, <ShortcutActivator>[
    SingleActivator(
      LogicalKeyboardKey.keyC,
      control: true,
      includeRepeats: false,
    ),
  ], notOnApple: true),

  /// `Ctrl+Shift+V`, auf Apple-Plattformen `Cmd+V`.
  terminalPaste(ShortcutScope.terminal, <ShortcutActivator>[
    TerminalClipboardActivator(TerminalClipboard.paste),
  ]),

  // --- Warteschlange (HUM-020, HUM-029, HUM-047, HUM-072) -------------------
  //
  // Die beiden Entscheidungen ohne Wiederholung (`docs/UX.md` 5.4), die
  // Buchstaben nur, solange kein Textfeld den Fokus hat (5.2); das prüfen die
  // Aktionen des Bildschirms, nicht die Bindung.

  /// `Enter`, `A`.
  allow(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.enter, includeRepeats: false),
    SingleActivator(LogicalKeyboardKey.numpadEnter, includeRepeats: false),
    SingleActivator(LogicalKeyboardKey.keyA, includeRepeats: false),
  ]),

  /// `Ctrl+F`, auch im Notizfeld.
  allowAnywhere(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(
      LogicalKeyboardKey.keyF,
      control: true,
      includeRepeats: false,
    ),
  ]),

  /// `B`.
  block(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyB, includeRepeats: false),
  ]),

  /// `Ctrl+Enter` und `Ctrl+L`, auch im Notizfeld, mit der Notiz.
  blockAnywhere(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(
      LogicalKeyboardKey.enter,
      control: true,
      includeRepeats: false,
    ),
    SingleActivator(
      LogicalKeyboardKey.keyL,
      control: true,
      includeRepeats: false,
    ),
  ]),

  /// `J`, `↓`.
  nextRequest(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyJ),
    SingleActivator(LogicalKeyboardKey.arrowDown),
  ]),

  /// `K`, `↑`.
  previousRequest(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyK),
    SingleActivator(LogicalKeyboardKey.arrowUp),
  ]),

  /// `Shift+Enter`.
  openRemember(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.enter, shift: true),
  ]),

  /// `N`.
  note(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyN),
  ]),

  /// `E`.
  edit(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyE),
  ]),

  /// `Esc`, ohne Wiederholung (HUM-049).
  closeHint(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.escape, includeRepeats: false),
  ]),

  /// `Ctrl+Shift+F`.
  allowGroup(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(
      LogicalKeyboardKey.keyF,
      control: true,
      shift: true,
      includeRepeats: false,
    ),
  ]),

  /// `Ctrl+Shift+L`.
  blockGroup(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(
      LogicalKeyboardKey.keyL,
      control: true,
      shift: true,
      includeRepeats: false,
    ),
  ]),

  /// `Ctrl+A`.
  selectGroup(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyA, control: true),
  ]),

  /// `Shift+J`.
  mergeArrivals(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyJ, shift: true),
  ]),

  /// `→`.
  openGroup(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowRight),
  ]),

  /// `←`.
  foldGroup(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowLeft),
  ]),

  /// `1` bis `4`, ein Segment je Ziffer.
  rememberDuration(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.digit1),
    SingleActivator(LogicalKeyboardKey.digit2),
    SingleActivator(LogicalKeyboardKey.digit3),
    SingleActivator(LogicalKeyboardKey.digit4),
  ], range: true),

  /// `Shift+1` bis `Shift+4`.
  rememberTarget(ShortcutScope.queue, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.digit1, shift: true),
    SingleActivator(LogicalKeyboardKey.digit2, shift: true),
    SingleActivator(LogicalKeyboardKey.digit3, shift: true),
    SingleActivator(LogicalKeyboardKey.digit4, shift: true),
  ], range: true),

  // --- Pause über Funden (HUM-049) ------------------------------------------

  /// `S`, ohne Wiederholung.
  sendAnyway(ShortcutScope.findingsPause, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyS, includeRepeats: false),
  ]),

  /// `P`, ohne Wiederholung.
  pseudonymize(ShortcutScope.findingsPause, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyP, includeRepeats: false),
  ]),

  /// `Esc`, ohne Wiederholung.
  closeFindingsPause(ShortcutScope.findingsPause, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.escape, includeRepeats: false),
  ]),

  // --- Editor (HUM-047) -----------------------------------------------------

  /// `Ctrl+R`.
  editorPseudonymizeSelection(ShortcutScope.editor, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyR, control: true),
  ]),

  /// `Ctrl+Enter`, ohne Wiederholung.
  editorSend(ShortcutScope.editor, <ShortcutActivator>[
    SingleActivator(
      LogicalKeyboardKey.enter,
      control: true,
      includeRepeats: false,
    ),
  ]),

  /// `Esc`.
  editorClose(ShortcutScope.editor, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.escape),
  ]),

  /// `F3`.
  editorNextFinding(ShortcutScope.editor, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.f3),
  ]),

  // --- Verlauf --------------------------------------------------------------

  /// `Enter`.
  historyOpen(ShortcutScope.history, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.enter),
    SingleActivator(LogicalKeyboardKey.numpadEnter),
  ]),

  /// `/`.
  historyFilter(ShortcutScope.history, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.slash),
  ]),

  /// `J`, `↓`.
  historyNext(ShortcutScope.history, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyJ),
    SingleActivator(LogicalKeyboardKey.arrowDown),
  ]),

  /// `K`, `↑`.
  historyPrevious(ShortcutScope.history, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyK),
    SingleActivator(LogicalKeyboardKey.arrowUp),
  ]),

  // --- Regeln ---------------------------------------------------------------

  /// `Alt+↑`.
  ruleUp(ShortcutScope.rules, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowUp, alt: true),
  ]),

  /// `Alt+↓`.
  ruleDown(ShortcutScope.rules, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowDown, alt: true),
  ]),

  // --- Einstellungen (HUM-404, HUM-405, HUM-414) ----------------------------

  /// `Ctrl+F`. Ohne `Cmd` auf macOS: Das Overlay bindet es nicht über
  /// [withCommandOnMac].
  settingsSearch(ShortcutScope.settings, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyF, control: true),
  ], commandOnMac: false),

  /// `Esc`: erst die Suche leeren, dann schließen.
  settingsClose(ShortcutScope.settings, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.escape),
  ]),

  /// `Ctrl+N`.
  serverAdd(ShortcutScope.settings, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyN, control: true),
  ]),

  /// `Ctrl+S`.
  serverSave(ShortcutScope.settings, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.keyS, control: true),
  ]),

  /// `Enter` auf einer Zeile der Serverliste.
  serverOpen(ShortcutScope.settings, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.enter),
    SingleActivator(LogicalKeyboardKey.numpadEnter),
  ]),

  /// `Space` auf einer Zeile der Serverliste.
  serverChoose(ShortcutScope.settings, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.space),
  ]),

  // --- Listen der Einstellungen (HUM-404, HUM-414) -------------------------
  //
  // `SettingsNav` und `ServerList` lesen diese Tasten in ihrem eigenen
  // `onKeyEvent`, gedrückt und wiederholt, ohne auf Modifikatoren zu achten
  // ([matches]); die Detailansicht eines Servers nimmt `Esc` nur gedrückt.

  /// `↓`.
  listNext(ShortcutScope.lists, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowDown),
  ]),

  /// `↑`.
  listPrevious(ShortcutScope.lists, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowUp),
  ]),

  /// `Pos1`.
  listFirst(ShortcutScope.lists, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.home),
  ]),

  /// `Ende`.
  listLast(ShortcutScope.lists, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.end),
  ]),

  /// `Esc` in der Detailansicht eines Servers: zurück zur Liste.
  listBack(ShortcutScope.lists, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.escape, includeRepeats: false),
  ]),

  // --- Befehlspalette (HUM-019) ---------------------------------------------

  /// `↓`.
  paletteNext(ShortcutScope.palette, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowDown),
  ]),

  /// `↑`.
  palettePrevious(ShortcutScope.palette, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowUp),
  ]),

  /// `Enter`.
  paletteRun(ShortcutScope.palette, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.enter),
    SingleActivator(LogicalKeyboardKey.numpadEnter),
  ]),

  /// `Esc`.
  paletteClose(ShortcutScope.palette, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.escape),
  ]),

  // --- Dialoge und Bedienelemente -------------------------------------------

  /// `Esc` im Notizfeld, im Dialog der Gruppenentscheidung, in der Pille der
  /// Anhalte-Rückfrage und in der Rückfrage vor dem Neustart des Daemons.
  closeLayer(ShortcutScope.controls, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.escape),
  ]),

  /// `↑` an einer senkrechten, `←` an einer waagerechten Trennlinie mit
  /// Fokus.
  dividerBack(ShortcutScope.controls, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowUp),
    SingleActivator(LogicalKeyboardKey.arrowLeft),
  ]),

  /// `↓` an einer senkrechten, `→` an einer waagerechten Trennlinie.
  dividerForward(ShortcutScope.controls, <ShortcutActivator>[
    SingleActivator(LogicalKeyboardKey.arrowDown),
    SingleActivator(LogicalKeyboardKey.arrowRight),
  ]);

  const AppShortcut(
    this.scope,
    this.activators, {
    this.range = false,
    this.commandOnMac = true,
    this.notOnApple = false,
  });

  /// Wo das Kürzel gilt.
  final ShortcutScope scope;

  /// Die Tasten, die es auslösen, in der Reihenfolge der Bindung.
  final List<ShortcutActivator> activators;

  /// Ob die Übersicht die Aktivatoren als Spanne zeigt (`Ctrl` `1–6`) statt
  /// jeden einzeln.
  final bool range;

  /// Ob ein `Ctrl` darin auf macOS auch als `Cmd` gilt, weil die Bindung über
  /// [withCommandOnMac] läuft. Die Übersicht zeigt es dann als `⌘`.
  final bool commandOnMac;

  /// Ob das Kürzel auf Apple-Plattformen fehlt.
  final bool notOnApple;

  /// Der Name des Kürzels in der Übersicht und in der Suche.
  String label(AppLocalizations l10n) => switch (this) {
    AppShortcut.navigate => l10n.shortcutNavigate,
    AppShortcut.palette => l10n.shortcutPalette,
    AppShortcut.settings => l10n.shortcutSettings,
    AppShortcut.toggleRail => l10n.shortcutToggleRail,
    AppShortcut.scaleIn => l10n.shortcutScaleIn,
    AppShortcut.scaleOut => l10n.shortcutScaleOut,
    AppShortcut.scaleReset => l10n.shortcutScaleReset,
    AppShortcut.terminalFullWindow => l10n.shortcutTerminalFullWindow,
    AppShortcut.terminalFloat => l10n.shortcutTerminalFloat,
    AppShortcut.terminalFocus => l10n.shortcutTerminalFocus,
    AppShortcut.terminalZoomIn => l10n.shortcutTerminalZoomIn,
    AppShortcut.terminalZoomOut => l10n.shortcutTerminalZoomOut,
    AppShortcut.terminalZoomReset => l10n.shortcutTerminalZoomReset,
    AppShortcut.terminalNewShell => l10n.shortcutTerminalNewShell,
    AppShortcut.terminalPreviousTab => l10n.shortcutTerminalPreviousTab,
    AppShortcut.terminalNextTab => l10n.shortcutTerminalNextTab,
    AppShortcut.terminalCopy => l10n.shortcutTerminalCopy,
    AppShortcut.terminalCopySelection => l10n.shortcutTerminalCopySelection,
    AppShortcut.terminalPaste => l10n.shortcutTerminalPaste,
    AppShortcut.allow => l10n.shortcutAllow,
    AppShortcut.allowAnywhere => l10n.shortcutAllowAnywhere,
    AppShortcut.block => l10n.shortcutBlock,
    AppShortcut.blockAnywhere => l10n.shortcutBlockAnywhere,
    AppShortcut.nextRequest => l10n.shortcutNextRequest,
    AppShortcut.previousRequest => l10n.shortcutPreviousRequest,
    AppShortcut.openRemember => l10n.shortcutOpenRemember,
    AppShortcut.note => l10n.shortcutNote,
    AppShortcut.edit => l10n.shortcutEdit,
    AppShortcut.closeHint => l10n.shortcutCloseHint,
    AppShortcut.allowGroup => l10n.shortcutAllowGroup,
    AppShortcut.blockGroup => l10n.shortcutBlockGroup,
    AppShortcut.selectGroup => l10n.shortcutSelectGroup,
    AppShortcut.mergeArrivals => l10n.shortcutMergeArrivals,
    AppShortcut.openGroup => l10n.shortcutOpenGroup,
    AppShortcut.foldGroup => l10n.shortcutFoldGroup,
    AppShortcut.rememberDuration => l10n.shortcutRememberDuration,
    AppShortcut.rememberTarget => l10n.shortcutRememberTarget,
    AppShortcut.sendAnyway => l10n.shortcutSendAnyway,
    AppShortcut.pseudonymize => l10n.shortcutPseudonymize,
    AppShortcut.closeFindingsPause => l10n.shortcutCloseFindingsPause,
    AppShortcut.editorPseudonymizeSelection =>
      l10n.shortcutEditorPseudonymizeSelection,
    AppShortcut.editorSend => l10n.shortcutEditorSend,
    AppShortcut.editorClose => l10n.shortcutEditorClose,
    AppShortcut.editorNextFinding => l10n.shortcutEditorNextFinding,
    AppShortcut.historyOpen => l10n.shortcutHistoryOpen,
    AppShortcut.historyFilter => l10n.shortcutHistoryFilter,
    AppShortcut.historyNext => l10n.shortcutHistoryNext,
    AppShortcut.historyPrevious => l10n.shortcutHistoryPrevious,
    AppShortcut.ruleUp => l10n.shortcutRuleUp,
    AppShortcut.ruleDown => l10n.shortcutRuleDown,
    AppShortcut.settingsSearch => l10n.shortcutSettingsSearch,
    AppShortcut.settingsClose => l10n.shortcutSettingsClose,
    AppShortcut.serverAdd => l10n.shortcutServerAdd,
    AppShortcut.serverSave => l10n.shortcutServerSave,
    AppShortcut.serverOpen => l10n.shortcutServerOpen,
    AppShortcut.serverChoose => l10n.shortcutServerChoose,
    AppShortcut.listNext => l10n.shortcutListNext,
    AppShortcut.listPrevious => l10n.shortcutListPrevious,
    AppShortcut.listFirst => l10n.shortcutListFirst,
    AppShortcut.listLast => l10n.shortcutListLast,
    AppShortcut.listBack => l10n.shortcutListBack,
    AppShortcut.paletteNext => l10n.shortcutPaletteNext,
    AppShortcut.palettePrevious => l10n.shortcutPalettePrevious,
    AppShortcut.paletteRun => l10n.shortcutPaletteRun,
    AppShortcut.paletteClose => l10n.shortcutPaletteClose,
    AppShortcut.closeLayer => l10n.shortcutCloseLayer,
    AppShortcut.dividerBack => l10n.shortcutDividerBack,
    AppShortcut.dividerForward => l10n.shortcutDividerForward,
  };

  /// Ob [event] eine Taste dieses Eintrags ist, nach der Taste allein.
  ///
  /// Für die Listen, die ihre Tasten in einem eigenen `onKeyEvent` lesen und
  /// dabei nie nach Modifikatoren gefragt haben; ob ein Druck, eine
  /// Wiederholung oder beides zählt, entscheidet der Aufrufer.
  bool matches(KeyEvent event) => activators.any(
    (ShortcutActivator activator) =>
        activator.triggers?.contains(event.logicalKey) ?? false,
  );

  /// Der einzige Aktivator eines Eintrags mit genau einem.
  ShortcutActivator get single => activators.single;

  /// Ob das Kürzel auf [platform] gilt; ohne Angabe
  /// [defaultTargetPlatform].
  bool appliesOn([TargetPlatform? platform]) {
    final TargetPlatform on = platform ?? defaultTargetPlatform;
    return !(notOnApple &&
        (on == TargetPlatform.macOS || on == TargetPlatform.iOS));
  }

  /// Die Einträge von [scope], in der Reihenfolge der Übersicht.
  static List<AppShortcut> of(ShortcutScope scope) => <AppShortcut>[
    for (final AppShortcut shortcut in values)
      if (shortcut.scope == scope) shortcut,
  ];

  /// Jeder Aktivator von [shortcuts] auf [intent].
  ///
  /// Für die Einträge, deren Tasten alle dieselbe Absicht auslösen; eine
  /// Bindung, deren Absicht von der Taste abhängt ([navigate],
  /// [rememberDuration]), liest [activators] selbst.
  static Map<ShortcutActivator, T> bind<T>(Map<AppShortcut, T> shortcuts) =>
      <ShortcutActivator, T>{
        for (final MapEntry<AppShortcut, T> entry in shortcuts.entries)
          for (final ShortcutActivator activator in entry.key.activators)
            activator: entry.value,
      };
}
