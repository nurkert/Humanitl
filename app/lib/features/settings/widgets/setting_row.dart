/// One row of the settings overlay: name and short help on the left, the
/// control on the right, every control of a page on the same right edge
/// (HUM-069, reworked by HUM-404).
library;

import 'dart:math' as math;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../registry/settings_registry.dart';
import '../settings_scope.dart';

/// The grid every row of the overlay shares.
abstract final class SettingRowGrid {
  /// The width of a control that fills its column, such as a text field.
  static const double controlWidth = 320;

  /// Below this width of the row the control moves under the text, at the
  /// left edge: two columns would leave the name a sliver.
  static const double stackBelow = 560;

  /// The least the name and its help keep beside a control; a control that
  /// would leave less moves under the text instead of squeezing it.
  static const double minText = 200;

  /// The room between the text and the control.
  static const double gutter = HSpace.x6;

  /// The inset of every row, and of the page heading above them, so that a
  /// lit row has its own margin and the text still starts on one edge.
  static const double inset = HSpace.x3;
}

/// A row with the name, help and control of the entry [settingId].
///
/// Name, help, longer explanation and whether a running session keeps the old
/// value come from the register ([settingsRegistry]), so the row says what
/// the search finds. The refusal is the daemon's own sentence (`why`), never
/// one of this form: the side that read the schema knows why a value is
/// wrong (`docs/UX.md` 4.4). It stands where the problem is, at the row.
class SettingRow extends StatefulWidget {
  /// Creates the row of [settingId] around [child].
  const SettingRow({
    required this.settingId,
    required this.child,
    this.fill = false,
    this.leading,
    this.title,
    this.help,
    this.details,
    this.status = const <Widget>[],
    this.refusal,
    this.refusalKey,
    super.key,
  });

  /// The id of the entry in the register.
  final String settingId;

  /// The control.
  final Widget child;

  /// Whether [child] takes the whole [SettingRowGrid.controlWidth], as a
  /// text field does; otherwise it keeps its own width at the right edge.
  final bool fill;

  /// A secondary action left of the control, such as "Check now" beside the
  /// switch of the update check. A jump to the setting lands on the control,
  /// not on it.
  final Widget? leading;

  /// The name in place of the register's, for a row of a form that is no
  /// entry of its own, such as the name of a new server (HUM-405). Such a row
  /// may carry an id the register does not know.
  final String? title;

  /// Help in place of the register's, for a sentence that names a value
  /// known only at runtime.
  final String? help;

  /// The longer explanation in place of the register's, likewise.
  final String? details;

  /// Lines under the help that say what the daemon made of the value.
  final List<Widget> status;

  /// The refusal of a row that writes through its own RPC; otherwise the
  /// overlay's last refusal of [settingId].
  final Diagnostic? refusal;

  /// The key of the refusal text, for tests.
  final Key? refusalKey;

  @override
  State<SettingRow> createState() => _SettingRowState();
}

class _SettingRowState extends State<SettingRow> {
  bool _detailsOpen = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SettingsController? scope = SettingsScope.maybeOf(context);
    final SettingEntry? entry = widget.title == null
        ? settingEntry(widget.settingId)
        : settingEntryOrNull(widget.settingId);
    final String title = widget.title ?? entry?.title(l10n) ?? '';
    final String? help = widget.help ?? entry?.help?.call(l10n);
    final String? details = widget.details ?? entry?.details?.call(l10n);
    final Diagnostic? refused =
        widget.refusal ?? scope?.refusal(widget.settingId);
    final bool saved = scope?.saved(widget.settingId) ?? false;
    final bool lit = scope?.highlighted == widget.settingId;

    final Widget text = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        // Die Kopfzeile ist so hoch wie ein Control, damit der Name auf der
        // Mitte des Schalters daneben steht, bei jeder Textgröße.
        ConstrainedBox(
          constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
          child: Row(
            children: <Widget>[
              Flexible(
                child: Text(
                  title,
                  style: tokens.typography.ui13.medium.tinted(
                    tokens.colors.fg0,
                  ),
                ),
              ),
              if (details != null)
                HIconButton(
                  key: ValueKey<String>('setting-info-${widget.settingId}'),
                  glyph: HGlyph.info,
                  size: 14,
                  color: _detailsOpen ? tokens.colors.fg0 : null,
                  semanticsLabel: l10n.settingsMoreInfo(title),
                  onPressed: () => setState(() => _detailsOpen = !_detailsOpen),
                ),
              _SavedMark(visible: saved, settingId: widget.settingId),
            ],
          ),
        ),
        if (help != null)
          Text(help, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
        if (entry?.nextSession ?? false)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: Text(
              l10n.settingsNextSession,
              key: ValueKey<String>('setting-next-${widget.settingId}'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
          ),
        if (details != null && _detailsOpen)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x2),
            child: Text(
              details,
              key: ValueKey<String>('setting-details-${widget.settingId}'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
        for (final Widget line in widget.status)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: line,
          ),
        if (refused != null)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: Text(
              DiagnosticL10n.resolve(refused, l10n).cause,
              key: widget.refusalKey,
              style: tokens.typography.ui12.tinted(tokens.stateText.error),
            ),
          ),
      ],
    );

    final Widget anchored = scope == null
        ? widget.child
        : Focus(focusNode: scope.anchor(widget.settingId), child: widget.child);
    final Widget control = switch (widget.leading) {
      final Widget leading => Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          leading,
          SizedBox(width: tokens.spacing.x2),
          anchored,
        ],
      ),
      null => anchored,
    };

    return AnimatedContainer(
      key: scope?.rowKey(widget.settingId),
      // Das Aufleuchten nach einem Sprung ist eine Farbe, keine Bewegung; es
      // bleibt auch unter reduzierter Bewegung (`docs/UX.md` 2.10).
      duration: HMotion.sweep,
      curve: HMotion.exit,
      decoration: BoxDecoration(
        color: HColorDerivation.fade(tokens.colors.accent, lit ? 0.12 : 0),
        borderRadius: HRadius.controlRadius,
      ),
      padding: EdgeInsets.symmetric(
        horizontal: SettingRowGrid.inset,
        vertical: tokens.spacing.x3,
      ),
      child: _SettingRowLayout(
        key: ValueKey<String>('setting-row-${widget.settingId}'),
        fill: widget.fill,
        gap: tokens.spacing.x2,
        text: text,
        control: control,
      ),
    );
  }
}

