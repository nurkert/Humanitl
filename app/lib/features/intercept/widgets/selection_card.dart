/// What a decision over more than one request is about (`docs/UX.md` 3.5).
///
/// As soon as a group is selected the action bar decides for all of it, and
/// the card beside it has to say what "all of it" means: host, method mix,
/// paths, findings. A card that kept showing one of twelve URLs while the bar
/// sends twelve would be the dark pattern of `backlog/CONVENTIONS.md` 4.13.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/held_groups.dart';
import 'group_header_row.dart';

/// The summary of a multi-selection.
class SelectionCard extends StatelessWidget {
  /// Creates the card for [flows].
  const SelectionCard({required this.flows, super.key});

  /// Every request the next decision covers, in queue order.
  final List<Flow> flows;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HeldGroups groups = groupFlows(flows);
    final int findings = groups.groups.fold(
      0,
      (int sum, HeldGroup group) => sum + group.findingsTotal,
    );
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            l10n.interceptSelectionTitle(flows.length),
            key: const Key('intercept-selection-title'),
            style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x2),
          for (final HeldGroup group in groups.groups)
            Padding(
              padding: EdgeInsets.only(bottom: tokens.spacing.x1),
              child: Text(
                l10n.interceptGroupSummary(
                  groupTitle(group, l10n),
                  group.length,
                  methodMix(group, l10n),
                  l10n.interceptGroupFindings(group.findingsTotal),
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            ),
          if (findings > 0) ...<Widget>[
            SizedBox(height: tokens.spacing.x2),
            HBadge(
              key: const Key('intercept-selection-findings'),
              text: l10n.interceptGroupFindings(findings),
              color: tokens.state.error,
            ),
          ],
          SizedBox(height: tokens.spacing.x3),
          const HHairline(),
          SizedBox(height: tokens.spacing.x2),
          // Every path, not a sample: this is the one place where the whole
          // reach of the next decision can be read.
          Expanded(
            child: ListView.builder(
              itemCount: flows.length,
              // No `itemExtent`: a fixed height for a line of text cuts it off
              // as soon as somebody scales the type (`docs/UX.md` 6). The list
              // is as long as the selection, so the cost of measuring is the
              // cost of what is on the screen.
              itemBuilder: (BuildContext context, int index) {
                final Flow flow = flows[index];
                return Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    '${flow.methodLabel} ${flow.host}${flow.path}',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
