/// The notice slot of the shell: the return banner and the one diagnostic the
/// desktop side raises (HUM-034).
///
/// One place, always the same place: the top right of the window, below the
/// header row of the panes. Since HUM-302 the notices float there over the
/// sections (`HNoticeLayer` in the shell) and no longer push them down, so a
/// running terminal keeps its size when a notice comes or goes. Both are rare
/// and both are dismissed by the person, so nothing here queues up behind
/// anything else (`docs/UX.md` 4.9 and `backlog/CONVENTIONS.md` 4.13,
/// predictability).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/h_notice_layer.dart';
import '../../../l10n/l10n.dart';
import '../../intercept/providers/flows.dart';
import '../../tray/attention_text.dart';
import '../../tray/providers/attention.dart';
import '../../tray/providers/notice.dart';
import '../../tray/widgets/attention_notice.dart';
import '../../tray/widgets/return_banner.dart';
import '../providers/navigation.dart';
import '../providers/notice_dismissal.dart';
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
    final AppLocalizations l10n = context.l10n;
    return HNoticeColumn(
      children: <Widget>[
        if (notice != null)
          AttentionNoticeBanner(
            diagnostic: notice,
            onDismiss: () => _dismissNotice(ref, notice),
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

  /// Closes the notice. A fact about the desktop, raised once, stays closed
  /// after a restart as well (HUM-276); a notice about one event does not.
  static void _dismissNotice(WidgetRef ref, Diagnostic notice) {
    final AttentionNotice notices = ref.read(attentionNoticeProvider.notifier);
    if (notices.lasting) {
      ref.read(dismissedNoticesProvider.notifier).remember(notice.code);
    }
    notices.dismiss();
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
