// Gibt die Token-Tabellen von `HTokens` als Markdown aus (HUM-059).
//
// `docs/DESIGN.md` nimmt seine Tabellen aus dieser Ausgabe, statt Werte von
// Hand abzuschreiben: die hellen Zustandsfarben und alle Textvarianten werden
// zur Laufzeit abgeleitet und stehen nirgends als Literal im Quelltext.
//
// Das Skript braucht `dart:ui` und läuft deshalb unter der Flutter-Engine:
//
//     cd app/packages/ui && flutter test tool/dump_tokens.dart
//
// Die Tabellen stehen zwischen den Markern `BEGIN TOKENS` und `END TOKENS`.
// Danach meldet `flutter test` „No tests ran" und endet mit Code 79; das ist
// erwartet, denn das Skript enthält keinen Testfall.
import 'package:flutter/widgets.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

/// Druckt alle Tabellen.
void main() {
  final StringBuffer out = StringBuffer()..writeln('BEGIN TOKENS');
  _surfaces(out);
  _states(out);
  _methods(out);
  _terminal(out);
  _typography(out);
  _spacing(out);
  _radii(out);
  _sizes(out);
  _motion(out);
  out.writeln('END TOKENS');
  // ignore: avoid_print
  print(out.toString());
}

String _hex(Color color) => '`${HColorDerivation.toHex(color)}`';

void _table(StringBuffer out, List<String> head, List<List<String>> rows) {
  out
    ..writeln()
    ..writeln('| ${head.join(' | ')} |')
    ..writeln('|${head.map((String _) => '---').join('|')}|');
  for (final List<String> row in rows) {
    out.writeln('| ${row.join(' | ')} |');
  }
  out.writeln();
}

void _surfaces(StringBuffer out) {
  final HSurfaceColors d = HTokens.dark.colors;
  final HSurfaceColors l = HTokens.light.colors;
  List<String> row(String name, Color Function(HSurfaceColors c) pick) =>
      <String>['`$name`', _hex(pick(d)), _hex(pick(l))];
  out.writeln('### surfaces');
  _table(
    out,
    <String>['Token', 'Dark', 'Light'],
    <List<String>>[
      row('bg0', (HSurfaceColors c) => c.bg0),
      row('bg1', (HSurfaceColors c) => c.bg1),
      row('bg2', (HSurfaceColors c) => c.bg2),
      row('bg3', (HSurfaceColors c) => c.bg3),
      row('line', (HSurfaceColors c) => c.line),
      row('lineStrong', (HSurfaceColors c) => c.lineStrong),
      row('fg0', (HSurfaceColors c) => c.fg0),
      row('fg1', (HSurfaceColors c) => c.fg1),
      row('fg2', (HSurfaceColors c) => c.fg2),
      row('accent', (HSurfaceColors c) => c.accent),
      row('accentText', (HSurfaceColors c) => c.accentText),
      row('accentFill', (HSurfaceColors c) => c.accentFill),
      row('onAccent', (HSurfaceColors c) => c.onAccent),
      row('warning', (HSurfaceColors c) => c.warning),
      row('warningText', (HSurfaceColors c) => c.warningText),
      row('info', (HSurfaceColors c) => c.info),
      row('infoText', (HSurfaceColors c) => c.infoText),
    ],
  );
}

void _states(StringBuffer out) {
  out.writeln('### states');
  _table(
    out,
    <String>['State', 'Dark area', 'Dark text', 'Light area', 'Light text'],
    <List<String>>[
      for (final HFlowState s in HFlowState.values)
        <String>[
          '`${s.name}`',
          _hex(HTokens.dark.stateColor(s)),
          _hex(HTokens.dark.stateTextColor(s)),
          _hex(HTokens.light.stateColor(s)),
          _hex(HTokens.light.stateTextColor(s)),
        ],
    ],
  );
}

void _methods(StringBuffer out) {
  const List<String> names = <String>[
    'GET, HEAD',
    'POST',
    'PUT, PATCH',
    'DELETE',
    'other',
  ];
  final List<List<Color>> columns = <List<Color>>[
    HTokens.dark.method.all,
    HTokens.dark.methodText.all,
    HTokens.light.method.all,
    HTokens.light.methodText.all,
  ];
  out.writeln('### methods');
  _table(
    out,
    <String>['Method', 'Dark area', 'Dark text', 'Light area', 'Light text'],
    <List<String>>[
      for (int i = 0; i < names.length; i++)
        <String>[
          names[i],
          for (final List<Color> column in columns) _hex(column[i]),
        ],
    ],
  );
}

void _terminal(StringBuffer out) {
  final HTerminalPalette d = HTokens.dark.terminal;
  final HTerminalPalette l = HTokens.light.terminal;
  const List<String> ansi = <String>[
    'black',
    'red',
    'green',
    'yellow',
    'blue',
    'magenta',
    'cyan',
    'white',
  ];
  out.writeln('### terminal');
  _table(
    out,
    <String>['Slot', 'Dark', 'Light'],
    <List<String>>[
      <String>['background', _hex(d.background), _hex(l.background)],
      <String>['foreground', _hex(d.foreground), _hex(l.foreground)],
      <String>['cursor', _hex(d.cursor), _hex(l.cursor)],
      <String>['selection', _hex(d.selection), _hex(l.selection)],
      for (int i = 0; i < hTerminalRamp; i++)
        <String>[ansi[i], _hex(d.normal[i]), _hex(l.normal[i])],
      for (int i = 0; i < hTerminalRamp; i++)
        <String>['bright ${ansi[i]}', _hex(d.bright[i]), _hex(l.bright[i])],
    ],
  );
}

