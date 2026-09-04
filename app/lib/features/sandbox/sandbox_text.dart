/// How the sandbox screen names what the daemon reports.
///
/// Four widgets show the same vocabulary -- header, mounts, environment and
/// the command sheet -- and all four must name a mode, an origin and a state
/// the same way. The functions live here and not in one of the widgets, so
/// none of them has to import another (ARCHITECTURE 5).
library;

import 'package:flutter/widgets.dart' show Color;

import '../../core/domain/domain.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';

/// The label of a sandbox state, in the person's language.
String sandboxStateLabel(AppLocalizations l10n, SandboxState state) =>
    switch (state) {
      SandboxState.stopped => l10n.sandboxStateStopped,
      SandboxState.starting => l10n.sandboxStateStarting,
      SandboxState.running => l10n.sandboxStateRunning,
      SandboxState.stopping => l10n.sandboxStateStopping,
      SandboxState.failed => l10n.sandboxStateFailed,
    };

/// The hue of a sandbox state.
///
/// Colour means state and nothing else (`docs/UX.md` 3.3). Running is the
/// green of an allowed request, because both mean "this is through and it is
/// as it should be"; starting and stopping are the amber of a hold, because
/// both mean "wait, this is not settled"; failed is the hue of an error and
/// deliberately not the red of a block, which means a request was refused.
Color sandboxStateColor(HTokens tokens, SandboxState state) => switch (state) {
  SandboxState.stopped => tokens.colors.fg2,
  SandboxState.starting || SandboxState.stopping => tokens.state.held,
  SandboxState.running => tokens.state.allowed,
  SandboxState.failed => tokens.state.error,
};

/// The label of a work mode.
String sandboxWorkModeLabel(AppLocalizations l10n, WorkMode mode) =>
    switch (mode) {
      WorkMode.ro => l10n.sandboxWorkModeRo,
      WorkMode.rw => l10n.sandboxWorkModeRw,
    };

/// The label of a mount mode: what it means, not the bubblewrap flag.
///
/// The flag is in the command line, and the command line is one click away.
/// A table that repeats `--ro-bind` explains nothing to the person the screen
/// is for.
String sandboxMountModeLabel(AppLocalizations l10n, MountMode mode) =>
    switch (mode) {
      MountMode.ro => l10n.sandboxMountModeRo,
      MountMode.rw => l10n.sandboxMountModeRw,
      MountMode.tmpfs => l10n.sandboxMountModeTmpfs,
      MountMode.masked => l10n.sandboxMountModeMasked,
      MountMode.proc => l10n.sandboxMountModeProc,
      MountMode.dev => l10n.sandboxMountModeDev,
      MountMode.symlink => l10n.sandboxMountModeSymlink,
    };

/// The same label for [mount], naming where a link points.
///
/// The target of a link lies inside the sandbox, so it cannot go in the column
/// that says "on this machine" -- it would read as a host path. It goes here,
/// next to the word that says it is a link.
String sandboxMountModeText(AppLocalizations l10n, MountEntry mount) =>
    mount.mode == MountMode.symlink && mount.linkTarget.isNotEmpty
    ? l10n.sandboxMountModeSymlinkTo(mount.linkTarget)
    : sandboxMountModeLabel(l10n, mount.mode);

/// The hue of a mount mode.
///
/// Only one of the seven is coloured: a writable bind is the single line in
/// the table through which the agent can change something on this machine.
/// Colouring the other six would turn a table into a decoration and make the
/// one that matters harder to find (`docs/UX.md` 3.3).
Color sandboxMountModeColor(HTokens tokens, MountMode mode) =>
    mode.isWritable ? tokens.stateText.held : tokens.colors.fg1;

/// The label of an origin.
String sandboxOriginLabel(AppLocalizations l10n, ValueOrigin origin) =>
    switch (origin) {
      ValueOrigin.profile => l10n.sandboxOriginProfile,
      ValueOrigin.adapter => l10n.sandboxOriginAdapter,
      ValueOrigin.session => l10n.sandboxOriginSession,
      ValueOrigin.user => l10n.sandboxOriginUser,
    };

