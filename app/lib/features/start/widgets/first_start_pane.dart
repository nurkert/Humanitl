/// The first-start screen in the projects tab (HUM-538, ADR-0024 point 3).
///
/// Collapsed it is one card: the line of defaults, "Start" with the focus
/// and "Customize…". Unfolded, the fields of the proposal stand under it,
/// one row each: a label column of fixed width and the options as one
/// segmented row, so the controls of all rows start on the same line. An
/// option the daemon calls not available stays visible and disabled, with
/// its reason under the row; a field a company policy locks stands
/// read-only with the policy named. Every word that is a value (option
/// names, the defaults line) is the daemon's; the labels are the app's.
///
/// `Enter` on "Start" starts. `Enter` on an option chooses it; the choice
/// is taken once the daemon answered the new proposal, and until then
/// "Start" waits (`FirstStart.asking`), so `Enter` in an unfolded field
/// never starts with a choice that is not committed yet.
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/cloud_inference_tag.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/first_start.dart';
import 'project_list.dart' show StartDiagnostic;
import 'start_page.dart';

/// Width of the label column of the unfolded fields.
const double _labelColumn = 132;

/// The first-start screen for [state].
class FirstStartPane extends ConsumerStatefulWidget {
  /// Creates the screen.
  const FirstStartPane({required this.state, super.key});

  /// What the screen shows.
  final FirstStart state;

  @override
  ConsumerState<FirstStartPane> createState() => _FirstStartPaneState();
}

class _FirstStartPaneState extends ConsumerState<FirstStartPane> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final FirstStart state = widget.state;
    final StartProposal? proposal = state.proposal;
    final FirstStartNotifier notifier = ref.read(firstStartProvider.notifier);
    final String name = proposal?.displayName ?? '';
    return StartPage(
      key: const Key('first-start'),
      title: name.isEmpty ? l10n.startNavProjects : name,
      subtitle: HMiddleEllipsis(
        proposal?.path ?? state.path,
        key: const Key('first-start-path'),
        style: tokens.typography.mono11.tinted(tokens.colors.fg2),
      ),
      intro: state.projectId.isEmpty
          ? l10n.firstStartIntro
          : l10n.firstStartChangeIntro,
      action: HButton(
        key: const Key('first-start-cancel'),
        variant: HButtonVariant.ghost,
        onPressed: notifier.cancel,
        child: Text(l10n.firstStartCancel),
      ),
      children: <Widget>[
        for (final Diagnostic diagnostic in <Diagnostic>[
          ...?proposal?.diagnostics,
          ...state.diagnostics,
        ])
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x3),
            child: StartDiagnostic(diagnostic: diagnostic),
          ),
        if (state.otherStartBusy)
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x3),
            child: Text(
              l10n.firstStartOtherStartBusy,
              key: const Key('first-start-other-start-busy'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          ),
        if (proposal == null)
          state.asking ? const HSkeleton(rows: 2) : const SizedBox.shrink()
        else ...<Widget>[
          _Summary(
            proposal: proposal,
            state: state,
            expanded: _expanded,
            onToggle: () => setState(() => _expanded = !_expanded),
            onStart: () => unawaited(notifier.start()),
          ),
          if (_expanded) ...<Widget>[
            SizedBox(height: tokens.spacing.x5),
            StartSectionLabel(text: l10n.firstStartChoiceTitle),
            _Fields(proposal: proposal, enabled: !state.saving),
          ],
        ],
      ],
    );
  }
}

/// The collapsed screen: the defaults over "Start" and "Customize…".
class _Summary extends StatelessWidget {
  const _Summary({
    required this.proposal,
    required this.state,
    required this.expanded,
    required this.onToggle,
    required this.onStart,
  });

