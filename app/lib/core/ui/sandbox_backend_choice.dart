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

/// The segments of [sandboxBackendOptions] for [list], as the settings row
/// and the isolation tab both show them (HUM-560).
///
/// While the answer for a new plan is on its way the segments are locked
/// ([enabled] false, HUM-504, HUM-510), and a locked segment loses the
/// focus. Whoever had just chosen with the keyboard gets it back on the
/// selected segment once the segments are enabled again; focus that was
/// elsewhere when they were locked, or that someone moved while they were
/// locked, is never pulled in.
class SandboxBackendSegments extends StatefulWidget {
  /// Creates the segments for [list]. [segmentsKey] is the key of the
  /// [HSegmented] inside.
  const SandboxBackendSegments({
    required this.list,
    required this.selected,
    required this.onSelect,
    required this.enabled,
    this.segmentsKey,
    super.key,
  });

  /// What `SandboxBackends` answered.
  final SandboxBackendList list;

  /// The value of the selected segment.
  final String selected;

  /// Called with the value of a chosen segment.
  final ValueChanged<String> onSelect;

  /// False while the list belongs to a plan that is no longer current.
  final bool enabled;

  /// The key of the [HSegmented], for tests.
  final Key? segmentsKey;

  @override
  State<SandboxBackendSegments> createState() => _SandboxBackendSegmentsState();
}

class _SandboxBackendSegmentsState extends State<SandboxBackendSegments> {
  /// Umschließt die Segmente: Hat einer von ihnen den Fokus, hat ihn auch
  /// dieser Knoten.
  final FocusNode _choice = FocusNode(
    debugLabel: 'sandbox-backend-choice',
    canRequestFocus: false,
    skipTraversal: true,
  );

  /// Ob die Segmente den Fokus hatten, als sie gesperrt wurden.
  bool _refocusOnUnlock = false;

  /// Ob die Sperre den Fokus schon fortgenommen hat; dann steht in
  /// [_fallback], wohin er fiel.
  bool _lost = false;

  /// Der Knoten, auf den der Fokus fiel, als die Sperre ihn den Segmenten
  /// nahm, oder null.
  FocusNode? _fallback;

  /// Ob der Fokus während der Sperre [_fallback] verlassen hat. Dann hat
  /// ihn jemand bewegt, und die Rückgabe entfällt, auch wenn er bis zum
  /// Entsperren wieder dort steht.
  bool _moved = false;

  /// Ob [_onPrimaryFocusChange] am Fokus-Manager hängt.
  bool _watching = false;

  @override
  void initState() {
    super.initState();
    _choice.addListener(_onFocusChange);
  }

  /// Hält fest, wohin der Fokus fiel, als die Sperre ihn nahm. Der Aufruf
  /// kommt, nachdem der Fokus-Manager den neuen Fokus gesetzt hat.
  void _onFocusChange() {
    if (_refocusOnUnlock && !_lost && !_choice.hasFocus) {
      _lost = true;
      _fallback = FocusManager.instance.primaryFocus;
      _watching = true;
      FocusManager.instance.addListener(_onPrimaryFocusChange);
    }
  }

  /// Jede Bewegung des Fokus weg von [_fallback] während der Sperre hebt
  /// die Rückgabe endgültig auf.
  void _onPrimaryFocusChange() {
    if (FocusManager.instance.primaryFocus != _fallback) {
      _moved = true;
      _stopWatching();
    }
  }

  void _stopWatching() {
    if (_watching) {
      _watching = false;
      FocusManager.instance.removeListener(_onPrimaryFocusChange);
    }
  }

