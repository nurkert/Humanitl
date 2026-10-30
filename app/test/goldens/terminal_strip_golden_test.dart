// Golden des schmalen Titelstreifens, zu dem das schwebende Terminal ohne
// freie Ecke schrumpft (HUM-315): einmal mit dem Profil der Sandbox und
// einer weiteren Shell, einmal mit dem Rückfall „Terminal" und dem Agenten,
// je dunkel und hell, Englisch und Deutsch. Die Größe ist das Token
// `HSize.terminalStrip`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/widgets/floating_terminal_strip.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'support/golden_variants.dart';

void main() {
  goldenVariants(
    'terminal_strip',
    constraints: BoxConstraints.tightFor(
      width: HSize.terminalStrip.width + 32,
      height: HSize.terminalStrip.height * 2 + 48,
    ),
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      background: variant.tokens.colors.bg0,
      padding: const EdgeInsets.all(16),
      child: Builder(
        builder: (BuildContext context) {
          final AppLocalizations l10n = context.l10n;
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              SizedBox.fromSize(
                size: HSize.terminalStrip,
                child: FloatingTerminalStrip(
                  title: l10n.shellTerminalStripTitle(
                    'opencode',
                    l10n.sandboxTerminalTabShell(1),
                  ),
                  onOpen: () {},
                ),
              ),
              const SizedBox(height: 16),
              SizedBox.fromSize(
                size: HSize.terminalStrip,
                child: FloatingTerminalStrip(
                  title: l10n.shellTerminalStripTitle(
                    l10n.shellTerminalStripFallback,
                    l10n.sandboxTerminalTabAgent,
                  ),
                  onOpen: () {},
                ),
              ),
            ],
          );
        },
      ),
    ),
  );
}
