/// The heading of a section of the Models page and the page without any
/// server (HUM-414).
library;

import 'package:flutter/widgets.dart';

import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';

/// The heading of a section of the page, with one sentence under it.
class ServerSectionHead extends StatelessWidget {
  /// [title] over [help].
  const ServerSectionHead({required this.title, required this.help, super.key});

  /// The heading.
  final String title;

  /// The sentence under it.
  final String help;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Semantics(
          header: true,
          child: Text(
            title,
            style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
          ),
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(help, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
      ],
    );
  }
}

/// The page without any server: what that means, the one way on, and where
/// a local server usually listens. Nothing is looked for on the network.
class ServerEmptyState extends StatelessWidget {
  /// The page without a server; [onAdd] adds one.
  const ServerEmptyState({required this.onAdd, super.key});

  /// "Add server".
  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return DecoratedBox(
      key: const Key('settings-server-empty'),
      decoration: BoxDecoration(
        border: Border.all(color: tokens.colors.line),
        borderRadius: BorderRadius.circular(tokens.radii.card),
      ),
      child: Padding(
        padding: EdgeInsets.symmetric(
          horizontal: tokens.spacing.x4,
          vertical: tokens.spacing.x6,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              l10n.settingsServerEmptyTitle,
              style: tokens.typography.ui13.tinted(tokens.colors.fg0),
            ),
            SizedBox(height: tokens.spacing.x3),
            HButton(
              key: const Key('settings-server-add'),
              variant: HButtonVariant.primary,
              autofocus: false,
              leading: HGlyphIcon(
                HGlyph.plus,
                size: 12,
                color: tokens.colors.onAccent,
              ),
              onPressed: onAdd,
              child: Text(l10n.settingsServerAdd),
            ),
            SizedBox(height: tokens.spacing.x3),
            Text(
              l10n.settingsServerEmptyHint,
              style: tokens.typography.mono11.tinted(tokens.colors.fg2),
            ),
          ],
        ),
      ),
    );
  }
}
