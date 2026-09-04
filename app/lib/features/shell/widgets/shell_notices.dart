/// The notice slot of the shell: the return banner and the one diagnostic the
/// desktop side raises (HUM-034).
///
/// One place, always the same place, directly under the header. Both are rare
/// and both are dismissed by the person, so nothing here queues up behind
/// anything else (`docs/UX.md` 4.9 and `backlog/CONVENTIONS.md` 4.13,
/// predictability).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../l10n/l10n.dart';
import '../../intercept/providers/flows.dart';
import '../../tray/attention_text.dart';
import '../../tray/providers/attention.dart';
import '../../tray/providers/notice.dart';
import '../../tray/widgets/attention_notice.dart';
import '../../tray/widgets/return_banner.dart';
import '../providers/navigation.dart';
import '../section.dart';

/// The slot. Takes no height while there is nothing to say.
class ShellNotices extends ConsumerWidget {
  /// Creates the slot.
  const ShellNotices({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final Diagnostic? notice = ref.watch(attentionNoticeProvider);
    final ReturnNotice? banner = ref.watch(
      attentionProvider.select((AttentionState state) => state.banner),
    );
    if (notice == null && banner == null) {
      return const SizedBox.shrink();
    }
    final AppLocalizations l10n = context.l10n;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        if (notice != null)
          AttentionNoticeCard(
            diagnostic: notice,
            onDismiss: ref.read(attentionNoticeProvider.notifier).dismiss,
          ),
        if (banner != null)
          ReturnBanner(
            sentence: waitedSentence(l10n, banner.waited),
            onJump: () => _jump(ref, banner.flowId),
            onDismiss: ref.read(attentionProvider.notifier).dismissBanner,
          ),
      ],
    );
  }

  /// Leads to the request that has waited longest, and to no other.
  static void _jump(WidgetRef ref, FlowId id) {
    ref.read(navigationProvider.notifier).go(Section.intercept);
    if (ref.read(flowsProvider).containsKey(id)) {
      ref.read(selectedFlowIdProvider.notifier).select(id);
    }
    ref.read(attentionProvider.notifier).dismissBanner();
  }
}