/// Text left, control at the right edge; the control under the text when
/// the two do not fit side by side.
///
/// A `Row` cannot decide that: whether they fit depends on how wide the
/// control turns out, and a segmented control with long German labels, or
/// at twice the text, is wider than any fixed column one could give it.
/// This layout measures the control first and then gives the text what is
/// left, so a control is never squeezed and never runs over the edge.
class _SettingRowLayout extends MultiChildRenderObjectWidget {
  _SettingRowLayout({
    required this.fill,
    required this.gap,
    required Widget text,
    required Widget control,
    super.key,
  }) : super(children: <Widget>[text, control]);

  final bool fill;
  final double gap;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderSettingRow(fill, gap);

  @override
  void updateRenderObject(BuildContext context, _RenderSettingRow row) {
    row
      ..fill = fill
      ..gap = gap;
  }
}

class _SettingRowParentData extends ContainerBoxParentData<RenderBox> {}

class _RenderSettingRow extends RenderBox
    with
        ContainerRenderObjectMixin<RenderBox, _SettingRowParentData>,
        RenderBoxContainerDefaultsMixin<RenderBox, _SettingRowParentData> {
  _RenderSettingRow(this._fill, this._gap);

  bool _fill;
  set fill(bool value) {
    if (value != _fill) {
      _fill = value;
      markNeedsLayout();
    }
  }

  double _gap;
  set gap(double value) {
    if (value != _gap) {
      _gap = value;
      markNeedsLayout();
    }
  }

  @override
  void setupParentData(RenderBox child) {
    if (child.parentData is! _SettingRowParentData) {
      child.parentData = _SettingRowParentData();
    }
  }

  @override
  void performLayout() {
    final RenderBox text = firstChild!;
    final RenderBox control = childAfter(text)!;
    final double width = constraints.maxWidth;
    final bool wide = width >= SettingRowGrid.stackBelow;

    // Erst das Control, mit der Breite, die es bekäme.
    if (_fill) {
      control.layout(
        BoxConstraints.tightFor(
          width: wide ? SettingRowGrid.controlWidth : width,
        ),
        parentUsesSize: true,
      );
    } else {
      control.layout(BoxConstraints(maxWidth: width), parentUsesSize: true);
    }
    final double left = width - SettingRowGrid.gutter - control.size.width;
    final bool beside = wide && left >= SettingRowGrid.minText;

    final _SettingRowParentData textAt =
        text.parentData! as _SettingRowParentData;
    final _SettingRowParentData controlAt =
        control.parentData! as _SettingRowParentData;
    if (beside) {
      text.layout(BoxConstraints(maxWidth: left), parentUsesSize: true);
      textAt.offset = Offset.zero;
      controlAt.offset = Offset(width - control.size.width, 0);
      size = constraints.constrain(
        Size(width, math.max(text.size.height, control.size.height)),
      );
      return;
    }
    // Untereinander: das Control an der linken Kante, unter dem Text.
    if (_fill) {
      control.layout(
        BoxConstraints.tightFor(width: width),
        parentUsesSize: true,
      );
    }
    text.layout(BoxConstraints(maxWidth: width), parentUsesSize: true);
    textAt.offset = Offset.zero;
    controlAt.offset = Offset(0, text.size.height + _gap);
    size = constraints.constrain(
      Size(width, text.size.height + _gap + control.size.height),
    );
  }

  @override
  void paint(PaintingContext context, Offset offset) =>
      defaultPaint(context, offset);

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) =>
      defaultHitTestChildren(result, position: position);
}

/// "Saved" beside the name, for a moment after the daemon took a value.
///
/// Hidden it takes no width at all, so a long name wraps only where it has
/// to; it fades in and out over [HMotion.arrive].
class _SavedMark extends StatelessWidget {
  const _SavedMark({required this.visible, required this.settingId});

  final bool visible;
  final String settingId;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    // Eine Blende ist Rückmeldung und keine Verschiebung; sie behält ihre
    // Dauer auch unter reduzierter Bewegung (`docs/UX.md` 2.10).
    return AnimatedSwitcher(
      duration: HMotion.arrive,
      switchInCurve: HMotion.enter,
      switchOutCurve: HMotion.exit,
      child: visible
          ? Padding(
              key: ValueKey<String>('setting-saved-$settingId'),
              padding: EdgeInsetsDirectional.only(start: tokens.spacing.x2),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  HGlyphIcon(HGlyph.check, size: 12, color: tokens.colors.fg1),
                  SizedBox(width: tokens.spacing.x1),
                  Text(
                    context.l10n.settingsSaved,
                    style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                  ),
                ],
              ),
            )
          : const SizedBox.shrink(),
    );
  }
}
