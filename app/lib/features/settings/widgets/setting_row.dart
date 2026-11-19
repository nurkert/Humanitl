/// One row of the settings overlay: name and short help on the left, the
/// control on the right, every control of a page on the same right edge
/// (HUM-069, reworked by HUM-404).
library;

import 'dart:math' as math;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

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
/// value come from the catalog of the daemon ([SettingsView], HUM-534), so
/// the row says what the search finds. A value a company policy locks is
/// read only: the control takes neither pointer nor focus, and the row names
/// the policy. The refusal is the daemon's own sentence (`why`), never
/// one of this form: the side that read the schema knows why a value is
/// wrong (`docs/UX.md` 4.4). It stands where the problem is, at the row.
class SettingRow extends ConsumerStatefulWidget {
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
    this.footer = const <Widget>[],
    this.refusal,
    this.refusalKey,
    super.key,
  });

  /// The id of the entry in the catalog.
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

  /// Lines under name, help and control together, across the whole width of
  /// the row (HUM-494): for an explanation too long for the column beside a
  /// wide control, such as the reasons and commands under the choice of the
  /// sandbox backend. [status] stays in the text column; a line there next
  /// to a control of four segments keeps about a third of the row.
  final List<Widget> footer;

  /// The refusal of a row that writes through its own RPC; otherwise the
  /// overlay's last refusal of [settingId].
  final Diagnostic? refusal;

  /// The key of the refusal text, for tests.
  final Key? refusalKey;

  @override
  ConsumerState<SettingRow> createState() => _SettingRowState();
}

class _SettingRowState extends ConsumerState<SettingRow> {
  bool _detailsOpen = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SettingsController? scope = SettingsScope.maybeOf(context);
    final SettingEntry? entry = SettingsView.of(
      context,
      ref,
    ).entry(widget.settingId);
    final String title = widget.title ?? entry?.title ?? '';
    final String? help = widget.help ?? entry?.help;
    final String? details = widget.details ?? entry?.details;
    final SettingsLock? lock = entry?.lock;
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
        if (lock != null)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: SettingLockNote(lock: lock, settingId: widget.settingId),
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

    // Gesperrt heißt nur lesbar (HUM-534): Das Bedienelement nimmt weder
    // Zeiger noch Fokus, und die Zeile nennt die Vorgabe.
    final Widget own = lock == null
        ? widget.child
        : ReadOnlySetting(settingId: widget.settingId, child: widget.child);
    final Widget anchored = scope == null
        ? own
        : Focus(focusNode: scope.anchor(widget.settingId), child: own);
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
      child: _withFooter(
        tokens,
        _SettingRowLayout(
          key: ValueKey<String>('setting-row-${widget.settingId}'),
          fill: widget.fill,
          gap: tokens.spacing.x2,
          text: text,
          control: control,
        ),
      ),
    );
  }

  /// [row] alone, or with [SettingRow.footer] under it at full width.
  Widget _withFooter(HTokens tokens, Widget row) {
    if (widget.footer.isEmpty) {
      return row;
    }
    // Die Fußzeilen stehen unter Text und Control zugleich, so breit wie die
    // Zeile: Neben einem breiten Control bliebe ihnen nur ein Streifen.
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        row,
        for (final Widget line in widget.footer)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x2),
            child: line,
          ),
      ],
    );
  }
}

/// The sentence under a setting a company policy locks (HUM-534).
class SettingLockNote extends StatelessWidget {
  /// Creates the note for [lock] of [settingId].
  const SettingLockNote({
    required this.lock,
    required this.settingId,
    super.key,
  });

  /// The lock.
  final SettingsLock lock;

  /// The id of the locked entry.
  final String settingId;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Text(
      context.l10n.settingsLockedBy(lock.lockedBy),
      key: ValueKey<String>('setting-lock-$settingId'),
      style: tokens.typography.ui12.tinted(tokens.colors.fg1),
    );
  }
}

/// A control that takes neither pointer nor focus: a value a company policy
/// locks is read only in every client (HUM-534).
class ReadOnlySetting extends StatelessWidget {
  /// Creates the read-only wrapper of [child].
  const ReadOnlySetting({
    required this.settingId,
    required this.child,
    super.key,
  });

  /// The id of the locked entry.
  final String settingId;

  /// The control.
  final Widget child;

  @override
  Widget build(BuildContext context) => Semantics(
    readOnly: true,
    child: ExcludeFocus(
      child: IgnorePointer(
        key: ValueKey<String>('setting-locked-$settingId'),
        child: Opacity(opacity: 0.5, child: child),
      ),
    ),
  );
}

/// A row with a control of its own (the model servers, HUM-405) under the
/// lock of a company policy: the note above, the row read only below.
/// Without a lock it is [child] as it stands (HUM-534).
class SettingLockGuard extends ConsumerWidget {
  /// Creates the guard of [settingId] around [child].
  const SettingLockGuard({
    required this.settingId,
    required this.child,
    this.alsoLockedBy = const <String>[],
    super.key,
  });

  /// The id of the entry in the catalog.
  final String settingId;

  /// Entries whose lock locks this row as well: the daemon refuses every
  /// change of the servers while one of their keys is locked.
  final List<String> alsoLockedBy;

  /// The row.
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final SettingsView view = SettingsView.of(context, ref);
    SettingsLock? lock;
    for (final String id in <String>[settingId, ...alsoLockedBy]) {
      lock ??= view.entry(id)?.lock;
    }
    if (lock == null) {
      return child;
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SettingLockNote(lock: lock, settingId: settingId),
        SizedBox(height: HTheme.of(context).spacing.x2),
        ReadOnlySetting(settingId: settingId, child: child),
      ],
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
