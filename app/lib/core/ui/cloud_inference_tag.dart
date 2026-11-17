/// The notice on cloud inference (HUM-516): the tag at the top of the
/// session and the note where the agent or its endpoint is chosen.
///
/// Whether the agent's inference goes to a provider outside this machine is
/// the daemon's answer (`SandboxEvent.Status.cloud_inference`, ADR-018); the
/// widgets here only draw it. The colour is the outflow token of the design
/// system, an orange that is neither the yellow of a warning nor the red of
/// errors and blocks: the notice blocks nothing. It never stands as colour
/// alone (`docs/UX.md` 3.3, rule 12): the tag carries its words, the note
/// the triangle glyph.
library;

import 'package:flutter/widgets.dart';

import '../../l10n/l10n.dart';
import '../domain/domain.dart';
import 'hover_label.dart';
import 'ui.dart';

/// The orange tag "Cloud inference: data goes to HOST" in the session
/// header. Hovering shows the whole notice, a click opens it as a popover
/// beside the tag.
class CloudInferenceTag extends StatefulWidget {
  /// Shows the tag for [cloud].
  const CloudInferenceTag({required this.cloud, super.key});

  /// What the daemon said.
  final CloudInference cloud;

  @override
  State<CloudInferenceTag> createState() => _CloudInferenceTagState();
}

class _CloudInferenceTagState extends State<CloudInferenceTag> {
  final OverlayPortalController _portal = OverlayPortalController();
  final LayerLink _link = LayerLink();

  /// Width of the popover with the whole text.
  static const double _popoverWidth = 360;

  void _toggle() => setState(_portal.toggle);

  void _close() => setState(_portal.hide);

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String provider = widget.cloud.provider;
    final String full = l10n.cloudInferenceNotice(provider);
    return CompositedTransformTarget(
      link: _link,
      child: OverlayPortal(
        controller: _portal,
        overlayChildBuilder: (BuildContext context) => Positioned(
          width: _popoverWidth,
          child: CompositedTransformFollower(
            link: _link,
            targetAnchor: Alignment.bottomLeft,
            offset: Offset(0, tokens.spacing.x1),
            child: HPopover(
              key: const Key('cloud-inference-popover'),
              title: l10n.cloudInferenceTitle(provider),
              body: full,
              closeLabel: l10n.cloudInferenceClose,
              onClose: _close,
              arrow: HPopoverArrow.up,
              width: _popoverWidth,
            ),
          ),
        ),
        child: HoverLabel(
          label: full,
          side: HoverLabelSide.below,
          child: HBadge(
            key: const Key('cloud-inference-tag'),
            text: l10n.cloudInferenceTag(provider),
            color: tokens.colors.outflow,
            textColor: tokens.colors.outflowText,
            semanticsLabel: full,
            overflow: TextOverflow.ellipsis,
            onTap: _toggle,
          ),
        ),
      ),
    );
  }
}

/// The whole notice as a short paragraph with the warning glyph, for the
/// place where the agent or its endpoint is chosen (settings, setup). Shown
/// once there, while the choice leads to a provider outside this machine.
class CloudInferenceNote extends StatelessWidget {
  /// Shows the note for [cloud].
  const CloudInferenceNote({required this.cloud, super.key});

  /// What the daemon said.
  final CloudInference cloud;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Row(
      key: const Key('cloud-inference-note'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        HGlyphIcon(
          HGlyph.triangleAlert,
          size: HType.ui12.fontSize!,
          color: tokens.colors.outflow,
        ),
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          child: Text(
            l10n.cloudInferenceNotice(cloud.provider),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ),
      ],
    );
  }
}
