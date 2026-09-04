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