  @override
  void didUpdateWidget(SandboxBackendSegments oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.enabled && !widget.enabled) {
      // Die Segmente bauen erst nach diesem Aufruf gesperrt; bis dahin hält
      // eines von ihnen den Fokus noch, wenn die Wahl ihn hatte.
      _stopWatching();
      _refocusOnUnlock = _choice.hasFocus;
      _lost = false;
      _fallback = null;
      _moved = false;
    } else if (!oldWidget.enabled && widget.enabled && _refocusOnUnlock) {
      _refocusOnUnlock = false;
      _stopWatching();
      // Erst nach dem Bild sind die Segmente wieder fokussierbar.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        // Hat jemand den Fokus während der Sperre bewegt (Tab, Klick), bleibt
        // er, wo er ist, auch wenn er inzwischen dorthin zurückkam, wohin die
        // Sperre ihn schob.
        final bool untouched =
            !_moved &&
            (!_lost || FocusManager.instance.primaryFocus == _fallback);
        _fallback = null;
        if (mounted && untouched) {
          _restoreFocus();
        }
      });
    }
  }

  @override
  void dispose() {
    _stopWatching();
    _choice
      ..removeListener(_onFocusChange)
      ..dispose();
    super.dispose();
  }

  /// Gibt den Fokus dem gewählten Segment zurück, oder dem ersten, wenn
  /// keines gewählt ist.
  void _restoreFocus() {
    final List<FocusNode> segments = _choice.traversalDescendants.toList();
    if (segments.isEmpty) {
      return;
    }
    final FocusNode target = segments.firstWhere(
      (FocusNode node) =>
          node.context
              ?.findAncestorWidgetOfExactType<HSegment<String>>()
              ?.selected ??
          false,
      orElse: () => segments.first,
    );
    target.requestFocus();
  }

  @override
  Widget build(BuildContext context) => Focus(
    focusNode: _choice,
    child: HSegmented<String>(
      key: widget.segmentsKey,
      options: sandboxBackendOptions(context.l10n, widget.list),
      selected: widget.selected,
      onSelect: widget.onSelect,
      enabled: widget.enabled,
    ),
  );
}

/// One line per backend the daemon admits: its name and isolation level, and
/// under it, where it cannot start now, the daemon's reason and fix, or that
/// the daemon is still checking it (HUM-502).
/// Optionally the lower bound `sandbox.min_isolation` at the end.
class SandboxBackendLines extends StatelessWidget {
  /// Creates the lines for [list]. [keyPrefix] makes the keys of the lines
  /// unique per screen, for tests.
  const SandboxBackendLines({
    required this.list,
    required this.keyPrefix,
    this.showMinimum = false,
    this.pending = false,
    super.key,
  });

  /// What `SandboxBackends` answered.
  final SandboxBackendList list;

  /// The start of every key under these lines.
  final String keyPrefix;

  /// Whether the lower bound stands under the backends.
  final bool showMinimum;

  /// Whether [list] belongs to a plan that is no longer current, while the
  /// answer for the new one is on its way (HUM-504): every line then stands
  /// in the muted colour of "still being checked", so the old availability,
  /// reasons and lower bound do not read as the current ones.
  final bool pending;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final IsolationLevel? minimum = list.minIsolation;
    final Color lineColor = pending ? tokens.colors.fg2 : tokens.colors.fg1;
    final Color warningColor = pending
        ? tokens.colors.fg2
        : tokens.colors.warningText;
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
            style: tokens.typography.ui12.tinted(lineColor),
          ),
          if (backend.undecided)
            // Die Suche des Daemons lief noch (HUM-502): weder verfügbar
            // noch fehlend. Die Liste fragt gleich wieder.
            Padding(
              padding: EdgeInsetsDirectional.only(
                start: tokens.spacing.x3,
                top: tokens.spacing.x1,
                bottom: tokens.spacing.x1,
              ),
              child: Text(
                l10n.sandboxBackendChecking,
                key: ValueKey<String>('$keyPrefix-checking-${backend.name}'),
                style: tokens.typography.ui12.tinted(tokens.colors.fg2),
              ),
            )
          else if (backend.reason case final Diagnostic reason)
            _Reason(
              backend: backend.name,
              reason: reason,
              keyPrefix: keyPrefix,
              color: warningColor,
            )
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
                style: tokens.typography.ui12.tinted(warningColor),
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
              style: tokens.typography.ui12.tinted(lineColor),
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
    required this.color,
  });

  final String backend;
  final Diagnostic reason;
  final String keyPrefix;

  /// The colour of the reason: the warning, or muted while pending.
  final Color color;

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
            style: tokens.typography.ui12.tinted(color),
          ),
          if (fix != null)
            Padding(
              padding: EdgeInsets.only(top: tokens.spacing.x1),
              child: FixControl(
                fix: fix,
                diagnosticCode: reason.code,
                // Was jemand kopieren soll, steht ganz da, auch ein langer
                // Pfad zum Kernel-Abbild (HUM-494).
                wrapText: true,
                copyKey: ValueKey<String>('$keyPrefix-fix-$backend'),
              ),
            ),
        ],
      ),
    );
  }
}
