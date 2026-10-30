/// Eine Zeile des Änderungs-Panels: eine Änderung der Wegwerf-Sandbox
/// (HUM-351).
///
/// Links das Kästchen mit dem Pfad als Beschriftung, daneben die Art, die
/// Größe und die Marken des Daemons; darunter, immer sichtbar, der Satz des
/// Daemons zu einem Konflikt oder einer Ablehnung. Aufgeklappt zeigt die
/// Zeile den Text-Diff oder, für Binäres, Größe und SHA-256. Jede Marke steht
/// als Wort und nie als Farbe allein (`docs/UX.md` 3.3).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/text/format.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'diff_view.dart';

/// Das Wort für [kind].
String changeKindLabel(AppLocalizations l10n, DisposableChangeKind kind) =>
    switch (kind) {
      DisposableChangeKind.added => l10n.sandboxChangesKindAdded,
      DisposableChangeKind.modified => l10n.sandboxChangesKindModified,
      DisposableChangeKind.removed => l10n.sandboxChangesKindRemoved,
      DisposableChangeKind.symlink => l10n.sandboxChangesKindSymlink,
      DisposableChangeKind.mode => l10n.sandboxChangesKindMode,
      DisposableChangeKind.unknown => l10n.sandboxChangesKindUnknown,
    };

/// Eine Änderung mit Kästchen, Marken und aufklappbarem Diff.
class ChangeRow extends StatefulWidget {
  /// Zeigt [change]. [selected] ist der Stand des Kästchens, [onChanged]
  /// wird mit dem gewünschten Stand gerufen; [enabled] ist falsch für eine
  /// Änderung, die der Daemon nie schreibt.
  const ChangeRow({
    required this.change,
    required this.selected,
    required this.onChanged,
    required this.enabled,
    this.overwriteUnavailable = false,
    super.key,
  });

  /// Wahr, wenn die Datei einen Konflikt hat und der Daemon zu alt ist, um
  /// sie einzeln zu überschreiben (vor Minor 26, HUM-351).
  final bool overwriteUnavailable;

  /// Die Änderung.
  final DisposableChange change;

  /// Ob sie zum Übernehmen gewählt ist.
  final bool selected;

  /// Wird mit dem gewünschten Stand gerufen.
  final ValueChanged<bool> onChanged;

  /// Falsch, wenn das Kästchen nicht angefasst werden darf.
  final bool enabled;

  @override
  State<ChangeRow> createState() => _ChangeRowState();
}

