/// The pieces of the backend choice that the settings and the sandbox screen
/// share (HUM-440): the segments, the words for an isolation level, and the
/// lines under the choice.
///
/// Lives in `core/ui` because both features draw the choice and no feature
/// imports another one (ARCHITECTURE 5). Nothing here judges a backend: the
/// segments, whether one is selectable, its level, why it cannot start and
/// the lower bound all come from `SandboxBackends` (ADR-018). The only rule
/// of this file is the one the daemon names itself: a backend it does not
/// admit (`SANDBOX_048`) is no choice at all ([SandboxBackendList.offered]).
library;

import 'package:flutter/widgets.dart';

import '../domain/domain.dart';
import '../../l10n/l10n.dart';
import 'fix_control.dart';
import 'ui.dart';

/// The sentence for [level], as it follows the name of a backend.
String isolationLevelText(AppLocalizations l10n, IsolationLevel? level) =>
    switch (level) {
      IsolationLevel.policy => l10n.sandboxBackendLevelPolicy,
      IsolationLevel.namespaces => l10n.sandboxBackendLevelNamespaces,
      IsolationLevel.kernel => l10n.sandboxBackendLevelKernel,
      null => l10n.sandboxBackendLevelUnknown,
    };

/// The short word for [level] in a badge, or null when the daemon named no
/// level this app knows.
String? isolationLevelShort(AppLocalizations l10n, IsolationLevel? level) =>
    switch (level) {
      IsolationLevel.policy => l10n.sandboxBackendShortPolicy,
      IsolationLevel.namespaces => l10n.sandboxBackendShortNamespaces,
      IsolationLevel.kernel => l10n.sandboxBackendShortKernel,
      null => null,
    };

/// The segments of the choice: `auto` first, then every backend the daemon
/// admits, by the name `sandbox.backend` writes. A backend that cannot start
/// now stays in its place but cannot be chosen; why, the lines of
/// [SandboxBackendLines] say. `auto` cannot be chosen either when the
/// backend it means cannot start (`SandboxBackendList.auto_backend`, HUM-440);
/// its reason stands under that backend's line.
List<HSegmentOption<String>> sandboxBackendOptions(
  AppLocalizations l10n,
  SandboxBackendList list,
) => <HSegmentOption<String>>[
  HSegmentOption<String>(
    value: FormKeys.sandboxBackendAuto,
    label: l10n.sandboxBackendAuto,
    enabled: list.autoAvailable,
  ),
  for (final SandboxBackendInfo backend in list.offered)
    HSegmentOption<String>(
      value: backend.name,
      label: backend.name,
      enabled: backend.available,
    ),
];

/// Whether [list] leaves anything to choose: two backends at least. Under
/// macOS the daemon lists Seatbelt alone, and a choice between `auto` and
/// the one backend `auto` means would be no choice. Only the segments hang
/// on this; the lines of [SandboxBackendLines] stand either way.
bool offersBackendChoice(SandboxBackendList list) => list.offered.length > 1;

/// One line per backend the daemon admits: its name and isolation level, and
/// under it, where it cannot start now, the daemon's reason and fix.
/// Optionally the lower bound `sandbox.min_isolation` at the end.
class SandboxBackendLines extends StatelessWidget {
  /// Creates the lines for [list]. [keyPrefix] makes the keys of the lines
  /// unique per screen, for tests.
  const SandboxBackendLines({
    required this.list,
    required this.keyPrefix,
    this.showMinimum = false,
    super.key,
  });

  /// What `SandboxBackends` answered.
  final SandboxBackendList list;

  /// The start of every key under these lines.
  final String keyPrefix;

  /// Whether the lower bound stands under the backends.
  final bool showMinimum;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final IsolationLevel? minimum = list.minIsolation;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        for (final SandboxBackendInfo backend in list.offered) ...<Widget>[
          Text(
            l10n.sandboxBackendLine(
              backend.name,
              isolationLevelText(l10n, backend.isolation),
            ),
            key: ValueKey<String>('$keyPrefix-level-${backend.name}'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          if (backend.reason case final Diagnostic reason)
            _Reason(backend: backend.name, reason: reason, keyPrefix: keyPrefix)
          else if (!backend.available)
            // Nicht verfügbar, aber ohne Befund: Der Daemon nennt keinen
            // Grund, und die Zeile sagt wenigstens, dass es nicht geht.
            Padding(
              padding: EdgeInsetsDirectional.only(
                start: tokens.spacing.x3,
                top: tokens.spacing.x1,
                bottom: tokens.spacing.x1,
              ),
              child: Text(
                l10n.sandboxBackendUnavailable,
                key: ValueKey<String>('$keyPrefix-reason-${backend.name}'),
                style: tokens.typography.ui12.tinted(tokens.colors.warningText),
              ),
            ),
        ],
        if (showMinimum && minimum != null)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: Text(
              l10n.sandboxBackendMinIsolation(
                isolationLevelText(l10n, minimum),
              ),
              key: ValueKey<String>('$keyPrefix-minimum'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
      ],
    );
  }
}

/// Why a backend cannot start now, indented under its line, with the fix of
/// the daemon as the same control every diagnostic card has.
class _Reason extends StatelessWidget {
  const _Reason({
    required this.backend,
    required this.reason,
    required this.keyPrefix,
  });

  final String backend;
  final Diagnostic reason;
  final String keyPrefix;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final FixAction? fix = reason.fix;
    return Padding(
      padding: EdgeInsetsDirectional.only(
        start: tokens.spacing.x3,
        top: tokens.spacing.x1,
        bottom: tokens.spacing.x1,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            DiagnosticL10n.resolve(reason, l10n).cause,
            key: ValueKey<String>('$keyPrefix-reason-$backend'),
            style: tokens.typography.ui12.tinted(tokens.colors.warningText),
          ),
          if (fix != null)
            Padding(
              padding: EdgeInsets.only(top: tokens.spacing.x1),
              child: FixControl(
                fix: fix,
                diagnosticCode: reason.code,
                copyKey: ValueKey<String>('$keyPrefix-fix-$backend'),
              ),
            ),
        ],
      ),
    );
  }
}
