import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import '../widgets/h_badge.dart';
import '../widgets/h_button.dart';
import '../widgets/h_checkbox.dart';
import '../widgets/h_focus_ring.dart';
import '../widgets/h_glyph.dart';
import '../widgets/h_hairline.dart';
import '../widgets/h_icon_button.dart';
import '../widgets/h_method_badge.dart';
import '../widgets/h_modal.dart';
import '../widgets/h_panel.dart';
import '../widgets/h_pill.dart';
import '../widgets/h_row.dart';
import '../widgets/h_segmented.dart';
import '../widgets/h_sheet.dart';
import '../widgets/h_skeleton.dart';
import '../widgets/h_state_glyph.dart';
import '../widgets/h_text_field.dart';

/// Every token and every wrapper of this package on one page, in both themes.
///
/// The gallery is a developer tool, not a product screen: its labels are
/// English literals on purpose and are never translated. It is also the later
/// basis of the golden tests of HUM-054.
class HGalleryPage extends StatefulWidget {
  /// Creates the gallery.
  const HGalleryPage({this.initialMode = HThemeMode.dark, super.key});

  /// Which theme the gallery starts in.
  final HThemeMode initialMode;

  @override
  State<HGalleryPage> createState() => _HGalleryPageState();
}

class _HGalleryPageState extends State<HGalleryPage> {
  late HThemeMode _mode = widget.initialMode;
  int _selectedRow = 1;
  bool _modalVisible = false;
  bool _sheetVisible = true;
  int _taps = 0;
  bool _loading = true;
  bool _checked = true;
  String _segment = 'allow';
  final Set<String> _chips = <String>{'GET'};
  final TextEditingController _text = TextEditingController(
    text: '**.npmjs.org',
  );
  final TextEditingController _empty = TextEditingController();

