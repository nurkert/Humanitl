/// What shows while `GetInfo` is in flight: the wordmark and one line.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'header_bar.dart';

/// The splash.
class Splash extends StatelessWidget {
  /// Creates the splash.
  const Splash({super.key});

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Wordmark(
            markSize: 24,
            style: tokens.typography.ui16.semibold.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x3),
          Text(
            context.l10n.setupConnecting,
            style: tokens.typography.ui13.tinted(tokens.colors.fg1),
          ),
        ],
      ),
    );
  }
}
