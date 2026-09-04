/// The exact command that starts the sandbox (HUM-040).
///
/// This is the proof. Every row of the mounts table and every line of the
/// environment was read out of this command inside the daemon, so a person
/// who does not believe the tables can read the command and check them. It is
/// therefore shown whole -- no ellipsis, no wrapping in the middle of a path,
/// selectable, and copyable into a shell.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'arrive.dart';

/// The sheet that shows the command line.
class ArgvSheet extends StatefulWidget {
  /// Shows [argv]. [onClose] closes the sheet.
  const ArgvSheet({required this.argv, required this.onClose, super.key});

  /// The whole command line, as the daemon built it.
  final String argv;

  /// Closes the sheet.
  final VoidCallback onClose;

  /// How wide the sheet is. Wider than the default, because a bubblewrap
  /// command line is long and the point of the sheet is to read it.
  static const double sheetWidth = 640;

  @override
  State<ArgvSheet> createState() => _ArgvSheetState();
}

class _ArgvSheetState extends State<ArgvSheet> {
  bool _copied = false;

  Future<void> _copy() async {
    await Clipboard.setData(ClipboardData(text: widget.argv));
    if (!mounted) {
      return;
    }
    setState(() => _copied = true);
    await Future<void>.delayed(HMotion.copyFeedback);
    if (mounted) {
      setState(() => _copied = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SandboxArrive(
      from: ArriveFrom.right,
      child: HSheet(
        width: ArgvSheet.sheetWidth,
        title: Text(l10n.sandboxArgvTitle),
        closeSemanticsLabel: l10n.sandboxArgvClose,
        onClose: widget.onClose,
        actions: <Widget>[
          HButton(
            key: const Key('sandbox-argv-copy'),
            variant: HButtonVariant.ghost,
            size: HButtonSize.sm,
            onPressed: widget.argv.isEmpty ? null : _copy,
            child: Text(
              _copied ? l10n.sandboxArgvCopied : l10n.sandboxArgvCopy,
            ),
          ),
        ],
        child: widget.argv.isEmpty
            ? Padding(
                padding: EdgeInsets.all(tokens.spacing.x3),
                child: Text(
                  l10n.sandboxArgvNone,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                ),
              )
            : Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Padding(
                    padding: EdgeInsets.fromLTRB(
                      tokens.spacing.x3,
                      tokens.spacing.x3,
                      tokens.spacing.x3,
                      tokens.spacing.x2,
                    ),
                    child: Text(
                      l10n.sandboxArgvHint,
                      style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                    ),
                  ),
                  const HHairline(),
                  Expanded(
                    child: SingleChildScrollView(
                      padding: EdgeInsets.all(tokens.spacing.x3),
                      child: SingleChildScrollView(
                        scrollDirection: Axis.horizontal,
                        // The app is built on `WidgetsApp`, so `SelectableText`
                        // is out of reach; `SelectableRegion` is the
                        // widgets-layer equivalent. The pointer drags, Ctrl+C
                        // copies, and no handle is dragged in a desktop window.
                        child: SelectableRegion(
                          selectionControls: emptyTextSelectionControls,
                          child: Text(
                            widget.argv,
                            key: const Key('sandbox-argv-text'),
                            // No wrapping: a path broken across two lines
                            // cannot be compared with the one on disk.
                            maxLines: 1,
                            style: tokens.typography.mono12.tinted(
                              tokens.colors.fg0,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
      ),
    );
  }
}
