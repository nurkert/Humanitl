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
///
/// A running pull can be cancelled (HUM-457): the pulling stage shows a
/// secondary "Cancel" button that sends `Sandbox(CancelImagePull)` with the
/// id this card chose for its pull, never "whatever pull is running". The
/// daemon kills the process group of the pull and answers when the pull has
/// returned; the card then returns to its pull button with a short note,
/// which names processes that still ran after the kill. A cancel that found
/// no pull while this one goes on is sent again (it overtook the pull).
/// A daemon before minor 37 cannot cancel, so the card offers no button there.
library;

import 'dart:async';
import 'dart:math' show Random;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/connection.dart';
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

/// The first minor of the contract that can cancel a pull (HUM-457).
const int imagePullCancelMinor = 37;

/// A random UUID (version 4) as the id of one pull: the card chooses it, so
/// it can cancel before the daemon answered.
String newImagePullId() {
  final Random random = Random.secure();
  final List<int> bytes = List<int>.generate(16, (_) => random.nextInt(256));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  final String hex = bytes
      .map((int byte) => byte.toRadixString(16).padLeft(2, '0'))
      .join();
  return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
      '${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
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

  /// The id of the running pull, empty when it cannot be cancelled.
  String _pullId = '';

  /// The outcome of the cancel, while one was sent for this pull: the
  /// daemon's answer, or null when the cancel itself failed and its outcome
  /// is unknown.
  Future<ImagePullCancelAnswer?>? _cancelAnswer;

  /// Whether the running pull has returned, with an image or an error.
  bool _pullReturned = false;

  /// Whether the last pull ended because it was cancelled.
  bool _cancelled = false;

  /// With [_cancelled]: how many processes of the pull still ran after the
  /// kill, as the daemon counted them.
  int _stillRunning = 0;

  /// How often a cancel that found no pull is sent again, and how long it
  /// waits in between (HUM-457). The cancel can overtake the registration of
  /// its pull in the daemon; then the daemon answers "not running" while the
  /// pull goes on. The retries end as soon as the pull returns.
  static const int _cancelAttempts = 8;
  static const Duration _cancelRetry = Duration(milliseconds: 250);

  /// Pulls; with [cancellable] under an id of its own, so that the pulling
  /// stage can offer to cancel it.
  Future<void> _pull({required bool cancellable}) async {
    final String pullId = cancellable ? newImagePullId() : '';
    setState(() {
      _stage = _Stage.pulling;
      _pullId = pullId;
      _cancelAnswer = null;
      _pullReturned = false;
      _cancelled = false;
      _stillRunning = 0;
    });
    try {
      final PulledImage image = await ref
          .read(daemonClientProvider)
          .pullImage(reference: widget.reference, pullId: pullId);
      _pullReturned = true;
      if (!mounted) {
        return;
      }
      setState(() {
        _stage = _Stage.pulled;
        _answer = image.digest;
      });
    } on DaemonException catch (error) {
      _pullReturned = true;
      // Only the daemon's answer to the cancel says whether this failure is
      // the cancel: a pull that failed on its own just before the cancel
      // arrived is a failure, and the cancel then answers "not cancelled".
      // When the outcome of the cancel is unknown (its call failed, or the
      // daemon warned that the pull had not ended in time), the card shows
      // the pull's own sentence, which says so itself when the pull was
      // cancelled; the code alone cannot tell a cancel from a registry error.
      final Future<ImagePullCancelAnswer?>? sent = _cancelAnswer;
      final ImagePullCancelAnswer? answer = sent == null ? null : await sent;
      final bool cancelled = answer?.cancelled ?? false;
      if (!mounted) {
        return;
      }
      setState(() {
        if (cancelled) {
          _stage = _Stage.idle;
          _cancelled = true;
          _stillRunning = answer?.stillRunning ?? 0;
        } else {
          _stage = _Stage.failed;
          _answer = error.diagnostic.why;
        }
      });
    }
  }

  /// Whether the pull with [pullId] is still the one running here.
  bool _stillPulling(String pullId) =>
      mounted && !_pullReturned && _pullId == pullId;

  /// One cancel call; null when the call failed and its outcome is unknown.
  static Future<ImagePullCancelAnswer?> _askToCancel(
    DaemonClient client,
    String pullId,
  ) async {
    try {
      return await client.cancelImagePull(pullId);
    } on DaemonException {
      return null;
    }
  }

  /// Cancels the running pull by its id (HUM-457).
  ///
  /// An answer "not running" while the pull goes on means the cancel
  /// overtook the pull's registration; it is sent again, a few times, until
  /// it finds the pull or the pull returns. If it never finds it, the button
  /// is usable again. A cancel whose call fails leaves its outcome unknown;
  /// the pull's own answer decides then.
  Future<void> _cancel() async {
    final String pullId = _pullId;
    final DaemonClient client = ref.read(daemonClientProvider);
    final Completer<ImagePullCancelAnswer?> outcome =
        Completer<ImagePullCancelAnswer?>();
    setState(() {
      _cancelAnswer = outcome.future;
    });
    ImagePullCancelAnswer? answer;
    for (int attempt = 1; ; attempt += 1) {
      answer = await _askToCancel(client, pullId);
      if (answer == null ||
          answer.cancelled ||
          attempt >= _cancelAttempts ||
          !_stillPulling(pullId)) {
        break;
      }
      await Future<void>.delayed(_cancelRetry);
      if (!_stillPulling(pullId)) {
        break;
      }
    }
    outcome.complete(answer);
    // Not found, or an unknown outcome while the pull goes on: the button is
    // usable again, so that the person can try once more.
    if ((answer == null || !answer.cancelled) && _stillPulling(pullId)) {
      setState(() {
        _cancelAnswer = null;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TextStyle body = tokens.typography.ui13.tinted(tokens.colors.fg1);
    // Only a daemon from minor 37 on can cancel a pull (HUM-457).
    final bool cancellable =
        (ref.watch(daemonInfoProvider).value?.protoMinor ?? 0) >=
        imagePullCancelMinor;
    final Widget pullButton = HButton(
      key: const Key('sandbox-image-pull'),
      variant: HButtonVariant.primary,
      onPressed: _ask,
      child: Text(l10n.sandboxImagePull),
    );
    return switch (_stage) {
      _Stage.idle when _cancelled => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            _stillRunning == 0
                ? l10n.sandboxImagePullCancelled
                : l10n.sandboxImagePullCancelledRemaining(_stillRunning),
            key: const Key('sandbox-image-pull-cancelled'),
            style: body,
          ),
          SizedBox(height: tokens.spacing.x2),
          pullButton,
        ],
      ),
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
                onPressed: () => unawaited(_pull(cancellable: cancellable)),
                child: Text(l10n.sandboxImagePullConfirm),
              ),
            ],
          ),
        ],
      ),
      _Stage.pulling when _pullId.isNotEmpty => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            l10n.sandboxImagePulling,
            key: const Key('sandbox-image-pulling'),
            style: body,
          ),
          SizedBox(height: tokens.spacing.x2),
          HButton(
            key: const Key('sandbox-image-pull-cancel-running'),
            variant: HButtonVariant.secondary,
            // Once sent, the cancel waits for the daemon's answer; a second
            // press would ask the same again.
            onPressed: _cancelAnswer == null
                ? () => unawaited(_cancel())
                : null,
            child: Text(l10n.sandboxImagePullCancelRunning),
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
