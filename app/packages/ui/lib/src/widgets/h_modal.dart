import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// A centred card over a scrim.
///
/// Modals are reserved for destructive confirmations — blocking more than five
/// flows at once, deleting a forever rule, stopping a running sandbox. A modal
/// is never used to make a normal decision; that happens in the queue.
///
/// Der Modal fängt den Fokus ein und gibt ihn beim Schließen zurück, und
/// `Escape` verwirft ihn, sofern [onDismiss] gesetzt ist. Ohne beides ist ein
/// Modal für die Tastatur eine Sackgasse: `Tab` liefe durch den Screen
/// dahinter, über den man gerade nicht entscheiden soll
/// (`docs/UX.md` 5.1 und 9, Punkt 16).
///
/// Die Karte ist `ModalContainer` aus `shadcn_flutter`, dieselbe Fläche, auf
/// der auch deren eigener Dialog steht. Ihr `AlertDialog` wäre der nächste
/// Nachbar, bringt aber seinen eigenen Verdunkler mit, der keine Berührung
/// annimmt, und schreibt Ecke und Rahmenfarbe fest — der Rahmen stünde dann
/// als `muted` auf `popover` und wäre nicht zu sehen. Der Verdunkler bleibt
/// deshalb unserer: er ist die Fläche, die den Modal schließt.
class HModal extends StatelessWidget {
  /// Creates a modal.
  const HModal({
    required this.title,
    required this.child,
    this.actions = const <Widget>[],
    this.onDismiss,
    this.width = 420,
    this.scrimSemanticsLabel,
    super.key,
  });

  /// The heading, 16/600.
  final Widget title;

  /// The body of the card.
  final Widget child;

  /// Buttons, right aligned below the body.
  final List<Widget> actions;

  /// Invoked when the scrim is tapped. Null makes the modal non-dismissible.
  final VoidCallback? onDismiss;

  /// Width of the card.
  final double width;

  /// Screen-reader label of the scrim.
  final String? scrimSemanticsLabel;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final VoidCallback? dismiss = onDismiss;
    return Shortcuts(
      shortcuts: <ShortcutActivator, Intent>{
        if (dismiss != null)
          const SingleActivator(LogicalKeyboardKey.escape):
              const DismissIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          if (dismiss != null)
            DismissIntent: CallbackAction<DismissIntent>(
              onInvoke: (DismissIntent intent) {
                dismiss();
                return null;
              },
            ),
        },
        child: HTheme.host(
          context,
          FocusScope(autofocus: true, child: _card(context, tokens)),
        ),
      ),
    );
  }

  Widget _card(BuildContext context, HTokens tokens) {
    return Stack(
      fit: StackFit.expand,
      children: <Widget>[
        Semantics(
          button: onDismiss != null,
          label: scrimSemanticsLabel,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: onDismiss,
            child: ColoredBox(
              color: HColorDerivation.fade(tokens.colors.bg0, 0.72),
            ),
          ),
        ),
        Center(
          child: SizedBox(
            width: width,
            child: shad.ModalContainer(
              filled: true,
              fillColor: tokens.colors.bg2,
              borderColor: tokens.colors.lineStrong,
              borderWidth: HSize.hairline,
              borderRadius: HRadius.cardRadius,
              padding: const EdgeInsets.all(HSpace.x4),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  DefaultTextStyle(
                    style: tokens.typography.ui16.semibold.tinted(
                      tokens.colors.fg0,
                    ),
                    child: title,
                  ),
                  SizedBox(height: tokens.spacing.x2),
                  DefaultTextStyle(
                    style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                    child: child,
                  ),
                  if (actions.isNotEmpty) ...<Widget>[
                    SizedBox(height: tokens.spacing.x4),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: <Widget>[
                        for (final Widget action in actions) ...<Widget>[
                          SizedBox(width: tokens.spacing.x2),
                          action,
                        ],
                      ],
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}
