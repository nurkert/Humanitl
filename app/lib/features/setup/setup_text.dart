/// How the setup screen names what the daemon reports (HUM-044).
///
/// Five widgets show the same vocabulary -- the four rows and the button under
/// them -- and all five must name a state and a severity the same way. The
/// functions live here and not in one of the widgets, so none of them has to
/// import another (ARCHITECTURE 5); it is the same arrangement the sandbox
/// screen uses (`features/sandbox/sandbox_text.dart`).
library;

import 'package:flutter/widgets.dart' show Color;

import '../../core/domain/domain.dart';
import '../../core/ui/diagnostic_severity.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/setup_provider.dart';

/// The heading of one row, in the person's language.
String setupCheckTitle(AppLocalizations l10n, SetupCheckKind kind) =>
    switch (kind) {
      SetupCheckKind.daemon => l10n.setupCheckDaemon,
      SetupCheckKind.llm => l10n.setupCheckLlm,
      SetupCheckKind.project => l10n.setupCheckProject,
      SetupCheckKind.sandbox => l10n.setupCheckSandbox,
    };

/// The word next to the mark of one row.
///
/// [SetupCheckState.unmeasured] has a word of its own and never borrows the
/// one of a passed check: the whole point of the state is that nobody looked.
String setupStateLabel(AppLocalizations l10n, SetupCheckState state) =>
    switch (state) {
      SetupCheckState.checking => l10n.setupStateChecking,
      SetupCheckState.ok => l10n.setupStateOk,
      SetupCheckState.warn => l10n.setupStateWarn,
      SetupCheckState.unmeasured => l10n.setupStateUnmeasured,
      SetupCheckState.failed => l10n.setupStateFailed,
    };

/// The hue of one state, as a surface: dots and marks.
///
/// Colour means state and nothing else (`docs/UX.md` 3.3). A check that could
/// not run wears the resting grey and not the yellow of a warning: yellow says
/// "something is off", and nobody knows whether anything is off. A check that
/// is still running wears the warning too: it is not settled yet. Neither is
/// `held`, which since HUM-260 means a held request and nothing else.
Color setupStateColor(HTokens tokens, SetupCheckState state) => switch (state) {
  SetupCheckState.checking => tokens.colors.warning,
  SetupCheckState.ok => tokens.state.allowed,
  SetupCheckState.warn => tokens.colors.warning,
  SetupCheckState.unmeasured => tokens.colors.fg2,
  SetupCheckState.failed => tokens.state.error,
};

/// The hue the word of one state may wear.
///
/// [setupStateColor] is the surface palette, clamped to 3:1 and right for a
/// mark. A word needs 4,5:1 and takes the text palette instead
/// (`docs/UX.md` 6).
Color setupStateTextColor(HTokens tokens, SetupCheckState state) =>
    switch (state) {
      SetupCheckState.checking => tokens.colors.warningText,
      SetupCheckState.ok => tokens.stateText.allowed,
      SetupCheckState.warn => tokens.colors.warningText,
      SetupCheckState.unmeasured => tokens.colors.fg1,
      SetupCheckState.failed => tokens.stateText.error,
    };

/// Whether the mark of one state is a disc.
///
/// A measured state is a disc, an unmeasured one a ring. The shape says what
/// the colour says, so a check nobody could run is never readable as a paler
/// version of one that passed -- the same rule the isolation panel keeps along
/// its own axis (`features/sandbox/sandbox_text.dart`).
bool setupStateFilled(SetupCheckState state) =>
    state == SetupCheckState.ok ||
    state == SetupCheckState.warn ||
    state == SetupCheckState.failed;

/// What stands under a row that has no evidence to show.
String setupNoEvidence(AppLocalizations l10n, SetupCheckState state) =>
    switch (state) {
      SetupCheckState.checking => l10n.setupEvidenceChecking,
      SetupCheckState.unmeasured => l10n.setupEvidenceUnmeasured,
      _ => l10n.setupEvidenceNone,
    };

/// Das Wort für [severity], für die Zeilen und Karten dieses Bildschirms.
///
/// Ein Alias auf die eine Abbildung in `core/ui/diagnostic_severity.dart`
/// (HUM-068): Kopierte Tabellen laufen auseinander, sobald jemand einen Grad
/// ergänzt, und dieser Bildschirm zeigt dieselben Befunde wie die Sandbox und
/// die Warteschlange.
String setupSeverityLabel(AppLocalizations l10n, Severity severity) =>
    severityLabel(l10n, severity);

/// Der Farbton für [severity]; derselbe Alias wie [setupSeverityLabel].
Color setupSeverityColor(HTokens tokens, Severity severity) =>
    severityColor(tokens, severity);

/// The word of one doctor line, in the person's language.
///
/// The eleven identifiers are part of the contract
/// (`humanitl_sandbox::doctor::CheckId`); this is their heading. A line this
/// build does not know keeps its identifier as its heading rather than
/// disappearing: a daemon that grew a twelfth check must be able to show it.
String doctorCheckTitle(AppLocalizations l10n, String id) => switch (id) {
  DoctorCheckId.bwrap => l10n.doctorCheckBwrap,
  DoctorCheckId.userns => l10n.doctorCheckUserns,
  DoctorCheckId.seccomp => l10n.doctorCheckSeccomp,
  DoctorCheckId.runtimeDir => l10n.doctorCheckRuntimeDir,
  DoctorCheckId.systemdUser => l10n.doctorCheckSystemdUser,
  DoctorCheckId.daemon => l10n.doctorCheckDaemon,
  DoctorCheckId.agent => l10n.doctorCheckAgent,
  DoctorCheckId.llm => l10n.doctorCheckLlm,
  DoctorCheckId.tray => l10n.doctorCheckTray,
  DoctorCheckId.renderer => l10n.doctorCheckRenderer,
  DoctorCheckId.diskSpace => l10n.doctorCheckDiskSpace,
  _ => id,
};

