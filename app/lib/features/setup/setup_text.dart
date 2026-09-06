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
/// not run wears the resting grey and not the amber of a warning: amber says
/// "something is off", and nobody knows whether anything is off.
Color setupStateColor(HTokens tokens, SetupCheckState state) => switch (state) {
  SetupCheckState.checking => tokens.state.held,
  SetupCheckState.ok => tokens.state.allowed,
  SetupCheckState.warn => tokens.state.held,
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
      SetupCheckState.checking => tokens.stateText.held,
      SetupCheckState.ok => tokens.stateText.allowed,
      SetupCheckState.warn => tokens.stateText.held,
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
/// Localised for the four codes the client raises itself, the daemon's own
/// words for everything else. It is the same split the whole product uses: the
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
  _ => (
    diagnostic.title.isEmpty ? diagnostic.code : diagnostic.title,
    diagnostic.why,
  ),
};
