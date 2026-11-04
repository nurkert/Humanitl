/// Wie ein Kürzel des Katalogs auf dem Schirm heißt (HUM-420).
///
/// Die Tasten werden aus den Aktivatoren gelesen, nicht aus einem zweiten
/// Text: Was hier steht, ist, was gebunden ist. Die Namen der Modifikatoren
/// sind die, die auf der Tastatur der Sprache stehen (`Strg` auf Deutsch,
/// HUM-052); auf macOS stehen die Zeichen der Tasten (`⌘`, `⌥`, `⇧`, `⌃`),
/// und ein `Ctrl`, das dort auch als `Cmd` gilt ([AppShortcut.commandOnMac]),
/// heißt `⌘`.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../l10n/l10n.dart';
import 'intents.dart';
import 'shortcut_catalog.dart';

/// Die Tasten eines Griffs, eine Kappe je Taste, Modifikatoren zuerst.
typedef ShortcutKeys = List<String>;

const String _plus = '+';
const String _minus = '-';

bool _isApple(TargetPlatform platform) =>
    platform == TargetPlatform.macOS || platform == TargetPlatform.iOS;

/// Die Griffe von [shortcut], wie die Übersicht sie zeigt, je Griff seine
/// Tasten.
///
/// Das `Enter` des Ziffernblocks steht nie eigens da: Jede Bindung, die es
/// nimmt, nimmt auch das `Enter` daneben. Eine Spanne ([AppShortcut.range])
/// steht als ein Griff mit der ersten und der letzten Taste (`Ctrl` `1–6`).
List<ShortcutKeys> shortcutChords(
  AppShortcut shortcut,
  AppLocalizations l10n, {
  TargetPlatform? platform,
}) {
  final TargetPlatform on = platform ?? defaultTargetPlatform;
  final List<ShortcutKeys> chords = <ShortcutKeys>[
    for (final ShortcutActivator activator in shortcut.activators)
      if (!(activator is SingleActivator &&
          activator.trigger == LogicalKeyboardKey.numpadEnter))
        activatorKeys(
          activator,
          l10n,
          commandOnMac: shortcut.commandOnMac,
          platform: on,
        ),
  ];
  if (!shortcut.range || chords.length < 2) {
    return chords;
  }
  final ShortcutKeys first = chords.first;
  final ShortcutKeys last = chords.last;
  return <ShortcutKeys>[
    <String>[...first.take(first.length - 1), '${first.last}–${last.last}'],
  ];
}

/// Die Tasten von [activator], Modifikatoren zuerst.
///
/// [commandOnMac] sagt, ob ein `Ctrl` auf macOS als `Cmd` gebunden ist; die
/// eigenen Aktivatoren der Shell wissen das selbst. Ein Aktivator, den diese
/// Funktion nicht kennt, ist ein Fehler: Die Übersicht zeigte sonst eine
/// Taste, die niemand bedacht hat.
ShortcutKeys activatorKeys(
  ShortcutActivator activator,
  AppLocalizations l10n, {
  bool commandOnMac = true,
  TargetPlatform? platform,
}) {
  final TargetPlatform on = platform ?? defaultTargetPlatform;
  final bool apple = _isApple(on);
  ShortcutKeys chord(
    String key, {
    bool control = false,
    bool command = false,
    bool alt = false,
    bool shift = false,
  }) => <String>[
    if (apple) ...<String>[
      if (control) '⌃',
      if (alt) '⌥',
      if (shift) '⇧',
      if (command) '⌘',
    ] else ...<String>[
      if (control || command) l10n.shortcutKeyCtrl,
      if (alt) l10n.shortcutKeyAlt,
      if (shift) l10n.shortcutKeyShift,
    ],
    key,
  ];
  return switch (activator) {
    final SingleActivator single => chord(
      _keyName(single.trigger, l10n),
      control: single.control && !(apple && commandOnMac),
      command: single.meta || (single.control && apple && commandOnMac),
      alt: single.alt,
      shift: single.shift,
    ),
    TerminalFocusActivator() => chord(l10n.shortcutKeyBackquote, control: true),
    TerminalZoomActivator(:final TerminalZoom zoom) => chord(
      switch (zoom) {
        TerminalZoom.zoomIn => _plus,
        TerminalZoom.zoomOut => _minus,
        TerminalZoom.reset => '0',
      },
      control: true,
      alt: true,
    ),
    // Auf macOS gilt `Cmd`, `Ctrl` aber auch; die Übersicht nennt `Cmd`.
    UiScaleActivator(:final UiScaleStep step) => chord(
      switch (step) {
        UiScaleStep.zoomIn => _plus,
        UiScaleStep.zoomOut => _minus,
        UiScaleStep.reset => '0',
      },
      control: !apple,
      command: apple,
    ),
    TerminalClipboardActivator(:final TerminalClipboard action) => chord(
      switch (action) {
        TerminalClipboard.copy => 'C',
        TerminalClipboard.paste => 'V',
      },
      control: !apple,
      shift: !apple,
      command: apple,
    ),
    _ => throw UnsupportedError(
      'No key names for ${activator.runtimeType}; '
      'add it to activatorKeys (HUM-420).',
    ),
  };
}