  final StartProposal proposal;
  final FirstStart state;
  final bool expanded;
  final VoidCallback onToggle;
  final VoidCallback onStart;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final CloudInference? cloud = proposal.cloudInference;
    return StartCard(
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x5),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              l10n.firstStartDefaults.toUpperCase(),
              style: tokens.typography.ui11
                  .copyWith(fontWeight: FontWeight.w600, letterSpacing: 0.6)
                  .tinted(tokens.colors.fg2),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              defaultsText(proposal),
              key: const Key('first-start-defaults'),
              style: tokens.typography.ui14.tinted(tokens.colors.fg0),
            ),
            if (cloud != null) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              Align(
                alignment: Alignment.centerLeft,
                child: CloudInferenceTag(cloud: cloud),
              ),
            ],
            SizedBox(height: tokens.spacing.x5),
            Row(
              children: <Widget>[
                HButton(
                  key: const Key('first-start-start'),
                  variant: HButtonVariant.primary,
                  size: HButtonSize.md,
                  autofocus: true,
                  onPressed: state.canStart ? onStart : null,
                  child: Text(
                    state.saving ? l10n.startStarting : l10n.startStart,
                  ),
                ),
                SizedBox(width: tokens.spacing.x2),
                HButton(
                  key: const Key('first-start-customize'),
                  size: HButtonSize.md,
                  onPressed: onToggle,
                  leading: Transform.rotate(
                    angle: expanded ? math.pi / 2 : 0,
                    child: HGlyphIcon(
                      HGlyph.chevronRight,
                      size: 14,
                      color: tokens.colors.fg1,
                    ),
                  ),
                  child: Text(l10n.firstStartCustomize),
                ),
                const Spacer(),
                ExcludeSemantics(child: HKeyHint(l10n.firstStartEnterKey)),
                SizedBox(width: tokens.spacing.x1),
                Text(
                  l10n.firstStartEnterHint,
                  style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// The defaults line of the daemon without its lead word `Defaults: `; the
/// card says that itself, in the person's language.
String defaultsText(StartProposal proposal) {
  const String lead = 'Defaults: ';
  final String line = proposal.defaultsLine;
  return line.startsWith(lead) ? line.substring(lead.length) : line;
}

/// The unfolded fields, one row each, divided by hairlines.
class _Fields extends StatelessWidget {
  const _Fields({required this.proposal, required this.enabled});

  final StartProposal proposal;
  final bool enabled;

  @override
  Widget build(BuildContext context) => StartCard(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final (int index, ProposalField field)
            in proposal.fields.indexed) ...<Widget>[
          if (index > 0) const HHairline(),
          _FieldRow(field: field, enabled: enabled),
        ],
      ],
    ),
  );
}

/// The label of a field in the person's language; an id this build does
/// not know keeps the daemon's id.
String fieldLabel(AppLocalizations l10n, String id) => switch (id) {
  'agent' => l10n.firstStartFieldAgent,
  'folder_mode' => l10n.firstStartFieldFolder,
  'isolation' => l10n.firstStartFieldIsolation,
  'ask_mode' => l10n.firstStartFieldAsk,
  'llm_server' => l10n.firstStartFieldServer,
  'profile' => l10n.firstStartFieldProfile,
  _ => id,
};

/// Where the program of the chosen agent comes from, in the person's
/// language; the daemon names the source `host` or `depot`.
String sourceLabel(AppLocalizations l10n, String source) => switch (source) {
  'depot' => l10n.firstStartSourceDepot,
  _ => l10n.firstStartSourceHost,
};

/// The short name of a source of an agent program.
String sourceName(AppLocalizations l10n, String source) => switch (source) {
  'depot' => l10n.firstStartSourceNameDepot,
  _ => l10n.firstStartSourceNameHost,
};

/// One field: label, options, and the lines under them.
class _FieldRow extends ConsumerWidget {
  const _FieldRow({required this.field, required this.enabled});

