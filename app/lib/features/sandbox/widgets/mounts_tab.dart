/// Every path the agent can see, and the sentence that sums it up (HUM-040).
///
/// The sentence is the answer to the question the person actually asks; the
/// table under it is the proof, and it lists every path without exception.
/// The order is the order of the command line, because that is where the rows
/// were read from -- a table sorted for looks would no longer line up with
/// the command it claims to describe.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../sandbox_text.dart';
import 'sandbox_table.dart';
import 'work_dir_picker.dart';

/// The mounts tab.
class MountsTab extends StatelessWidget {
  /// Shows the mounts of [status].
  const MountsTab({required this.status, super.key});

  /// What the daemon last said.
  final SandboxStatus status;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<MountEntry> extra = status.extraHostPaths;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.fromLTRB(
            tokens.spacing.x3,
            tokens.spacing.x3,
            tokens.spacing.x3,
            tokens.spacing.x2,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              // The largest text of this tab, because it is the most
              // important thing on it (`docs/UX.md` 3.1).
              Text(
                workDirSentence(l10n, status),
                key: const Key('sandbox-mounts-sentence'),
                style: tokens.typography.ui14.tinted(tokens.colors.fg0),
              ),
              if (extra.isNotEmpty) ...<Widget>[
                SizedBox(height: tokens.spacing.x2),
                Text(
                  l10n.sandboxMountsExtra(extra.length),
                  style: tokens.typography.ui12.tinted(tokens.state.held),
                ),
                SizedBox(height: tokens.spacing.x1),
                for (final MountEntry mount in extra)
                  Padding(
                    padding: EdgeInsets.only(bottom: tokens.spacing.x1),
                    child: Text(
                      '${mount.src} '
                      '(${sandboxMountModeLabel(l10n, mount.mode)})',
                      style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                    ),
                  ),
              ],
              SizedBox(height: tokens.spacing.x2),
              Text(
                l10n.sandboxMountsSystemNote,
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            ],
          ),
        ),
        const HHairline(),
        Expanded(
          child: status.mounts.isEmpty
              ? _Empty(text: l10n.sandboxMountsEmpty)
              : SandboxTable(
                  scrollKey: const PageStorageKey<String>('sandbox-mounts'),
                  columns: <String>[
                    l10n.sandboxMountsColDst,
                    l10n.sandboxMountsColSrc,
                    l10n.sandboxMountsColMode,
                    l10n.sandboxMountsColOrigin,
                  ],
                  rows: <SandboxRowData>[
                    for (final MountEntry mount in status.mounts)
                      SandboxRowData(
                        key: ValueKey<String>('mount-${mount.dst}'),
                        cells: <SandboxCell>[
                          SandboxCell(
                            mount.dst,
                            mono: true,
                            strong: true,
                            color: tokens.colors.fg0,
                          ),
                          mount.hasHostPath
                              ? SandboxCell(mount.src, mono: true)
                              : SandboxCell(
                                  l10n.sandboxMountNoHostPath,
                                  color: tokens.colors.fg2,
                                ),
                          SandboxCell(
                            sandboxMountModeText(l10n, mount),
                            color: sandboxMountModeColor(tokens, mount.mode),
                            strong: mount.mode.isWritable,
                          ),
                          SandboxCell(
                            sandboxOriginLabel(l10n, mount.origin),
                            color: tokens.colors.fg2,
                          ),
                        ],
                      ),
                  ],
                ),
        ),
        const HHairline(),
        Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x1,
          ),
          child: Text(
            l10n.sandboxMountsCount(status.mounts.length),
            style: tokens.typography.ui11.tinted(tokens.colors.fg2),
          ),
        ),
      ],
    );
  }
}

/// An empty state that says why it is empty.
class _Empty extends StatelessWidget {
  const _Empty({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Text(
        text,
        style: tokens.typography.ui13.tinted(tokens.colors.fg1),
      ),
    );
  }
}