class _ChangeRowState extends State<ChangeRow> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DisposableChange change = widget.change;
    final String id = change.id;
    final bool hasDetail =
        change.hasDiff ||
        change.binary ||
        change.symlinkTarget.isNotEmpty ||
        change.sha256.isNotEmpty ||
        change.kind == DisposableChangeKind.removed;
    return Padding(
      key: ValueKey<String>('change-row-$id'),
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: Align(
                  alignment: Alignment.centerLeft,
                  // Der Pfad-Hash als Tooltip: Er ist die Kennung, mit der
                  // `humanitl sandbox apply ID` dieselbe Änderung meint.
                  child: HoverLabel(
                    label: l10n.sandboxChangesIdTooltip(id),
                    child: HCheckbox(
                      key: ValueKey<String>('change-select-$id'),
                      label: change.path,
                      value: widget.selected,
                      enabled: widget.enabled,
                      onChanged: widget.onChanged,
                    ),
                  ),
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              Flexible(
                child: Wrap(
                  alignment: WrapAlignment.end,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  spacing: tokens.spacing.x1,
                  runSpacing: tokens.spacing.x1,
                  children: <Widget>[
                    HBadge(
                      key: ValueKey<String>('change-kind-$id'),
                      text: changeKindLabel(l10n, change.kind),
                      color: tokens.colors.fg2,
                      textColor: tokens.colors.fg1,
                    ),
                    if (change.kind != DisposableChangeKind.removed &&
                        change.entry == 'file')
                      Text(
                        formatBytes(change.size),
                        style: tokens.typography.mono11.tinted(
                          tokens.colors.fg2,
                        ),
                      ),
                    if (change.hasConflict)
                      HBadge(
                        key: ValueKey<String>('change-conflict-$id'),
                        text: l10n.sandboxChangesBadgeConflict,
                        color: tokens.colors.warning,
                        textColor: tokens.colors.warningText,
                      ),
                    if (change.isRefused)
                      HBadge(
                        key: ValueKey<String>('change-refused-$id'),
                        text: l10n.sandboxChangesBadgeRefused,
                        color: tokens.state.error,
                        textColor: tokens.stateText.error,
                      ),
                    if (change.strippedBits != 0)
                      HBadge(
                        key: ValueKey<String>('change-stripped-$id'),
                        text: l10n.sandboxChangesBadgeStripped,
                        color: tokens.colors.info,
                        textColor: tokens.colors.infoText,
                      ),
                    if (change.hardlinked)
                      HBadge(
                        key: ValueKey<String>('change-hardlink-$id'),
                        text: l10n.sandboxChangesBadgeHardlink,
                        color: tokens.colors.info,
                        textColor: tokens.colors.infoText,
                      ),
                  ],
                ),
              ),
              SizedBox(width: tokens.spacing.x1),
              if (hasDetail)
                // Die Drehung sagt auf und zu; das Wort steht im Label.
                AnimatedRotation(
                  turns: _open ? 0.25 : 0,
                  duration: tokens.motion.press,
                  child: HIconButton(
                    key: ValueKey<String>('change-expand-$id'),
                    glyph: HGlyph.chevronRight,
                    semanticsLabel: _open
                        ? l10n.sandboxChangesHideDetail(change.path)
                        : l10n.sandboxChangesShowDetail(change.path),
                    onPressed: () => setState(() => _open = !_open),
                  ),
                )
              else
                const SizedBox(width: HSize.hitMin),
            ],
          ),
          if (change.hasConflict)
            _Reason(
              key: ValueKey<String>('change-conflict-why-$id'),
              text: l10n.sandboxChangesConflictWhy(change.conflict),
              color: tokens.colors.warningText,
            ),
          if (widget.overwriteUnavailable)
            _Reason(
              key: ValueKey<String>('change-overwrite-unavailable-$id'),
              text: l10n.sandboxChangesOverwriteNeedsUpdate,
              color: tokens.colors.warningText,
            ),
          if (change.isRefused)
            _Reason(
              key: ValueKey<String>('change-refused-why-$id'),
              text: l10n.sandboxChangesRefusedWhy(change.refused),
              color: tokens.stateText.error,
            ),
          if (change.strippedBits != 0)
            _Reason(
              text: l10n.sandboxChangesStrippedWhy,
              color: tokens.colors.fg1,
            ),
          if (change.hardlinked)
            _Reason(
              text: l10n.sandboxChangesHardlinkWhy,
              color: tokens.colors.fg1,
            ),
          if (change.mangled)
            _Reason(text: l10n.sandboxChangesMangled, color: tokens.colors.fg1),
          if (_open)
            Padding(
              key: ValueKey<String>('change-detail-$id'),
              padding: EdgeInsets.only(
                top: tokens.spacing.x2,
                left: tokens.spacing.x5,
              ),
              child: _Detail(change: change),
            ),
        ],
      ),
    );
  }
}

/// Ein Satz des Daemons unter der Zeile.
class _Reason extends StatelessWidget {
  const _Reason({required this.text, required this.color, super.key});

  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.only(left: tokens.spacing.x5, top: tokens.spacing.x1),
      child: Text(text, style: tokens.typography.ui12.tinted(color)),
    );
  }
}

/// Was die aufgeklappte Zeile zeigt.
class _Detail extends StatelessWidget {
  const _Detail({required this.change});

  final DisposableChange change;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TextStyle small = tokens.typography.ui12.tinted(tokens.colors.fg1);
    final TextStyle mono = tokens.typography.mono12.tinted(tokens.colors.fg0);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (change.symlinkTarget.isNotEmpty)
          Text(
            l10n.sandboxChangesSymlinkTarget(change.symlinkTarget),
            key: ValueKey<String>('change-target-${change.id}'),
            style: mono,
          ),
        if (change.kind == DisposableChangeKind.removed)
          Text(l10n.sandboxChangesRemoved, style: small),
        if (change.hasDiff)
          DiffView(
            key: ValueKey<String>('change-diff-${change.id}'),
            diff: change.diff,
          )
        else if (change.sha256.isNotEmpty)
          Text(
            change.binary
                ? l10n.sandboxChangesBinary(
                    formatBytes(change.size),
                    change.sha256,
                  )
                : l10n.sandboxChangesNoText(
                    formatBytes(change.size),
                    change.sha256,
                  ),
            key: ValueKey<String>('change-binary-${change.id}'),
            style: mono,
          ),
      ],
    );
  }
}