  final ProposalField field;
  final bool enabled;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final ProposalOption? chosen = field.chosen;
    final SettingsLock? lock = field.lock;
    final List<Widget> notes = <Widget>[
      if (lock != null)
        _Note(
          key: Key('first-start-locked-${field.id}'),
          glyph: HGlyph.lock,
          color: tokens.colors.fg2,
          text: l10n.firstStartLockedBy(lock.lockedBy),
        ),
      if (field.id == 'agent' && chosen != null)
        ..._sourceNotes(tokens, l10n, chosen),
      if (lock == null)
        for (final ProposalOption option in field.options)
          if (!option.available)
            _Note(
              key: Key('first-start-reason-${field.id}-${option.value}'),
              glyph: HGlyph.triangleAlert,
              color: tokens.colors.warning,
              text: l10n.firstStartUnavailable(
                option.label,
                _reason(l10n, option.reasons),
              ),
            ),
    ];
    final Widget control = switch (field) {
      _ when lock != null => _LockedValue(field: field, chosen: chosen),
      _ when field.options.isEmpty => Text(
        l10n.firstStartNoOption,
        style: tokens.typography.ui13.tinted(tokens.colors.fg2),
      ),
      _ => SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: HSegmented<String>(
          key: Key('first-start-options-${field.id}'),
          enabled: enabled,
          selected: field.value,
          options: <HSegmentOption<String>>[
            for (final ProposalOption option in field.options)
              HSegmentOption<String>(
                value: option.value,
                label: option.label,
                enabled: option.available,
              ),
          ],
          onSelect: (String value) {
            for (final ProposalOption option in field.options) {
              if (option.value == value) {
                unawaited(
                  ref.read(firstStartProvider.notifier).choose(field, option),
                );
              }
            }
          },
        ),
      ),
    };
    // Beschriftung und Control stehen in einer Zeile und auf derselben
    // Mitte; die Hinweise darunter beginnen an der Kante der Controls.
    return Padding(
      key: Key('first-start-field-${field.id}'),
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x4,
        vertical: tokens.spacing.x3,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          ConstrainedBox(
            constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
            child: Row(
              children: <Widget>[
                SizedBox(
                  width: _labelColumn,
                  child: Text(
                    fieldLabel(l10n, field.id),
                    style: tokens.typography.ui13
                        .copyWith(fontWeight: FontWeight.w600)
                        .tinted(tokens.colors.fg1),
                  ),
                ),
                Flexible(child: control),
              ],
            ),
          ),
          for (final Widget note in notes)
            Padding(
              padding: EdgeInsets.only(
                left: _labelColumn,
                top: tokens.spacing.x2,
              ),
              child: note,
            ),
        ],
      ),
    );
  }

  /// Both sources of the chosen agent with their state, read-only until
  /// the source becomes part of the choice (HUM-554): the one a start uses
  /// first, then the other; a source without the program names why.
  static List<Widget> _sourceNotes(
    HTokens tokens,
    AppLocalizations l10n,
    ProposalOption chosen,
  ) {
    if (chosen.sources.isEmpty) {
      return <Widget>[
        if (chosen.source.isNotEmpty)
          _Note(
            key: Key('first-start-source-${chosen.source}'),
            glyph: HGlyph.info,
            color: tokens.colors.fg2,
            text: sourceLabel(l10n, chosen.source),
          ),
      ];
    }
    final List<AgentSourceOption> ordered = <AgentSourceOption>[
      ...chosen.sources.where((AgentSourceOption s) => s.selected),
      ...chosen.sources.where((AgentSourceOption s) => !s.selected),
    ];
    return <Widget>[
      for (final AgentSourceOption source in ordered)
        _Note(
          key: Key('first-start-source-${source.source}'),
          glyph: source.available ? HGlyph.info : HGlyph.triangleAlert,
          color: source.available ? tokens.colors.fg2 : tokens.colors.warning,
          text: switch ((source.selected, source.available)) {
            (true, true) => sourceLabel(l10n, source.source),
            (true, false) => l10n.firstStartUnavailable(
              sourceLabel(l10n, source.source),
              _reason(l10n, source.reasons),
            ),
            (false, true) => l10n.firstStartSourceReady(
              sourceName(l10n, source.source),
            ),
            (false, false) => l10n.firstStartUnavailable(
              sourceName(l10n, source.source),
              _reason(l10n, source.reasons),
            ),
          },
        ),
    ];
  }

  /// The cause of the first reason in the person's language.
  static String _reason(AppLocalizations l10n, List<Diagnostic> reasons) {
    if (reasons.isEmpty) {
      return '';
    }
    final DiagnosticText text = DiagnosticL10n.resolve(reasons.first, l10n);
    return text.causeWithOriginal(l10n);
  }
}

/// A locked field: the value read-only, with the badge.
class _LockedValue extends StatelessWidget {
  const _LockedValue({required this.field, required this.chosen});

  final ProposalField field;
  final ProposalOption? chosen;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Flexible(
          child: Text(
            chosen?.label ?? field.value,
            key: Key('first-start-value-${field.id}'),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tokens.typography.ui13.tinted(tokens.colors.fg0),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        HBadge(text: l10n.firstStartLocked, color: tokens.colors.fg2),
      ],
    );
  }
}

/// One line under a field: a glyph and a sentence.
class _Note extends StatelessWidget {
  const _Note({
    required this.glyph,
    required this.color,
    required this.text,
    super.key,
  });

  final HGlyph glyph;
  final Color color;
  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(top: 2),
          child: HGlyphIcon(glyph, size: 12, color: color),
        ),
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          child: Text(
            text,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ),
      ],
    );
  }
}
