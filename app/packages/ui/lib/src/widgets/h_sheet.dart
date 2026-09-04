import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_hairline.dart';
import 'h_icon_button.dart';

/// A panel that slides in from the right: rule from request, history detail,
/// isolation details.
///
/// A sheet never asks a question that would block the queue — that is what the
/// inspector pane is for. It is a widget, not a route, so the shell decides how
/// it is mounted.
///
/// Das Blatt hält den Fokus bei sich und schließt auf `Escape`, sobald
/// [onClose] gesetzt ist. Ohne beides ist es für die Tastatur eine Sackgasse:
/// `Tab` liefe durch den Bildschirm dahinter, den das Blatt gerade verdeckt,
/// und der einzige Weg heraus wäre das Schließkreuz mit der Maus
/// (`docs/UX.md` 5.1). Denselben Weg geht `HModal`.
class HSheet extends StatelessWidget {
  /// Creates a sheet.
  const HSheet({
    required this.title,
    required this.child,
    this.actions = const <Widget>[],
    required this.closeSemanticsLabel,
    this.onClose,
    this.width = 360,
    super.key,
  });

  /// The heading, 16/600.
  final Widget title;

  /// The body.
  final Widget child;

  /// Actions in the header, left of the close affordance.
  final List<Widget> actions;

  /// Invoked when the close affordance is used. Null hides it.
  final VoidCallback? onClose;

  /// Screen-reader label of the close affordance. Required, and passed by the
  /// caller already localised: this package hard-wires no user-visible string.
  final String closeSemanticsLabel;

  /// Width of the sheet.
  final double width;

  @override
  Widget build(BuildContext context) {
    final VoidCallback? close = onClose;
    return Shortcuts(
      shortcuts: <ShortcutActivator, Intent>{
        if (close != null)
          const SingleActivator(LogicalKeyboardKey.escape):
              const DismissIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          if (close != null)
            DismissIntent: CallbackAction<DismissIntent>(
              onInvoke: (DismissIntent intent) {
                close();
                return null;
              },
            ),
        },
        child: FocusScope(autofocus: true, child: _panel(context)),
      ),
    );
  }

  Widget _panel(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final VoidCallback? onClose = this.onClose;
    return SizedBox(
      width: width,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          border: Border(left: BorderSide(color: tokens.colors.line)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Padding(
              padding: const EdgeInsets.fromLTRB(
                HSpace.panelPadding,
                HSpace.x2,
                HSpace.x2,
                HSpace.x2,
              ),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: DefaultTextStyle(
                      style: tokens.typography.ui16.semibold.tinted(
                        tokens.colors.fg0,
                      ),
                      child: title,
                    ),
                  ),
                  ...actions,
                  if (onClose != null)
                    HIconButton(
                      glyph: HGlyph.close,
                      onPressed: onClose,
                      semanticsLabel: closeSemanticsLabel,
                    ),
                ],
              ),
            ),
            const HHairline(),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.all(HSpace.panelPadding),
                child: child,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