String _px(double value) =>
    value == value.roundToDouble() ? '${value.round()}' : '$value';

void _typography(StringBuffer out) {
  final HTypography t = HTokens.dark.typography;
  final Map<String, TextStyle> styles = <String, TextStyle>{
    'ui11': t.ui11,
    'ui12': t.ui12,
    'ui13': t.ui13,
    'ui14': t.ui14,
    'ui16': t.ui16,
    'ui20': t.ui20,
    'mono11': t.mono11,
    'mono12': t.mono12,
    'mono13': t.mono13,
    'mono14': t.mono14,
  };
  out.writeln('### typography');
  _table(
    out,
    <String>['Token', 'Family', 'Size/line (px)', 'Weight'],
    <List<String>>[
      for (final MapEntry<String, TextStyle> e in styles.entries)
        <String>[
          '`${e.key}`',
          e.value.fontFamily ?? '',
          '${_px(e.value.fontSize!)}/'
              '${(e.value.fontSize! * e.value.height!).round()}',
          '${e.value.fontWeight!.value}',
        ],
    ],
  );
}

void _spacing(StringBuffer out) {
  final HSpacingTokens s = HTokens.dark.spacing;
  out.writeln('### spacing');
  _table(
    out,
    <String>['Token', 'px'],
    <List<String>>[
      <String>['`unit`', _px(s.unit)],
      <String>['`x1`', _px(s.x1)],
      <String>['`x2`', _px(s.x2)],
      <String>['`x3`', _px(s.x3)],
      <String>['`x4`', _px(s.x4)],
      <String>['`x5`', _px(s.x5)],
      <String>['`x6`', _px(s.x6)],
      <String>['`x7`', _px(s.x7)],
      <String>['`x8`', _px(s.x8)],
      <String>['`panelPadding`', _px(HSpace.panelPadding)],
    ],
  );
}

void _radii(StringBuffer out) {
  final HRadiusTokens r = HTokens.dark.radii;
  out.writeln('### radii');
  _table(
    out,
    <String>['Token', 'px'],
    <List<String>>[
      <String>['`control`', _px(r.control)],
      <String>['`card`', _px(r.card)],
      <String>['`panel`', _px(r.panel)],
      <String>['`badge`', _px(r.badge)],
    ],
  );
}

void _sizes(StringBuffer out) {
  final HSizeTokens s = HTokens.dark.sizes;
  final (int q, int i, int c) = s.paneRatio;
  out.writeln('### sizes');
  _table(
    out,
    <String>['Token', 'Value'],
    <List<String>>[
      <String>['`headerBar`', _px(s.headerBar)],
      <String>['`statusBar`', _px(s.statusBar)],
      <String>['`row`', _px(s.row)],
      <String>['`rowSelected`', _px(s.rowSelected)],
      <String>['`rowHistory`', _px(s.rowHistory)],
      <String>['`rowBody`', _px(s.rowBody)],
      <String>['`rowActionSlot`', _px(s.rowActionSlot)],
      <String>['`hitMin`', _px(s.hitMin)],
      <String>[
        '`hitDecision`',
        '${_px(s.hitDecision.width)} x ${_px(s.hitDecision.height)}',
      ],
      <String>['`paneMinQueue`', _px(s.paneMinQueue)],
      <String>['`paneMinInspector`', _px(s.paneMinInspector)],
      <String>['`paneMinContext`', _px(s.paneMinContext)],
      <String>['`paneRatio`', '$q/$i/$c'],
      <String>['`measureChars`', '${s.measureChars}'],
      <String>['`stateRail`', _px(HSize.stateRail)],
      <String>['`selectionRail`', _px(HSize.selectionRail)],
      <String>['`cautionTape`', _px(HSize.cautionTape)],
      <String>['`hairline`', _px(HSize.hairline)],
      <String>['`splitter`', _px(HSize.splitter)],
      <String>['`glyph`', _px(HSize.glyph)],
      <String>['`tick`', _px(HSize.tick)],
      <String>['`ringStroke`', _px(HSize.ringStroke)],
    ],
  );
}

String _cubic(Cubic c) => 'cubic-bezier(${c.a}, ${c.b}, ${c.c}, ${c.d})';

void _motion(StringBuffer out) {
  final HMotionTokens m = HTokens.dark.motion;
  String ms(Duration d) => '${d.inMilliseconds} ms';
  out.writeln('### motion');
  _table(
    out,
    <String>['Token', 'Value'],
    <List<String>>[
      <String>['`enter`', _cubic(m.enter)],
      <String>['`exit`', _cubic(m.exit)],
      <String>['`arrive`', ms(m.arrive)],
      <String>['`press`', ms(m.press)],
      <String>['`sweep`', ms(m.sweep)],
      <String>['`leave`', ms(m.leave)],
      <String>['`ruleDraw`', ms(m.ruleDraw)],
      <String>['`breathe`', ms(m.breathe)],
      <String>['`holdToConfirm`', ms(m.holdToConfirm)],
      <String>['`holdToBlock`', ms(m.holdToBlock)],
      <String>['`stagger`', ms(m.stagger)],
      <String>['`rearm`', ms(HMotion.rearm)],
      <String>['`checkStagger`', ms(HMotion.checkStagger)],
      <String>['`cautionTape`', ms(HMotion.cautionTape)],
      <String>['`arriveOffset`', '${_px(HMotion.arriveOffset)} px'],
      <String>['`leaveOffset`', '${_px(HMotion.leaveOffset)} px'],
      <String>['`breatheBelow`', '${HMotion.breatheBelow}'],
      <String>['`breatheMinOpacity`', '${HMotion.breatheMinOpacity}'],
    ],
  );
}