  @override
  void dispose() {
    _text.dispose();
    _empty.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final Brightness platform =
        MediaQuery.maybeOf(context)?.platformBrightness ?? Brightness.dark;
    final HTokens tokens = _mode.resolve(platform);
    return Directionality(
      textDirection: TextDirection.ltr,
      child: HTheme(
        tokens: tokens,
        child: ColoredBox(
          color: tokens.colors.bg0,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              _header(tokens),
              const HHairline(),
              Expanded(child: _body(tokens)),
              const HHairline(),
              _statusBar(tokens),
            ],
          ),
        ),
      ),
    );
  }

  Widget _header(HTokens tokens) {
    return SizedBox(
      height: tokens.sizes.headerBar,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: HSpace.x3),
        child: Row(
          children: <Widget>[
            Text(
              'Humanitl · Airlock tokens',
              style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
            ),
            const Spacer(),
            HButton(
              variant: _mode == HThemeMode.dark
                  ? HButtonVariant.primary
                  : HButtonVariant.ghost,
              onPressed: () => setState(() => _mode = HThemeMode.dark),
              child: const Text('Dark'),
            ),
            const SizedBox(width: HSpace.x2),
            HButton(
              variant: _mode == HThemeMode.light
                  ? HButtonVariant.primary
                  : HButtonVariant.ghost,
              onPressed: () => setState(() => _mode = HThemeMode.light),
              child: const Text('Light'),
            ),
            const SizedBox(width: HSpace.x2),
            HButton(
              variant: _mode == HThemeMode.system
                  ? HButtonVariant.primary
                  : HButtonVariant.ghost,
              onPressed: () => setState(() => _mode = HThemeMode.system),
              child: const Text('System'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _statusBar(HTokens tokens) {
    return SizedBox(
      height: tokens.sizes.statusBar,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: HSpace.x3),
        child: Row(
          children: <Widget>[
            Text(
              'brightness ${tokens.brightness.name} · taps $_taps',
              style: tokens.typography.ui11.tinted(tokens.colors.fg2),
            ),
          ],
        ),
      ),
    );
  }

  Widget _body(HTokens tokens) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(HSpace.x4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          _section(tokens, 'Neutral ladder', _neutralSwatches(tokens)),
          _section(tokens, 'State colours', _stateSwatches(tokens)),
          _section(tokens, 'State colours as text', _stateTextRows(tokens)),
          _section(tokens, 'Type scale', _typeScale(tokens)),
          _section(tokens, 'Buttons', _buttons(tokens)),
          _section(tokens, 'Release valve', _pills(tokens)),
          _section(tokens, 'Badges', _badges(tokens)),
          _section(tokens, 'State glyphs', _glyphs(tokens)),
          _section(tokens, 'Rows', _rows(tokens)),
          _section(tokens, 'Row densities and slots', _rowVariants(tokens)),
          _section(tokens, 'Focus ring', _focusRings(tokens)),
          _section(tokens, 'Form controls', _formControls(tokens)),
          _section(tokens, 'Waiting', _waiting(tokens)),
          _section(tokens, 'Glyphs', _allGlyphs(tokens)),
          _section(tokens, 'Panel', _panel(tokens)),
          _section(tokens, 'Sheet and modal', _overlays(tokens)),
        ],
      ),
    );
  }

  Widget _section(HTokens tokens, String title, Widget child) {
    return Padding(
      padding: const EdgeInsets.only(bottom: HSpace.x8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            title,
            style: tokens.typography.ui16.semibold.tinted(tokens.colors.fg0),
          ),
          const SizedBox(height: HSpace.x1),
          const HHairline(),
          const SizedBox(height: HSpace.x3),
          child,
        ],
      ),
    );
  }

  Widget _swatch(HTokens tokens, String name, Color color) {
    return SizedBox(
      width: 132,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Container(
            height: 44,
            decoration: BoxDecoration(
              color: color,
              border: Border.all(color: tokens.colors.line),
              borderRadius: HRadius.controlRadius,
            ),
          ),
          const SizedBox(height: HSpace.x1),
          Text(name, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
          Text(
            HColorDerivation.toHex(color),
            style: tokens.typography.mono11.tinted(tokens.colors.fg2),
          ),
        ],
      ),
    );
  }

  Widget _neutralSwatches(HTokens tokens) {
    final HSurfaceColors c = tokens.colors;
    return Wrap(
      spacing: HSpace.x3,
      runSpacing: HSpace.x3,
      children: <Widget>[
        _swatch(tokens, 'bg-0', c.bg0),
        _swatch(tokens, 'bg-1', c.bg1),
        _swatch(tokens, 'bg-2', c.bg2),
        _swatch(tokens, 'bg-3', c.bg3),
        _swatch(tokens, 'line', c.line),
        _swatch(tokens, 'line-strong', c.lineStrong),
        _swatch(tokens, 'fg-0', c.fg0),
        _swatch(tokens, 'fg-1', c.fg1),
        _swatch(tokens, 'fg-2', c.fg2),
        _swatch(tokens, 'accent', c.accent),
      ],
    );
  }

  Widget _stateSwatches(HTokens tokens) {
    return Wrap(
      spacing: HSpace.x3,
      runSpacing: HSpace.x3,
      children: <Widget>[
        for (final HFlowState state in HFlowState.values)
          _swatch(tokens, state.l10nKey, tokens.stateColor(state)),
      ],
    );
  }

  Widget _typeScale(HTokens tokens) {
    final Map<String, TextStyle> styles = <String, TextStyle>{
      'ui20 / 28': tokens.typography.ui20,
      'ui16 / 24': tokens.typography.ui16,
      'ui14 / 22': tokens.typography.ui14,
      'ui13 / 20': tokens.typography.ui13,
      'ui12 / 16': tokens.typography.ui12,
      'ui11 / 16': tokens.typography.ui11,
      'mono14 / 22': tokens.typography.mono14,
      'mono13 / 20': tokens.typography.mono13,
      'mono12 / 16': tokens.typography.mono12,
      'mono11 / 16': tokens.typography.mono11,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        for (final MapEntry<String, TextStyle> entry in styles.entries)
          Padding(
            padding: const EdgeInsets.only(bottom: HSpace.x2),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.baseline,
              textBaseline: TextBaseline.alphabetic,
              children: <Widget>[
                SizedBox(
                  width: 96,
                  child: Text(
                    entry.key,
                    style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                  ),
                ),
                Expanded(
                  child: Text(
                    'GET api.github.com/repos 0123456789',
                    style: entry.value.tinted(tokens.colors.fg0),
                  ),
                ),
                Text(
                  '400 500 600',
                  style: entry.value.semibold.tinted(tokens.colors.fg1),
                ),
              ],
            ),
          ),
      ],
    );
  }

  /// One row of the four variants per size and per interaction state, so that
  /// hover, press and focus are visible without a pointer and can be captured
  /// by a golden. The rows are `enabled`, `hovered`, `pressed`, `focused` and
  /// `disabled`; the label of each row names size and state.
  Widget _buttons(HTokens tokens) {
    const List<(String, HButtonPreview?, bool)> rows =
        <(String, HButtonPreview?, bool)>[
          ('enabled', null, true),
          ('hovered', HButtonPreview.hovered, true),
          ('pressed', HButtonPreview.pressed, true),
          ('focused', HButtonPreview.focused, true),
          ('disabled', null, false),
        ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        for (final HButtonSize size in HButtonSize.values)
          for (final (String label, HButtonPreview? preview, bool enabled)
              in rows)
            Padding(
              padding: const EdgeInsets.only(bottom: HSpace.x2),
              child: Wrap(
                spacing: HSpace.x2,
                runSpacing: HSpace.x2,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  SizedBox(
                    width: 96,
                    child: Text(
                      '${size.name} · $label',
                      style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                    ),
                  ),
                  for (final HButtonVariant variant in HButtonVariant.values)
                    HButton(
                      variant: variant,
                      size: size,
                      preview: preview,
                      onPressed: enabled ? () => setState(() => _taps++) : null,
                      child: Text(variant.name),
                    ),
                  if (preview == null && enabled)
                    HButton(
                      size: size,
                      leading: HGlyphIcon(
                        HGlyph.bolt,
                        size: 14,
                        color: tokens.colors.fg1,
                      ),
                      onPressed: () => setState(() => _taps++),
                      child: const Text('leading'),
                    ),
                ],
              ),
            ),
      ],
    );
  }

  Widget _pills(HTokens tokens) {
    return Wrap(
      spacing: HSpace.x3,
      runSpacing: HSpace.x3,
      children: <Widget>[
        HPill(
          left: const Text('Allow'),
          onLeft: () => setState(() => _taps++),
          onRight: () => setState(() => _taps++),
          onLeftLongPress: () => setState(() => _taps += 10),
          leftSemanticsLabel: 'allow once',
          rightSemanticsLabel: 'allow scope',
        ),
        HPill(
          left: const Text('Block'),
          accent: tokens.state.blocked,
          onLeft: () => setState(() => _taps++),
          onRight: () => setState(() => _taps++),
          leftSemanticsLabel: 'block once',
          rightSemanticsLabel: 'block scope',
        ),
      ],
    );
  }

  Widget _badges(HTokens tokens) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Wrap(
          spacing: HSpace.x2,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            for (final HFlowState state in HFlowState.values)
              HBadge(text: state.name, color: tokens.stateColor(state)),
          ],
        ),
        const SizedBox(height: HSpace.x3),
        Wrap(
          spacing: HSpace.x2,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            for (final String method in <String>[
              'GET',
              'HEAD',
              'POST',
              'PUT',
              'PATCH',
              'DELETE',
              'PROPFIND',
            ])
              HMethodBadge(method: method),
          ],
        ),
        const SizedBox(height: HSpace.x2),
        Text(
          'neutral, for lists: fg1 on bg2 (docs/UX.md 3.3, rule 4)',
          style: tokens.typography.mono11.tinted(tokens.colors.fg2),
        ),
        const SizedBox(height: HSpace.x1),
        Wrap(
          spacing: HSpace.x2,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            for (final String method in <String>[
              'GET',
              'POST',
              'PUT',
              'DELETE',
              'PROPFIND',
            ])
              HMethodBadge(method: method, neutral: true),
          ],
        ),
        const SizedBox(height: HSpace.x3),
        Wrap(
          spacing: HSpace.x2,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            HBadge(
              text: '3 findings',
              color: tokens.state.error,
              onTap: () => setState(() => _taps++),
              semanticsLabel: 'three findings, open them',
            ),
            const HBadge(text: 'tls 1.3', mono: true),
          ],
        ),
      ],
    );
  }

  Widget _glyphs(HTokens tokens) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Wrap(
          spacing: HSpace.x4,
          runSpacing: HSpace.x3,
          children: <Widget>[
            for (final HFlowState state in HFlowState.values)
              Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  HStateGlyph(state: state, size: 20),
                  const SizedBox(height: HSpace.x1),
                  Text(
                    state.name,
                    style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                  ),
                ],
              ),
          ],
        ),
        const SizedBox(height: HSpace.x4),
        Wrap(
          spacing: HSpace.x4,
          children: <Widget>[
            for (final double progress in <double>[1.0, 0.5, 0.15])
              Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  HStateGlyph(
                    state: HFlowState.held,
                    size: 24,
                    progress: progress,
                  ),
                  const SizedBox(height: HSpace.x1),
                  Text(
                    '${(progress * 100).round()} %',
                    style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                  ),
                ],
              ),
          ],
        ),
      ],
    );
  }

  /// Fläche gegen Text: dieselbe Zustandsfarbe zweimal, links als Tönung mit
  /// dem Wort darauf, rechts als Fläche. Die Zahl darunter ist der gemessene
  /// Kontrast des Wortes auf seiner eigenen Tönung.
  Widget _stateTextRows(HTokens tokens) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        for (final HFlowState state in HFlowState.values)
          Padding(
            padding: const EdgeInsets.only(bottom: HSpace.x2),
            child: Row(
              children: <Widget>[
                SizedBox(
                  width: 132,
                  child: Text(
                    state.name,
                    style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                  ),
                ),
                HBadge(
                  text: state.name,
                  color: tokens.stateColor(state),
                  textColor: tokens.stateTextColor(state),
                ),
                const SizedBox(width: HSpace.x3),
                Text(
                  'area ${HColorDerivation.toHex(tokens.stateColor(state))} · '
                  'text ${HColorDerivation.toHex(tokens.stateTextColor(state))}'
                  ' · '
                  '${HColorDerivation.worstTextContrast(tokens.stateTextColor(state), tokens.stateColor(state), tokens.colors.ladder).toStringAsFixed(2)}:1',
                  style: tokens.typography.mono11.tinted(tokens.colors.fg1),
                ),
              ],
            ),
          ),
      ],
    );
  }

  /// Die drei Dichten, der Aktionsslot, das Zustands-Glyph, die getönte Rail
  /// und die Mehrfachauswahl.
  Widget _rowVariants(HTokens tokens) {
    Widget labelled(String label, Widget row) => Padding(
      padding: const EdgeInsets.only(bottom: HSpace.x2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            label,
            style: tokens.typography.mono11.tinted(tokens.colors.fg2),
          ),
          DecoratedBox(
            decoration: BoxDecoration(
              border: Border.all(color: tokens.colors.line),
            ),
            child: row,
          ),
        ],
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        labelled(
          'row 36 · state glyph · tinted rail · action slot',
          HRow(
            state: HFlowState.held,
            tintedRail: true,
            onTap: () => setState(() => _taps++),
            stateGlyph: const HStateGlyph(
              state: HFlowState.held,
              progress: 0.4,
            ),
            leading: const HMethodBadge(method: 'GET', neutral: true),
            title: const Text('registry.npmjs.org'),
            trailing: const HBadge(text: '1:47'),
            actionSlot: HIconButton(
              glyph: HGlyph.shieldX,
              onPressed: () => setState(() => _taps++),
              semanticsLabel: 'block',
            ),
            semanticsLabel: 'held flow',
            semanticsValue: '1:47 left',
          ),
        ),
        labelled(
          'rowHistory 28 · full saturation',
          HRow(
            state: HFlowState.blocked,
            minHeight: HSize.rowHistory,
            onTap: () => setState(() => _taps++),
            stateGlyph: const HStateGlyph(state: HFlowState.blocked),
            leading: const HMethodBadge(method: 'DELETE', neutral: true),
            title: const Text('telemetry.example.com'),
            semanticsLabel: 'blocked flow',
          ),
        ),
        labelled(
          'rowBody 24',
          HRow(
            state: HFlowState.allowed,
            minHeight: HSize.rowBody,
            title: Text(
              '{"token": "…"}',
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            ),
            semanticsLabel: 'body line',
          ),
        ),
        labelled(
          'in a multi selection, without the cursor',
          Column(
            children: <Widget>[
              HRow(
                state: HFlowState.held,
                inSelection: true,
                tintedRail: true,
                onTap: () => setState(() => _taps++),
                stateGlyph: const HStateGlyph(state: HFlowState.held),
                title: const Text('api.github.com'),
                semanticsLabel: 'member of the selection',
              ),
              HRow(
                state: HFlowState.held,
                inSelection: true,
                selected: true,
                tintedRail: true,
                onTap: () => setState(() => _taps++),
                stateGlyph: const HStateGlyph(state: HFlowState.held),
                title: const Text('api.github.com'),
                semanticsLabel: 'member carrying the cursor',
              ),
            ],
          ),
        ),
      ],
    );
  }

  /// Der Fokusring an jeder Form, die es gibt: reservierter Platz um ein
  /// Control, auf der Kante einer Zeile.
  Widget _focusRings(HTokens tokens) {
    return Wrap(
      spacing: HSpace.x4,
      runSpacing: HSpace.x3,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: <Widget>[
        Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'button, focused',
              style: tokens.typography.mono11.tinted(tokens.colors.fg2),
            ),
            HButton(
              variant: HButtonVariant.primary,
              preview: HButtonPreview.focused,
              onPressed: () => setState(() => _taps++),
              child: const Text('Send'),
            ),
          ],
        ),
        Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'ring around a control',
              style: tokens.typography.mono11.tinted(tokens.colors.fg2),
            ),
            HFocusRing(
              visible: true,
              radius: tokens.radii.control,
              child: HBadge(text: 'chip', color: tokens.colors.fg1),
            ),
          ],
        ),
        SizedBox(
          width: 260,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                'ring on the edge of a row',
                style: tokens.typography.mono11.tinted(tokens.colors.fg2),
              ),
              HFocusRing.inline(
                visible: true,
                child: HRow(
                  state: HFlowState.allowed,
                  title: const Text('api.github.com'),
                  semanticsLabel: 'focused row',
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  /// Eingabefeld, Segmente, Chips und Kästchen, jeweils in jedem Zustand.
  Widget _formControls(HTokens tokens) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SizedBox(
          width: 320,
          child: HTextField(controller: _text, semanticsLabel: 'host pattern'),
        ),
        const SizedBox(height: HSpace.x2),
        SizedBox(
          width: 320,
          child: HTextField(
            controller: _empty,
            semanticsLabel: 'port',
            hint: '443',
            digitsOnly: true,
          ),
        ),
        const SizedBox(height: HSpace.x2),
        SizedBox(
          width: 320,
          child: HTextField(
            controller: _text,
            semanticsLabel: 'bundled rule',
            enabled: false,
          ),
        ),
        const SizedBox(height: HSpace.x3),
        HSegmented<String>(
          selected: _segment,
          onSelect: (String value) => setState(() => _segment = value),
          options: <HSegmentOption<String>>[
            HSegmentOption<String>(
              value: 'allow',
              label: 'allow',
              leading: HGlyphIcon(
                HGlyph.arrowUpRight,
                size: 12,
                color: tokens.stateTextColor(HFlowState.allowed),
              ),
            ),
            HSegmentOption<String>(
              value: 'block',
              label: 'block',
              leading: HGlyphIcon(
                HGlyph.shieldX,
                size: 12,
                color: tokens.stateTextColor(HFlowState.blocked),
              ),
            ),
            HSegmentOption<String>(
              value: 'redact',
              label: 'redact',
              leading: HGlyphIcon(
                HGlyph.redactBar,
                size: 12,
                color: tokens.stateTextColor(HFlowState.passthroughLlm),
              ),
            ),
          ],
        ),
        const SizedBox(height: HSpace.x2),
        HSegmented<String>(
          selected: _segment,
          enabled: false,
          onSelect: (String value) => setState(() => _segment = value),
          options: const <HSegmentOption<String>>[
            HSegmentOption<String>(value: 'allow', label: 'allow'),
            HSegmentOption<String>(value: 'block', label: 'block'),
          ],
        ),
        const SizedBox(height: HSpace.x3),
        HChoiceChips<String>(
          selected: _chips,
          onToggle: (String value) => setState(() {
            if (!_chips.remove(value)) {
              _chips.add(value);
            }
          }),
          options: const <HSegmentOption<String>>[
            HSegmentOption<String>(value: 'GET', label: 'GET'),
            HSegmentOption<String>(value: 'POST', label: 'POST'),
            HSegmentOption<String>(value: 'DELETE', label: 'DELETE'),
          ],
        ),
        const SizedBox(height: HSpace.x3),
        SizedBox(
          width: 420,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              HCheckbox(
                label: 'Remember for this session',
                value: _checked,
                onChanged: (bool value) => setState(() => _checked = value),
              ),
              HCheckbox(
                label: 'Keep the rule after the session ends',
                hint:
                    'A rule that outlives the session is one nobody sees '
                    'again.',
                value: !_checked,
                onChanged: (bool value) => setState(() => _checked = !value),
              ),
              HCheckbox(
                label: 'Bundled rule',
                value: true,
                enabled: false,
                onChanged: (bool value) {},
              ),
            ],
          ),
        ),
      ],
    );
  }

  /// Das Skelett in den drei Dichten und der Wartezustand darüber.
  Widget _waiting(HTokens tokens) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        HButton(
          onPressed: () => setState(() => _loading = !_loading),
          child: Text(_loading ? 'Answer arrives' : 'Wait again'),
        ),
        const SizedBox(height: HSpace.x3),
        SizedBox(
          width: 360,
          child: HWait(
            loading: _loading,
            skeleton: const HSkeleton(rows: 3),
            child: Text(
              'three rules matched',
              style: tokens.typography.ui13.tinted(tokens.colors.fg0),
            ),
          ),
        ),
        const SizedBox(height: HSpace.x3),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            for (final (String label, double height) in <(String, double)>[
              ('row 36', HSize.row),
              ('rowHistory 28', HSize.rowHistory),
              ('rowBody 24', HSize.rowBody),
            ])
              SizedBox(
                width: 200,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      label,
                      style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                    ),
                    HSkeleton(rows: 3, rowHeight: height),
                  ],
                ),
              ),
          ],
        ),
      ],
    );
  }

  /// Jedes Glyph des Systems, auch die fünf, die die Screens bisher selbst
  /// gemalt haben.
  Widget _allGlyphs(HTokens tokens) {
    return Wrap(
      spacing: HSpace.x4,
      runSpacing: HSpace.x3,
      children: <Widget>[
        for (final HGlyph glyph in HGlyph.values)
          Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              HGlyphIcon(glyph, size: 20, color: tokens.colors.fg1),
              const SizedBox(height: HSpace.x1),
              Text(
                glyph.name,
                style: tokens.typography.mono11.tinted(tokens.colors.fg2),
              ),
            ],
          ),
      ],
    );
  }

  Widget _rows(HTokens tokens) {
    const List<(HFlowState, String, String)>
    data = <(HFlowState, String, String)>[
      (HFlowState.held, 'registry.npmjs.org', '/left-pad/-/left-pad-1.3.0.tgz'),
      (HFlowState.allowedEdited, 'api.github.com', '/repos/humanitl/humanitl'),
      (HFlowState.blocked, 'telemetry.example.com', '/v1/collect'),
    ];
    return DecoratedBox(
      decoration: BoxDecoration(border: Border.all(color: tokens.colors.line)),
      child: Column(
        children: <Widget>[
          for (int i = 0; i < data.length; i++)
            HRow(
              state: data[i].$1,
              selected: _selectedRow == i,
              onTap: () => setState(() => _selectedRow = i),
              leading: HStateGlyph(
                state: data[i].$1,
                progress: data[i].$1 == HFlowState.held ? 0.4 : null,
              ),
              title: Text(data[i].$2),
              subtitle: Text(data[i].$3),
              trailing: const HMethodBadge(method: 'GET'),
              semanticsLabel: data[i].$2,
            ),
        ],
      ),
    );
  }

  Widget _panel(HTokens tokens) {
    return SizedBox(
      width: 420,
      child: HPanel(
        title: const Text('Isolation'),
        actions: <Widget>[
          HIconButton(
            glyph: HGlyph.close,
            onPressed: () => setState(() => _taps++),
            semanticsLabel: 'dismiss panel',
          ),
        ],
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'no network interface · single socket · seccomp active',
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            ),
            const SizedBox(height: HSpace.x2),
            Text(
              'panel padding ${HSpace.panelPadding.toInt()} · radius '
              '${tokens.radii.panel.toInt()} · hairline border',
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
          ],
        ),
      ),
    );
  }

  Widget _overlays(HTokens tokens) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Wrap(
          spacing: HSpace.x2,
          children: <Widget>[
            HButton(
              onPressed: () => setState(() => _modalVisible = !_modalVisible),
              child: const Text('Toggle modal'),
            ),
            HButton(
              onPressed: () => setState(() => _sheetVisible = !_sheetVisible),
              child: const Text('Toggle sheet'),
            ),
          ],
        ),
        const SizedBox(height: HSpace.x3),
        SizedBox(
          height: 260,
          child: DecoratedBox(
            decoration: BoxDecoration(
              border: Border.all(color: tokens.colors.line),
            ),
            child: Stack(
              children: <Widget>[
                Positioned.fill(
                  child: ColoredBox(
                    color: tokens.colors.bg1,
                    child: Center(
                      child: Text(
                        'shell content',
                        style: tokens.typography.ui13.tinted(tokens.colors.fg2),
                      ),
                    ),
                  ),
                ),
                if (_sheetVisible)
                  Positioned(
                    top: 0,
                    bottom: 0,
                    right: 0,
                    child: HSheet(
                      title: const Text('Rule from request'),
                      onClose: () => setState(() => _sheetVisible = false),
                      closeSemanticsLabel: 'close sheet',
                      width: 300,
                      child: Text(
                        'allow · GET · **.npmjs.org · session',
                        style: tokens.typography.mono12.tinted(
                          tokens.colors.fg1,
                        ),
                      ),
                    ),
                  ),
                if (_modalVisible)
                  HModal(
                    title: const Text('Delete forever rule?'),
                    onDismiss: () => setState(() => _modalVisible = false),
                    scrimSemanticsLabel: 'dismiss modal',
                    width: 320,
                    actions: <Widget>[
                      HButton(
                        variant: HButtonVariant.ghost,
                        onPressed: () => setState(() => _modalVisible = false),
                        child: const Text('Cancel'),
                      ),
                      HButton(
                        variant: HButtonVariant.danger,
                        onPressed: () => setState(() => _modalVisible = false),
                        child: const Text('Delete'),
                      ),
                    ],
                    child: const Text(
                      'The rule allows every request to **.npmjs.org.',
                    ),
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(height: HSpace.x3),
        Text(
          'motion: enter ${HMotion.enter} · arrive '
          '${HMotion.arrive.inMilliseconds} ms · press '
          '${HMotion.press.inMilliseconds} ms · hold '
          '${HMotion.holdToConfirm.inMilliseconds} ms',
          style: tokens.typography.mono11.tinted(tokens.colors.fg2),
        ),
      ],
    );
  }
}