/// [keys] als eine Zeile Text: `Ctrl+Shift+0` unter Linux, `⌃⇧0` auf
/// macOS. Für Hinweise im Fließtext und die Suche.
String chordText(
  ShortcutKeys keys,
  AppLocalizations l10n, {
  TargetPlatform? platform,
}) {
  if (_isApple(platform ?? defaultTargetPlatform)) {
    return keys.join();
  }
  return <String>[
    for (final String key in keys)
      switch (key) {
        _plus => l10n.shortcutKeyPlus,
        _minus => l10n.shortcutKeyMinus,
        _ => key,
      },
  ].join('+');
}

/// Jeder Griff von [shortcut] als Text, durch ` / ` getrennt.
String shortcutText(
  AppShortcut shortcut,
  AppLocalizations l10n, {
  TargetPlatform? platform,
}) => <String>[
  for (final ShortcutKeys keys in shortcutChords(
    shortcut,
    l10n,
    platform: platform,
  ))
    chordText(keys, l10n, platform: platform),
].join(' / ');

/// Der eine Griff von [activator] als Text, etwa für den Hinweis an einem
/// Knopf.
String activatorText(
  ShortcutActivator activator,
  AppLocalizations l10n, {
  bool commandOnMac = true,
  TargetPlatform? platform,
}) => chordText(
  activatorKeys(
    activator,
    l10n,
    commandOnMac: commandOnMac,
    platform: platform,
  ),
  l10n,
  platform: platform,
);

String _keyName(LogicalKeyboardKey key, AppLocalizations l10n) {
  if (key == LogicalKeyboardKey.enter ||
      key == LogicalKeyboardKey.numpadEnter) {
    return l10n.shortcutKeyEnter;
  }
  if (key == LogicalKeyboardKey.escape) {
    return l10n.shortcutKeyEscape;
  }
  if (key == LogicalKeyboardKey.space) {
    return l10n.shortcutKeySpace;
  }
  if (key == LogicalKeyboardKey.pageUp) {
    return l10n.shortcutKeyPageUp;
  }
  if (key == LogicalKeyboardKey.pageDown) {
    return l10n.shortcutKeyPageDown;
  }
  if (key == LogicalKeyboardKey.home) {
    return l10n.shortcutKeyHome;
  }
  if (key == LogicalKeyboardKey.end) {
    return l10n.shortcutKeyEnd;
  }
  if (key == LogicalKeyboardKey.arrowUp) {
    return '↑';
  }
  if (key == LogicalKeyboardKey.arrowDown) {
    return '↓';
  }
  if (key == LogicalKeyboardKey.arrowLeft) {
    return '←';
  }
  if (key == LogicalKeyboardKey.arrowRight) {
    return '→';
  }
  // Buchstaben, Ziffern, Satzzeichen und Funktionstasten tragen ihren Namen
  // auf der Kappe: `A`, `1`, `,`, `/`, `F3`.
  return key.keyLabel.toUpperCase();
}
