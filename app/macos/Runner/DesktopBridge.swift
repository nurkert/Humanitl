import Cocoa
import FlutterMacOS
import UserNotifications

// What Linux gets over D-Bus (HUM-034), on macOS (HUM-373): the menu bar item,
// the Dock badge and the one notification. This file only draws and reports;
// every word, the count and the rules about when something is shown come from
// Dart (`macos_tray.dart`, `macos_notifications.dart`).
final class DesktopBridge: NSObject, UNUserNotificationCenterDelegate {
  static let shared = DesktopBridge()

  // The identifier of the one notification. Adding under the same identifier
  // replaces what stands, so nothing stacks.
  private static let notificationId = "dev.humanitl.held"

  // The longer edge of the menu bar icon in points. The pixmaps Dart sends are
  // 18 and 36 pixels, for one and two pixels per point.
  private static let iconPoints: CGFloat = 18

  private var trayChannel: FlutterMethodChannel?
  private var notificationChannel: FlutterMethodChannel?
  private var statusItem: NSStatusItem?
  private let countEntry = NSMenuItem(title: "", action: nil, keyEquivalent: "")
  private let showEntry = NSMenuItem(title: "", action: nil, keyEquivalent: "")
  private let quitEntry = NSMenuItem(title: "", action: nil, keyEquivalent: "")

  // Button sets are registered per set of labels: a category is fixed once it
  // is registered, and the set with `Allow` differs from the one without.
  private var categories: [String: UNNotificationCategory] = [:]

  // What waits for a category whose registration has not finished. Only what
  // the system has confirmed can be delivered with its buttons.
  private var registering: [String: [() -> Void]] = [:]

  // Counts posts and withdrawals. Registering a category is asynchronous, so a
  // withdrawal can overtake the post it is meant to take back.
  private var notificationEpoch = 0

  // A press that launched the app arrives before Dart listens. It waits here
  // until `start`, because a button must not end in silence.
  private var dartListens = false
  private var pendingAnswers: [[String: String]] = []

  // Closing the window hides everything of this app, and App Nap then throttles
  // its timers. The heartbeat and the bundling window of the notification have
  // to keep running while the agent waits for an answer.
  private var activity: NSObjectProtocol?

  // Called from `awakeFromNib`, so before the system can hand over a response:
  // the delegate has to be set before the app finishes launching.
  func install(messenger: FlutterBinaryMessenger) {
    let tray = FlutterMethodChannel(name: "humanitl/tray", binaryMessenger: messenger)
    let notifications = FlutterMethodChannel(
      name: "humanitl/notifications", binaryMessenger: messenger)
    tray.setMethodCallHandler { [weak self] call, result in
      self?.handleTray(call, result: result)
    }
    notifications.setMethodCallHandler { [weak self] call, result in
      self?.handleNotification(call, result: result)
    }
    trayChannel = tray
    notificationChannel = notifications
    UNUserNotificationCenter.current().delegate = self
    activity = ProcessInfo.processInfo.beginActivity(
      options: .userInitiatedAllowingIdleSystemSleep,
      reason: "Held requests are announced while no window is open")
    NotificationCenter.default.addObserver(
      self, selector: #selector(willTerminate),
      name: NSApplication.willTerminateNotification, object: nil)
  }

  // MARK: Tray

