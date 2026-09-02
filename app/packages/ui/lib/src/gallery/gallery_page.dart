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
import '../widgets/h_glyph.dart';
import '../widgets/h_hairline.dart';
import '../widgets/h_icon_button.dart';
import '../widgets/h_method_badge.dart';
import '../widgets/h_modal.dart';
import '../widgets/h_panel.dart';
import '../widgets/h_pill.dart';
import '../widgets/h_row.dart';
import '../widgets/h_sheet.dart';
import '../widgets/h_state_glyph.dart';

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
          _section(tokens, 'Type scale', _typeScale(tokens)),
          _section(tokens, 'Buttons', _buttons(tokens)),
          _section(tokens, 'Release valve', _pills(tokens)),
          _section(tokens, 'Badges', _badges(tokens)),
          _section(tokens, 'State glyphs', _glyphs(tokens)),
          _section(tokens, 'Rows', _rows(tokens)),
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
