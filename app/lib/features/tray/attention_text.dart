/// Turns the attention state into the words the desktop shows (HUM-034).
///
/// Separate from the state machine because it needs a locale, and separate
/// from the widget because it is worth testing on its own: these are the
/// sentences a person reads while the window is not in front, and there is no
/// second chance to say them differently.
///
/// Every duration becomes a word. A notification is a still image in a
/// foreign window manager and a tray tooltip is a hover away; a `mm:ss` in
/// either is wrong one second after it was drawn (`docs/UX.md` 4.9).
library;

import '../../l10n/l10n.dart';
import 'desktop_ports.dart';
import 'providers/attention.dart';

/// Above this many minutes the remaining time is said in hours.
///
/// An hour and a half is the point where "about 90 minutes" stops being
/// easier to grasp than "about two hours".
const int _hoursAbove = 90;

/// The remaining hold budget as a word.
String remainingPhrase(AppLocalizations l10n, Duration remaining) {
  if (remaining.inSeconds < 60) {
    return l10n.trayRemainingUnderMinute;
  }
  final int minutes = (remaining.inSeconds / 60).round();
  if (minutes < _hoursAbove) {
    return l10n.trayRemainingMinutes(minutes);
  }
  return l10n.trayRemainingHours((minutes / 60).round());
}

/// How long the agent has been waiting, as a sentence.
///
/// Rounded down, not to the nearest: the banner claims a wait that has
/// certainly happened, never one that is half a minute away.
String waitedSentence(AppLocalizations l10n, Duration waited) {
  final int minutes = waited.inMinutes;
  if (minutes < _hoursAbove) {
    return l10n.trayReturnMinutes(minutes);
  }
  return l10n.trayReturnHours(minutes ~/ 60);
}

/// What the tray shows for [state].
TrayFace trayFace(AppLocalizations l10n, AttentionState state) {
  final (String title, String detail) = switch (state.tray) {
    TrayIconState.idle => (l10n.trayTooltipIdle, ''),
    TrayIconState.held => (l10n.trayTooltipHeld(state.held), ''),
    TrayIconState.alert => (
      state.held == 0 ? l10n.trayTooltipIdle : l10n.trayTooltipHeld(state.held),
      l10n.trayTooltipTimedOut(state.timedOutAway),
    ),
    TrayIconState.offline => (
      l10n.trayTooltipOffline,
      l10n.trayTooltipOfflineDetail,
    ),
  };
  return TrayFace(
    state: state.tray,
    count: state.held,
    title: title,
    detail: detail,
    menuShow: l10n.trayMenuShow,
    menuQuit: l10n.trayMenuQuit,
  );
}

/// The window title for [state].
String windowTitle(AppLocalizations l10n, AttentionState state) =>
    state.held == 0
    ? l10n.appTitle
    : l10n.trayWindowTitle(state.held, l10n.appTitle);

/// The notification for [notice].
///
/// The host is the summary and everything else is the body, in two lines:
/// what the request is, and what is true about it right now.
DesktopNotification notificationFor(AppLocalizations l10n, HeldNotice notice) {
  final StringBuffer second = StringBuffer(
    remainingPhrase(l10n, notice.remaining),
  );
  if (notice.others > 0) {
    second
      ..write(l10n.traySeparator)
      ..write(l10n.trayNotifyMore(notice.others));
  }
  if (notice.findings > 0) {
    second
      ..write(l10n.traySeparator)
      ..write(l10n.trayNotifyFindings(notice.findings));
  }
  return DesktopNotification(
    flowId: notice.flowId,
    summary: notice.host,
    body:
        '${l10n.trayNotifyDetail(notice.method, notice.path)}\n'
        '$second',
    actions: <NotificationAction>[
      // No `Allow` while a finding is unresolved: a request that carries a
      // secret is sent after reading a sentence and holding a control, and a
      // notification button is neither (`docs/UX.md` 4.7). Blocking stays,
      // because the agent may retry a block.
      if (notice.mayAllow)
        NotificationAction(
          kind: NotificationActionKind.allow,
          label: l10n.trayActionAllow,
        ),
      NotificationAction(
        kind: NotificationActionKind.block,
        label: l10n.trayActionBlock,
      ),
      NotificationAction(
        kind: NotificationActionKind.show,
        label: l10n.trayActionShow,
      ),
    ],
  );
}