  private func handleTray(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "start":
      start(result: result)
    case "show":
      if let face = call.arguments as? [String: Any] {
        show(face)
      }
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  // Answers null when notifications may be shown, a sentence when the person
  // refused them, and an error when the system could not be asked at all.
  private func start(result: @escaping FlutterResult) {
    installStatusItem()
    let center = UNUserNotificationCenter.current()
    // What the last run left standing names requests of that run.
    center.removeAllDeliveredNotifications()
    center.requestAuthorization(options: [.alert]) { granted, error in
      DispatchQueue.main.async {
        self.dartListens = true
        let answers = self.pendingAnswers
        self.pendingAnswers = []
        answers.forEach { self.notificationChannel?.invokeMethod("action", arguments: $0) }
        if let error = error {
          result(
            FlutterError(
              code: "notifications", message: error.localizedDescription, details: nil))
        } else {
          result(granted ? nil : "denied in System Settings")
        }
      }
    }
  }

  private func installStatusItem() {
    if statusItem != nil {
      return
    }
    let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    let menu = NSMenu()
    menu.autoenablesItems = false
    showEntry.target = self
    showEntry.action = #selector(showPressed)
    quitEntry.target = self
    quitEntry.action = #selector(quitPressed)
    // Informative and nothing else, as on Linux (`docs/UX.md` 8).
    countEntry.isEnabled = false
    menu.addItem(showEntry)
    menu.addItem(.separator())
    menu.addItem(countEntry)
    menu.addItem(quitEntry)
    item.menu = menu
    statusItem = item
  }

  private func show(_ face: [String: Any]) {
    installStatusItem()
    let label = face["title"] as? String ?? ""
    countEntry.title = label
    showEntry.title = face["menuShow"] as? String ?? ""
    quitEntry.title = face["menuQuit"] as? String ?? ""
    if let button = statusItem?.button {
      button.image = DesktopBridge.image(from: face["pixmaps"] as? [[String: Any]] ?? [])
      button.toolTip = face["tooltip"] as? String
      button.setAccessibilityLabel(label)
    }
    let badge = face["badge"] as? String ?? ""
    NSApp.dockTile.badgeLabel = badge.isEmpty ? nil : badge
  }

  // One image with one representation per pixmap. The bytes are ARGB and not
  // premultiplied (`TrayPixmap`).
  static func image(from pixmaps: [[String: Any]]) -> NSImage? {
    let size = NSSize(width: iconPoints, height: iconPoints)
    let image = NSImage(size: size)
    for pixmap in pixmaps {
      guard let width = pixmap["width"] as? Int, let height = pixmap["height"] as? Int,
        let argb = (pixmap["argb"] as? FlutterStandardTypedData)?.data,
        argb.count == width * height * 4,
        let rep = NSBitmapImageRep(
          bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8,
          samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
          bitmapFormat: [.alphaFirst, .alphaNonpremultiplied], bytesPerRow: width * 4,
          bitsPerPixel: 32),
        let bytes = rep.bitmapData
      else {
        continue
      }
      argb.copyBytes(to: bytes, count: argb.count)
      rep.size = size
      image.addRepresentation(rep)
    }
    return image.representations.isEmpty ? nil : image
  }

  @objc private func showPressed() {
    trayChannel?.invokeMethod("command", arguments: "show")
  }

  @objc private func quitPressed() {
    trayChannel?.invokeMethod("command", arguments: "quit")
  }

  // MARK: Notification

  private func handleNotification(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "post":
      if let notification = call.arguments as? [String: Any] {
        post(notification)
      }
      result(nil)
    case "withdraw":
      withdraw()
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  private func post(_ notification: [String: Any]) {
    let content = UNMutableNotificationContent()
    content.title = notification["title"] as? String ?? ""
    content.body = notification["body"] as? String ?? ""
    // The category belongs to a set of buttons, so the request travels here.
    // A press on a message that outlived the queue still names its own request.
    content.userInfo = ["flowId": notification["flowId"] as? String ?? ""]
    let actions = notification["actions"] as? [[String: String]] ?? []
    let category = actions.flatMap { [$0["id"] ?? "", $0["label"] ?? ""] }
      .joined(separator: "\n")
    content.categoryIdentifier = category

    notificationEpoch += 1
    let epoch = notificationEpoch
    let deliver = { [weak self] in
      guard let self = self, self.notificationEpoch == epoch else {
        return
      }
      UNUserNotificationCenter.current().add(
        UNNotificationRequest(
          identifier: DesktopBridge.notificationId, content: content, trigger: nil))
    }
    if registering[category] != nil {
      registering[category]?.append(deliver)
      return
    }
    if categories[category] != nil {
      deliver()
      return
    }
    registering[category] = [deliver]
    categories[category] = UNNotificationCategory(
      identifier: category,
      actions: actions.map(DesktopBridge.action),
      intentIdentifiers: [],
      options: [])
    let center = UNUserNotificationCenter.current()
    center.setNotificationCategories(Set(categories.values))
    // The read waits for the write. A notification added right after an
    // unfinished registration comes without its buttons.
    center.getNotificationCategories { _ in
      DispatchQueue.main.async {
        self.registering.removeValue(forKey: category)?.forEach { $0() }
      }
    }
  }

  // `Allow` and `Block` answer from the message and leave the window where it
  // is (HUM-034 acceptance); only `Show` brings the app forward.
  private static func action(_ action: [String: String]) -> UNNotificationAction {
    let id = action["id"] ?? ""
    return UNNotificationAction(
      identifier: id, title: action["label"] ?? "",
      options: id == "show" ? [.foreground] : [])
  }

  private func withdraw() {
    notificationEpoch += 1
    let center = UNUserNotificationCenter.current()
    center.removePendingNotificationRequests(withIdentifiers: [DesktopBridge.notificationId])
    center.removeDeliveredNotifications(withIdentifiers: [DesktopBridge.notificationId])
  }

  @objc private func willTerminate() {
    withdraw()
  }

  // Without this the system swallows the message while the app is active, and
  // the app stays active with its window closed. Whether a message is due at
  // all has been decided in Dart.
  func userNotificationCenter(
    _ center: UNUserNotificationCenter,
    willPresent notification: UNNotification,
    withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
  ) {
    completionHandler([.banner, .list])
  }

  func userNotificationCenter(
    _ center: UNUserNotificationCenter,
    didReceive response: UNNotificationResponse,
    withCompletionHandler completionHandler: @escaping () -> Void
  ) {
    let flowId = response.notification.request.content.userInfo["flowId"] as? String ?? ""
    // A click on the message itself: the same as its `Show` button.
    let id =
      response.actionIdentifier == UNNotificationDefaultActionIdentifier
      ? "show" : response.actionIdentifier
    let answer = ["action": id, "flowId": flowId]
    DispatchQueue.main.async {
      if self.dartListens {
        self.notificationChannel?.invokeMethod("action", arguments: answer)
      } else {
        self.pendingAnswers.append(answer)
      }
    }
    completionHandler()
  }
}