/// Whether this mode lets the agent write outside the sandbox.
extension MountModeWritable on MountMode {
  /// True for a writable bind of a host path.
  bool get isWritable => this == MountMode.rw;
}

/// [duration] as `m:ss`, or `h:mm:ss` from an hour on.
///
/// Exact and never rounded to "a few minutes": the uptime of a sandbox is
/// part of what the screen proves, and a rounded figure is a claim about
/// something nobody measured (CONVENTIONS 4.13).
String sandboxUptimeText(Duration duration) {
  final Duration clamped = duration.isNegative ? Duration.zero : duration;
  final int hours = clamped.inHours;
  final int minutes = clamped.inMinutes.remainder(60);
  final int seconds = clamped.inSeconds.remainder(60);
  final String ss = seconds.toString().padLeft(2, '0');
  if (hours == 0) {
    return '$minutes:$ss';
  }
  return '$hours:${minutes.toString().padLeft(2, '0')}:$ss';
}

/// The sentence of one guarantee.
///
/// Word for word the sentence from `docs/SECURITY.md` section 1 and
/// BACKLOG.md 4.1. The panel makes a claim the product makes elsewhere; two
/// wordings would be two claims.
String isolationCheckSentence(AppLocalizations l10n, IsolationCheck check) =>
    switch (check) {
      IsolationCheck.noNetworkInterface => l10n.isolationCheck1,
      IsolationCheck.singleSocket => l10n.isolationCheck2,
      IsolationCheck.seccompActive => l10n.isolationCheck3,
    };

/// The word for what one guarantee looks like right now.
///
/// The dot carries the colour and this carries the word. Four states and four
/// words: "not measured" is never the word for a passed check, and never its
/// colour either.
String isolationSegmentLabel(AppLocalizations l10n, IsolationSegment segment) =>
    switch (segment) {
      IsolationSegment.unknown => l10n.isolationStateUnknown,
      IsolationSegment.running => l10n.isolationStateRunning,
      IsolationSegment.passed => l10n.isolationStatePassed,
      IsolationSegment.failed => l10n.isolationStateFailed,
    };

/// The hue of one guarantee.
///
/// Red here, and not the orange of an error, because a guarantee that does
/// not hold is not a hiccup on a screen: the daemon has already killed the
/// sandbox over it, and nothing of the agent's goes anywhere. That is exactly
/// what red means in this product (`docs/UX.md` 3.3, rule 6, and the ring of
/// BACKLOG.md section 5). Unknown is `fg2`, the same grey the ring wears
/// before anything was measured -- never a shade of green.
Color isolationSegmentColor(HTokens tokens, IsolationSegment segment) =>
    switch (segment) {
      IsolationSegment.unknown => tokens.colors.fg2,
      IsolationSegment.running => tokens.state.held,
      IsolationSegment.passed => tokens.state.allowed,
      IsolationSegment.failed => tokens.state.blocked,
    };

/// Whether the dot of one guarantee is filled.
///
/// **Filled means measured.** A guarantee nobody has measured yet -- unknown,
/// or being measured right now while the sandbox comes up -- wears a ring and
/// not a disc, so the shape says what the colour says and a result that is
/// missing can never be read as a paler version of one that is there. This is
/// the same rule along the time axis as [SandboxStatus.carryChecksInto]: what
/// was not measured in this run is not shown as measured.
bool isolationSegmentFilled(IsolationSegment segment) =>
    segment == IsolationSegment.passed || segment == IsolationSegment.failed;

/// The hue the word of one guarantee may wear.
///
/// [isolationSegmentColor] is the surface palette, clamped to 3:1 and right
/// for a dot and for an arc of the ring. A sentence needs 4,5:1 and takes the
/// text palette instead (`docs/UX.md` 6).
Color isolationSegmentTextColor(HTokens tokens, IsolationSegment segment) =>
    switch (segment) {
      IsolationSegment.unknown => tokens.colors.fg2,
      IsolationSegment.running => tokens.stateText.held,
      IsolationSegment.passed => tokens.stateText.allowed,
      IsolationSegment.failed => tokens.stateText.blocked,
    };
