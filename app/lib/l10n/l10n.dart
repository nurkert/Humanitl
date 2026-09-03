/// Access to the generated localizations (`flutter gen-l10n`, see
/// `l10n.yaml`). Every user-visible string of the app comes from
/// `l10n/app_en.arb` (source) and `l10n/app_de.arb`; none is written in code.
library;

import 'package:flutter/widgets.dart';

import '../core/ui/ui.dart';
import 'generated/app_localizations.dart';

export 'generated/app_localizations.dart';

/// `context.l10n` instead of `AppLocalizations.of(context)`.
extension L10nContext on BuildContext {
  /// The localizations of the closest `Localizations` ancestor.
  AppLocalizations get l10n => AppLocalizations.of(this);
}

/// Labels the design system asks for by key (`HFlowState.l10nKey`).
extension FlowStateLabels on AppLocalizations {
  /// The label of [state] in the current language.
  String flowStateLabel(HFlowState state) => switch (state) {
    HFlowState.held => stateHeld,
    HFlowState.allowed => stateAllowed,
    HFlowState.allowedEdited => stateAllowedEdited,
    HFlowState.blocked => stateBlocked,
    HFlowState.timedOut => stateTimedOut,
    HFlowState.autoRule => stateAutoRule,
    HFlowState.passthroughLlm => statePassthroughLlm,
    HFlowState.error => stateError,
  };
}
