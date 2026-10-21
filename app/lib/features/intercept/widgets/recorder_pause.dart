/// Die Pause vor der ersten Freigabe während einer Störung der Aufzeichnung
/// (HUM-250).
///
/// Sie steht an der Stelle der Entscheidungsleiste, wie die Pause mit offenen
/// Funden: kein Modal, keine Ebene (`docs/UX.md` 5.4). Oben steht, was los
/// ist, darunter, was ein Senden jetzt heißt, am Fuß die zwei Wege:
/// unaufgezeichnet senden oder zurück. Das Widget entscheidet nichts; beide
/// Knöpfe rufen, was der Aufrufer hineinreicht (`RecorderPause` in
/// `providers/recorder_health.dart`).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// Die Pause der Aufzeichnung.
class RecorderPauseView extends StatelessWidget {
  /// Baut die Pause; [onSend] ist null, solange der Knopf noch nicht annimmt.
  const RecorderPauseView({
    required this.onSend,
    required this.onBack,
    super.key,
  });

  /// „Unaufgezeichnet senden", oder null, solange die Pause noch nicht
  /// [HMotion.rearm] stand.
  final VoidCallback? onSend;

  /// Schließt die Pause, ohne zu senden.
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Die Farbe des Fehlers und nicht Rot: Rot heißt in diesem Programm
    // blockiert und nichts anderes (`docs/UX.md` 3.3). Dieselbe Farbe trägt
    // die Statusleiste, solange die Störung dauert.
    final Color color = tokens.stateColor(HFlowState.error);
    return Semantics(
      container: true,
      label: l10n.interceptRecorderPauseTitle,
      child: DecoratedBox(
        key: const Key('intercept-recorder-pause'),
        decoration: BoxDecoration(
          color: tokens.colors.bg2,
          border: Border.all(color: color),
          borderRadius: BorderRadius.circular(tokens.radii.card),
        ),
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x3),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                l10n.interceptRecorderPauseTitle,
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
              ),
              SizedBox(height: tokens.spacing.x2),
              Text(
                l10n.interceptRecorderPauseBody,
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
              SizedBox(height: tokens.spacing.x3),
              Wrap(
                spacing: tokens.spacing.x3,
                runSpacing: tokens.spacing.x2,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  _PauseButton(
                    key: const Key('intercept-recorder-pause-send'),
                    variant: HButtonVariant.secondary,
                    label: l10n.interceptRecorderPauseSend,
                    shortcut: l10n.interceptFindingsPauseKeySend,
                    onPressed: onSend,
                  ),
                  _PauseButton(
                    key: const Key('intercept-recorder-pause-back'),
                    variant: HButtonVariant.ghost,
                    label: l10n.interceptFindingsPauseBack,
                    shortcut: l10n.interceptFindingsPauseKeyBack,
                    onPressed: onBack,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Ein Knopf am Fuß der Pause, mit seiner Taste daneben.
class _PauseButton extends StatelessWidget {
  const _PauseButton({
    required this.variant,
    required this.label,
    required this.shortcut,
    required this.onPressed,
    super.key,
  });

  final HButtonVariant variant;
  final String label;
  final String shortcut;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return HButton(
      variant: variant,
      onPressed: onPressed,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(label),
          SizedBox(width: tokens.spacing.x2),
          Text(
            shortcut,
            style: tokens.typography.mono11.tinted(tokens.colors.fg1),
          ),
        ],
      ),
    );
  }
}