/// The state one doctor line stands in, in the vocabulary of the four rows.
///
/// The mapping is the same one [setupChecks] uses, so a line inside the
/// machine list wears exactly the mark its row would wear.
SetupCheckState doctorLineState(DoctorCheck line) => switch (line.status) {
  DoctorStatus.fail => SetupCheckState.failed,
  DoctorStatus.unknown => SetupCheckState.unmeasured,
  DoctorStatus.warn when !line.isMeasured => SetupCheckState.unmeasured,
  DoctorStatus.warn => SetupCheckState.warn,
  DoctorStatus.ok => SetupCheckState.ok,
};

/// Title and cause of a diagnostic, in the person's language.
///
/// Localised for the codes the client raises itself. Every other code
/// takes its title from ARB (`DiagnosticL10n`, HUM-052) and keeps the daemon's
/// own sentence as the cause. It is the same split the whole product uses: the
/// generic sentence is the title, the measured one is the cause
/// (`docs/UX.md` 4.4).
(String, String) setupDiagnosticText(
  AppLocalizations l10n,
  Diagnostic diagnostic,
) => switch (diagnostic.code) {
  DiagnosticCodes.daemonUnreachable => (
    l10n.setupDaemonMissingTitle,
    l10n.setupDaemonMissingWhy,
  ),
  DiagnosticCodes.protoIncompatible => (
    l10n.setupVersionMismatchTitle,
    l10n.setupVersionMismatchWhy,
  ),
  DiagnosticCodes.tokenInvalid => (
    l10n.setupTokenInvalidTitle,
    l10n.setupTokenInvalidWhy,
  ),
  // Ein im Client gebauter Befund trägt keinen Titel: den hält das Register
  // im Daemon, und über die Leitung kommt er nicht. Ohne diesen Fall fiele
  // die Karte auf `diagnostic.code` zurück und trüge „CONFIG_013" als
  // Überschrift -- internes Vokabular auf dem Schirm (CONVENTIONS 4.13), und
  // dazu ein zweites Mal, weil das Abzeichen den Code schon zeigt.
  DiagnosticCodes.noProjectFolder => (
    l10n.setupProjectNoFolderTitle,
    l10n.setupProjectNoFolderWhy,
  ),
  // Auch der gescheiterte Ordnerdialog entsteht im Client (HUM-282). Sein
  // `why` trägt den Fehlertext der Plattform auf Englisch; der übersetzte
  // Satz mit dem Ausweg über die Kommandozeile steht deshalb als Grund, der
  // rohe als Detail darunter.
  DiagnosticCodes.folderDialogFailed => (
    l10n.diagUI003Title,
    l10n.diagUI003Why,
  ),
  _ => _titleAndCause(DiagnosticL10n.resolve(diagnostic, l10n)),
};

(String, String) _titleAndCause(DiagnosticText text) =>
    (text.title, text.cause);

/// The code of the finding a model server raises when it wants a login
/// (`daemon/crates/core-types/src/diagnostics/codes.rs`, `LLM_002`).
const String _llmLogin = 'LLM_002';

/// The code of the finding for a model server nobody answers at (`LLM_001`).
const String _llmUnreachable = 'LLM_001';

/// The headline of an open step, in plain words, or null for a green one
/// (HUM-287).
///
/// It names the situation before the finding under it says why: the finding
/// keeps its code, its cause and its fix (`docs/UX.md` 4.4), the headline only
/// says what the person is looking at. The words come from the state and the
/// code the daemon sent; nothing about the machine is judged here.
String? setupStepHeadline(AppLocalizations l10n, SetupCheck check) {
  final String? code = check.diagnostic?.code;
  return switch ((check.kind, check.state)) {
    (_, SetupCheckState.ok) => null,
    (_, SetupCheckState.checking) => l10n.setupHeadChecking,
    (SetupCheckKind.daemon, SetupCheckState.failed) => l10n.setupHeadDaemonDown,
    (SetupCheckKind.llm, _) when code == _llmLogin => l10n.setupHeadLlmKey,
    (SetupCheckKind.llm, _) when code == _llmUnreachable =>
      l10n.setupHeadLlmUnreachable,
    (SetupCheckKind.llm, SetupCheckState.failed) => l10n.setupHeadLlmFailed,
    (SetupCheckKind.llm, SetupCheckState.unmeasured) =>
      l10n.setupHeadLlmContact,
    (SetupCheckKind.project, SetupCheckState.failed) =>
      code == DiagnosticCodes.noProjectFolder
          ? l10n.setupHeadProjectNone
          : l10n.setupHeadProjectRefused,
    (SetupCheckKind.sandbox, SetupCheckState.failed) =>
      l10n.setupHeadSandboxFailed,
    (SetupCheckKind.sandbox, SetupCheckState.unmeasured) =>
      l10n.setupHeadSandboxUnmeasured,
    (_, SetupCheckState.warn) => l10n.setupHeadWarn,
    (_, SetupCheckState.unmeasured) => l10n.setupHeadUnmeasured,
  };
}
