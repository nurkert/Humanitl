/// Pulling the image a start refused (HUM-439).
///
/// `SANDBOX_043` says that the image the sandbox starts from is not on this
/// machine with the digest it was chosen by. No start ever pulls; the daemon's
/// fix is the command that does. On the card a primary button above that
/// command does the same through `Sandbox(ImagePull)`, and only after it said
/// what pulling does: the image comes over the network of this machine
/// directly, not through the proxy, so no rule, no hold and no record apply
/// (`backlog/sprint-7.md`, HUM-439). The question stands in the card, next to
/// the reason the start failed, and the answer that pulls is never the
/// preselected one: the focus opens on "Cancel" (`docs/UX.md` 5.4).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// The beginning of the daemon's fix on `SANDBOX_043`, before the reference.
const String imagePullCommand = 'humanitl sandbox image pull ';

/// The reference a `SANDBOX_043` asks to pull, or null for any other
/// diagnostic.
///
/// The daemon names it in its fix (`pull_command_text` of the adapter);
/// a `SANDBOX_043` whose fix is something else (an image that cannot be the
/// root, `ChangeSetting`) gets no pull button, because pulling would not help.
String? imagePullReference(Diagnostic diagnostic) {
  if (diagnostic.code != DiagnosticCodes.sandboxImage) {
    return null;
  }
  return switch (diagnostic.fix) {
    FixActionCopyCommand(:final String command)
        when command.startsWith(imagePullCommand) =>
      command.substring(imagePullCommand.length).trim(),
    _ => null,
  };
}

enum _Stage { idle, asking, pulling, pulled, failed }

/// The pull button of a `SANDBOX_043` card, with its question and its answer.
class ImagePullAction extends ConsumerStatefulWidget {
  /// Creates the action for [reference], `name@sha256:<hex>`.
  const ImagePullAction({required this.reference, super.key});

  /// What is pulled.
  final String reference;

  @override
  ConsumerState<ImagePullAction> createState() => _ImagePullActionState();
}

class _ImagePullActionState extends ConsumerState<ImagePullAction> {
  _Stage _stage = _Stage.idle;

  /// The question, scrolled into view when it opens: the card of a
  /// diagnostic is capped in height, and the question below the reason would
  /// otherwise open out of sight.
  final GlobalKey _question = GlobalKey();

  void _ask() {
    setState(() => _stage = _Stage.asking);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final BuildContext? question = _question.currentContext;
      if (question != null && question.mounted) {
        unawaited(
          Scrollable.ensureVisible(
            question,
            alignmentPolicy: ScrollPositionAlignmentPolicy.keepVisibleAtEnd,
          ),
        );
      }
    });
  }

  /// The digest after a pull, the daemon's sentence after a failure.
  String _answer = '';

  Future<void> _pull() async {
    setState(() => _stage = _Stage.pulling);
    try {
      final PulledImage image = await ref
          .read(daemonClientProvider)
          .pullImage(reference: widget.reference);
      if (!mounted) {
        return;
      }
      setState(() {
        _stage = _Stage.pulled;
        _answer = image.digest;
      });
    } on DaemonException catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _stage = _Stage.failed;
        _answer = error.diagnostic.why;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TextStyle body = tokens.typography.ui13.tinted(tokens.colors.fg1);
    final Widget pullButton = HButton(
      key: const Key('sandbox-image-pull'),
      variant: HButtonVariant.primary,
      onPressed: _ask,
      child: Text(l10n.sandboxImagePull),
    );
    return switch (_stage) {
      _Stage.idle => pullButton,
      _Stage.asking => Column(
        key: _question,
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            l10n.sandboxImagePullNetwork(widget.reference),
            key: const Key('sandbox-image-pull-network'),
            style: body,
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x2,
            children: <Widget>[
              HButton(
                key: const Key('sandbox-image-pull-cancel'),
                variant: HButtonVariant.secondary,
                autofocus: true,
                onPressed: () => setState(() => _stage = _Stage.idle),
                child: Text(l10n.sandboxImagePullCancel),
              ),
              HButton(
                key: const Key('sandbox-image-pull-confirm'),
                variant: HButtonVariant.primary,
                onPressed: () => unawaited(_pull()),
                child: Text(l10n.sandboxImagePullConfirm),
              ),
            ],
          ),
        ],
      ),
      _Stage.pulling => Text(
        l10n.sandboxImagePulling,
        key: const Key('sandbox-image-pulling'),
        style: body,
      ),
      _Stage.pulled => Text(
        l10n.sandboxImagePulled(_answer),
        key: const Key('sandbox-image-pulled'),
        style: body,
      ),
      _Stage.failed => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            l10n.sandboxImagePullFailed(_answer),
            key: const Key('sandbox-image-pull-failed'),
            style: body,
          ),
          SizedBox(height: tokens.spacing.x2),
          pullButton,
        ],
      ),
    };
  }
}
