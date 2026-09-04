/// The three seams between this program and the desktop it runs on: the
/// window, the notification and the tray (HUM-034).
///
/// Everything above these ports is ordinary Dart and runs in a widget test.
/// Everything below them talks to a session bus that no test has, so the
/// default is deliberately inert: an app that was not wired up in `main`
/// shows no tray, sends no notification and touches no window title. The
/// Linux implementations live next door in `platform/`.
library;

import 'package:flutter/foundation.dart' show immutable;

import '../../core/domain/domain.dart';

/// What the tray icon says at a glance (`docs/UX.md` 4.9).
enum TrayIconState {
  /// Nothing waits.
  idle,

  /// One or more requests wait for a decision; the count is drawn into the
  /// icon.
  held,

  /// A hold ran out and blocked a request since the window last had focus.
  alert,

  /// The daemon does not answer, so the number of held requests is unknown.
  ///
  /// Not in the table of `docs/UX.md` 4.9, which knows three states. A tray
  /// that keeps showing the last count it saw claims something it cannot
  /// know any more, and `backlog/CONVENTIONS.md` 4.13 forbids exactly that:
  /// what the daemon does not know stands as unknown.
  offline,
}

/// Everything the tray shows, already in the language of the person.
///
/// A value type: the adapter compares it with the face it drew last and stays
/// quiet when nothing changed, so a queue that grows and shrinks by one does
/// not repaint an icon that looks the same.
class TrayFace {
  /// Creates a face.
  const TrayFace({
    required this.state,
    required this.count,
    required this.title,
    required this.detail,
    required this.menuShow,
    required this.menuQuit,
  });

  /// Which of the four looks the icon takes.
  final TrayIconState state;

  /// How many requests are held. Zero in every state but [TrayIconState.held]
  /// and [TrayIconState.alert].
  final int count;

  /// The first tooltip line and the informational entry of the menu. It says
  /// what the number counts, because a bare digit in a tray is a riddle.
  final String title;

  /// The second tooltip line, or empty. Carries the timed-out count in
  /// [TrayIconState.alert] and the reason in [TrayIconState.offline].
  final String detail;

  /// Label of the menu entry that brings the window forward.
  final String menuShow;

  /// Label of the menu entry that ends the program.
  final String menuQuit;

  @override
  bool operator ==(Object other) =>
      other is TrayFace &&
      other.state == state &&
      other.count == count &&
      other.title == title &&
      other.detail == detail &&
      other.menuShow == menuShow &&
      other.menuQuit == menuQuit;

  @override
  int get hashCode =>
      Object.hash(state, count, title, detail, menuShow, menuQuit);
}

/// What a person asked of the tray.
enum TrayCommand {
  /// Bring the window forward.
  show,

  /// End the program.
  quit,
}

/// The three buttons a notification can carry.
enum NotificationActionKind {
  /// Send the named request on.
  allow,

  /// Refuse the named request.
  block,

  /// Bring the window forward on the named request.
  show,
}

/// What a person pressed, and on which request.
///
/// The request travels with the answer because a notification can outlive the
/// message it belongs to: a server that ignores `replaces_id` leaves the old
/// popup standing, and a press on it has to name the request that popup was
/// about, not whatever the program is showing now. Without the request in the
/// answer such a press is dropped in silence, which is the one thing a button
/// must never do (HUM-034).
@immutable
class NotificationAnswer {
  /// Creates an answer.
  const NotificationAnswer({required this.kind, required this.flowId});

  /// Which button it was.
  final NotificationActionKind kind;

  /// The request the message named.
  final FlowId flowId;

  @override
  bool operator ==(Object other) =>
      other is NotificationAnswer &&
      other.kind == kind &&
      other.flowId == flowId;

  @override
  int get hashCode => Object.hash(kind, flowId);
}

/// One button of a notification: what it does and what it is called.
class NotificationAction {
  /// Creates an action.
  const NotificationAction({required this.kind, required this.label});

  /// What pressing it does.
  final NotificationActionKind kind;

  /// The label, already translated.
  final String label;

  @override
  bool operator ==(Object other) =>
      other is NotificationAction && other.kind == kind && other.label == label;

  @override
  int get hashCode => Object.hash(kind, label);
}

