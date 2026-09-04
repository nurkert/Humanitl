/// Every environment variable the sandbox sets (HUM-040).
///
/// # Why most values are withheld, and why none of them can be revealed
///
/// The daemon sends the value of a variable only when the screen needs it as
/// proof -- proxy, certificates, paths, language, and what the adapter sets to
/// steer the agent. Everything else arrives with the mark `withheld` and no
/// value at all. The rule runs that way round on purpose: a list of suspicious
/// names is always incomplete, and the gaps are the dangerous ones
/// (`AWS_ACCESS_KEY_ID` ends in `_ID`, `DATABASE_URL` carries the password in
/// the URL). A new variable is therefore silent rather than accidentally
/// visible (CONVENTIONS 4.17).
///
/// There is consequently nothing this window could uncover, and a control that
/// promised to uncover it would be a lie -- the worst kind on a screen whose
/// whole purpose is to be believed (CONVENTIONS 4.13).
///
/// What the tab does guarantee is the other half of the promise: a withheld
/// value must never look like an empty one. It is drawn as dots in the error
/// hue with the word for it beside them; a variable that really carries
/// nothing says so in words.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../sandbox_text.dart';
import 'sandbox_table.dart';

/// How many dots a withheld value is drawn with.
///
/// A fixed number, not the length of the value: the length of a secret is
/// itself a fact about it, and the daemon does not send that either.
const int envMaskDots = 8;

/// The mark a withheld value is drawn as.
final String envMask = '•' * envMaskDots;

/// The environment tab.
class EnvTab extends StatefulWidget {
  /// Shows the environment of [status].
  const EnvTab({required this.status, super.key});

  /// What the daemon last said.
  final SandboxStatus status;

  @override
  State<EnvTab> createState() => _EnvTabState();
}

class _EnvTabState extends State<EnvTab> {
  final TextEditingController _filter = TextEditingController();

  @override
  void dispose() {
    _filter.dispose();
    super.dispose();
  }

  List<EnvEntry> get _shown {
    final String query = _filter.text.trim().toLowerCase();
    if (query.isEmpty) {
      return widget.status.env;
    }
    return widget.status.env
        .where((EnvEntry entry) => entry.key.toLowerCase().contains(query))
        .toList();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<EnvEntry> shown = _shown;
    final bool anyWithheld = widget.status.env.any(
      (EnvEntry entry) => entry.withheld,
    );
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
          child: Row(
            children: <Widget>[
              Expanded(
                child: HTextField(
                  controller: _filter,
                  semanticsLabel: l10n.sandboxEnvSearchHint,
                  hint: l10n.sandboxEnvSearchHint,
                  mono: true,
                  onChanged: (String _) => setState(() {}),
                ),
              ),
              SizedBox(width: tokens.spacing.x3),
              Text(
                l10n.sandboxEnvCount(shown.length, widget.status.env.length),
                style: tokens.typography.ui11.tinted(tokens.colors.fg2),
              ),
            ],
          ),
        ),
        if (anyWithheld)
          Padding(
            padding: EdgeInsets.fromLTRB(
              tokens.spacing.x3,
              0,
              tokens.spacing.x3,
              tokens.spacing.x2,
            ),
            child: Text(
              l10n.sandboxEnvMaskedWhy,
              key: const Key('sandbox-env-masked-why'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
        const HHairline(),
        Expanded(child: _body(tokens, l10n, shown)),
      ],
    );
  }

  Widget _body(HTokens tokens, AppLocalizations l10n, List<EnvEntry> shown) {
    if (widget.status.env.isEmpty) {
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Text(
          l10n.sandboxEnvEmptyAll,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      );
    }
    if (shown.isEmpty) {
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Text(
          l10n.sandboxEnvNoMatch(widget.status.env.length),
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      );
    }
    return SandboxTable(
      scrollKey: const PageStorageKey<String>('sandbox-env'),
      columns: <String>[
        l10n.sandboxEnvColKey,
        l10n.sandboxEnvColValue,
        l10n.sandboxEnvColOrigin,
      ],
      rows: <SandboxRowData>[
        for (final EnvEntry entry in shown)
          SandboxRowData(
            key: ValueKey<String>('env-${entry.key}'),
            cells: <SandboxCell>[
              SandboxCell(
                entry.key,
                mono: true,
                strong: true,
                color: tokens.colors.fg0,
              ),
              _value(tokens, l10n, entry),
              SandboxCell(
                sandboxOriginLabel(l10n, entry.origin),
                color: tokens.colors.fg2,
              ),
            ],
          ),
      ],
    );
  }

  /// The value cell. Three shapes, and never two of them alike: a real value,
  /// a withheld one, an empty one.
  SandboxCell _value(HTokens tokens, AppLocalizations l10n, EnvEntry entry) {
    if (entry.isMasked) {
      return SandboxCell(
        '$envMask  ${l10n.sandboxEnvMasked}',
        mono: true,
        color: tokens.stateText.error,
      );
    }
    if (entry.isEmpty) {
      return SandboxCell(l10n.sandboxEnvEmpty, color: tokens.colors.fg2);
    }
    return SandboxCell(entry.value, mono: true);
  }
}
