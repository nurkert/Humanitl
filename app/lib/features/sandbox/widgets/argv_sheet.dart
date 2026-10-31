/// The exact command that starts the sandbox (HUM-040).
///
/// This is the proof. Every row of the mounts table and every line of the
/// environment was read out of this command inside the daemon, so a person
/// who does not believe the tables can read the command and check them. It is
/// therefore shown whole -- no ellipsis, no wrapping in the middle of a path,
/// selectable, and copyable into a shell.
///
/// Under Seatbelt the command does not say what the sandbox allows; the
/// profile does, so the sheet shows it under the command, just as whole
/// (HUM-376).
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'arrive.dart';

/// The sheet that shows the command line.
class ArgvSheet extends StatefulWidget {
  /// Shows [argv]. [onClose] closes the sheet.
  const ArgvSheet({
    required this.argv,
    required this.onClose,
    this.policy = '',
    super.key,
  });

  /// The whole command line, as the daemon built it.
  final String argv;

  /// The text of the backend's policy; empty where the command says it all.
  final String policy;

  /// Closes the sheet.
  final VoidCallback onClose;

  /// How wide the sheet is. Wider than the default, because a sandbox
  /// command line is long and the point of the sheet is to read it.
  static const double sheetWidth = 640;

  @override
  State<ArgvSheet> createState() => _ArgvSheetState();
}

class _ArgvSheetState extends State<ArgvSheet> {
  /// What was copied last, while the button says so.
  String? _copied;

  Future<void> _copy(String text) async {
    await Clipboard.setData(ClipboardData(text: text));
    if (!mounted) {
      return;
    }
    setState(() => _copied = text);
    await Future<void>.delayed(HMotion.copyFeedback);
    if (mounted) {
      setState(() => _copied = null);
    }
  }

  /// Selectable, unwrapped text: a path broken across two lines cannot be
  /// compared with the one on disk.
  Widget _whole(String text, Key key, HTokens tokens, {int? maxLines}) =>
      SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        // The app is built on `WidgetsApp`, so `SelectableText` is out of
        // reach; `SelectableRegion` is the widgets-layer equivalent. The
        // pointer drags, Ctrl+C copies, and no handle is dragged in a desktop
        // window.
        child: SelectableRegion(
          selectionControls: emptyTextSelectionControls,
          child: Text(
            text,
            key: key,
            maxLines: maxLines,
            style: tokens.typography.mono12.tinted(tokens.colors.fg0),
          ),
        ),
      );

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
            onPressed: widget.argv.isEmpty ? null : () => _copy(widget.argv),
            child: Text(
              _copied == widget.argv
                  ? l10n.sandboxArgvCopied
                  : l10n.sandboxArgvCopy,
            ),
          ),
          if (widget.policy.isNotEmpty)
            HButton(
              key: const Key('sandbox-policy-copy'),
              variant: HButtonVariant.ghost,
              size: HButtonSize.sm,
              onPressed: () => _copy(widget.policy),
              child: Text(
                _copied == widget.policy
                    ? l10n.sandboxArgvCopied
                    : l10n.sandboxPolicyCopy,
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
                      widget.policy.isEmpty
                          ? l10n.sandboxArgvHint
                          : l10n.sandboxPolicyArgvHint,
                      style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                    ),
                  ),
                  const HHairline(),
                  Expanded(
                    child: SingleChildScrollView(
                      padding: EdgeInsets.all(tokens.spacing.x3),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          _whole(
                            widget.argv,
                            const Key('sandbox-argv-text'),
                            tokens,
                            maxLines: 1,
                          ),
                          if (widget.policy.isNotEmpty) ...<Widget>[
                            SizedBox(height: tokens.spacing.x3),
                            Text(
                              l10n.sandboxPolicyTitle,
                              style: tokens.typography.ui12.tinted(
                                tokens.colors.fg1,
                              ),
                            ),
                            SizedBox(height: tokens.spacing.x2),
                            _whole(
                              widget.policy,
                              const Key('sandbox-policy-text'),
                              tokens,
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),
                ],
              ),
      ),
    );
  }
}