/// One desktop notification, already in the language of the person.
///
/// The host is the summary and nothing else is, because a notification is a
/// still image in a foreign window manager and the summary is the only line
/// it is guaranteed to show large (`docs/UX.md` 4.9).
class DesktopNotification {
  /// Creates a notification.
  const DesktopNotification({
    required this.flowId,
    required this.summary,
    required this.body,
    required this.actions,
  });

  /// The request this message is about; it goes into every action key.
  final FlowId flowId;

  /// The one important thing: the host.
  final String summary;

  /// Method, path, the remaining time as a word, and how many more wait.
  final String body;

  /// The buttons, in the order they should appear.
  final List<NotificationAction> actions;
}

/// The window this program lives in.
abstract interface class WindowPort {
  /// True whenever the window comes to the front, false when it leaves it.
  Stream<bool> get focus;

  /// Sets the window title; the count of held requests rides in it.
  Future<void> setTitle(String title);

  /// Brings the window forward and gives it the keyboard.
  Future<void> reveal();

  /// Ends the program the way the tray menu asks for it.
  Future<void> quit();

  /// Releases whatever the port holds.
  Future<void> dispose();
}

/// The one notification this program ever shows.
///
/// One, not one per request: [post] replaces what stands, [withdraw] takes it
/// away. Nothing here stacks.
abstract interface class NotificationPort {
  /// Shows [notification], replacing the one that stands.
  Future<void> post(DesktopNotification notification);

  /// Takes the notification off the screen.
  Future<void> withdraw();

  /// The buttons a person pressed, each with the request it was about.
  Stream<NotificationAnswer> get actions;

  /// Releases whatever the port holds.
  Future<void> dispose();
}

/// The tray icon.
abstract interface class TrayPort {
  /// Registers the icon with the desktop.
  ///
  /// Returns null when the tray works and a diagnostic -- once -- when this
  /// desktop has no tray to register with. The caller shows it and never
  /// asks again: a program that keeps complaining about a missing tray is
  /// the nagging this feature exists to avoid.
  Future<Diagnostic?> start();

  /// Draws [face].
  Future<void> show(TrayFace face);

  /// What a person asked of the tray.
  Stream<TrayCommand> get commands;

  /// Releases whatever the port holds.
  Future<void> dispose();
}

/// The three ports together, as one injectable value.
class DesktopPorts {
  /// Creates a bundle.
  const DesktopPorts({
    required this.window,
    required this.notifications,
    required this.tray,
  });

  /// A bundle that does nothing at all: no window, no notification, no tray.
  ///
  /// The default everywhere but in `main`. A widget test that never asked for
  /// a desktop must not find one, and a headless run must not fail because a
  /// session bus is missing.
  factory DesktopPorts.inert() => DesktopPorts(
    window: const _InertWindow(),
    notifications: _InertNotifications(),
    tray: _InertTray(),
  );

  /// The window.
  final WindowPort window;

  /// The notification.
  final NotificationPort notifications;

  /// The tray.
  final TrayPort tray;

  /// Releases all three.
  Future<void> dispose() async {
    await window.dispose();
    await notifications.dispose();
    await tray.dispose();
  }
}

class _InertWindow implements WindowPort {
  const _InertWindow();

  @override
  Stream<bool> get focus => const Stream<bool>.empty();

  @override
  Future<void> setTitle(String title) async {}

  @override
  Future<void> reveal() async {}

  @override
  Future<void> quit() async {}

  @override
  Future<void> dispose() async {}
}

class _InertNotifications implements NotificationPort {
  @override
  Stream<NotificationAnswer> get actions =>
      const Stream<NotificationAnswer>.empty();

  @override
  Future<void> post(DesktopNotification notification) async {}

  @override
  Future<void> withdraw() async {}

  @override
  Future<void> dispose() async {}
}

class _InertTray implements TrayPort {
  @override
  Stream<TrayCommand> get commands => const Stream<TrayCommand>.empty();

  /// No diagnostic: nobody asked this build for a tray, so nothing is missing.
  @override
  Future<Diagnostic?> start() async => null;

  @override
  Future<void> show(TrayFace face) async {}

  @override
  Future<void> dispose() async {}
}
