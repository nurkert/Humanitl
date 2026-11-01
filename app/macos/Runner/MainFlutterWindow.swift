import Cocoa
import FlutterMacOS

// Where the traffic lights stand, for the header bar of the app (HUM-383). In
// full screen the system hides them until the pointer reaches the top edge;
// Dart frees their room while they are hidden. The raw values are the words
// on the channel `humanitl/window` (`traffic_lights.dart`).
enum TrafficLightState: String {
  case windowed
  case fullScreenHidden
  case fullScreenRevealed
}

// The window of the app (HUM-325): title Humanitl, 1280 x 720 at the first
// start, and no second title bar. The Flutter header bar is the title bar, as
// on Linux (HUM-255); macOS keeps its traffic lights at the left end of it.
class MainFlutterWindow: NSWindow {
  // Where the traffic lights sit (HUM-342): centred on the header bar of the
  // app instead of the 28 pt title bar of the system, 16 pt from the left
  // edge, which leaves a 14 pt gap between the last button and the wordmark.
  // `trafficLightInset` in window_frame.dart leaves room up to 72 pt; the
  // buttons end at 70. The bar grows with the scale of the app (HUM-378), so
  // Dart reports its height; 40 pt is `HSize.headerBar` at 100 %.
  var headerBarHeight: CGFloat = 40 {
    didSet { placeTrafficLights() }
  }
  private static let systemTitleBarHeight: CGFloat = 28
  private static let firstButtonLeft: CGFloat = 16
  private static let buttonPitch: CGFloat = 20

  // What the header bar of the app has to leave free (HUM-383). Every change
  // goes to Dart once; `DesktopBridge` sends it over `humanitl/window`.
  private(set) var trafficLightState: TrafficLightState = .windowed {
    didSet {
      if trafficLightState != oldValue {
        DesktopBridge.shared.reportTrafficLights(trafficLightState)
      }
    }
  }
  // Between `willEnter` and `didEnter` and between `willExit` and `didExit`
  // the system animates the window, and the title bar is not yet where it
  // will stay. The state comes from the notification then, not from the
  // geometry.
  private var fullScreenTransition = false
  // Counts transitions, so a late check only judges the one it was set for.
  private var transitionEpoch = 0
  // How long a transition may take before its end is assumed. A failed
  // transition ends without a `did` notification; AppKit tells only the
  // window delegate (`windowDidFailToEnterFullScreen`), and that delegate
  // belongs to window_manager (`WindowManager.swift` sets it), so this window
  // cannot become it without taking it away.
  private static let transitionTimeout: TimeInterval = 2
  private var titleBarObservations: [NSKeyValueObservation] = []
  private var titleBarWindowObservers: [NSObjectProtocol] = []

  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    self.contentViewController = flutterViewController
    self.title = "Humanitl"
    self.titleVisibility = .hidden
    self.titlebarAppearsTransparent = true
    self.styleMask.insert(.fullSizeContentView)
    self.setContentSize(NSSize(width: 1280, height: 720))
    self.center()

    RegisterGeneratedPlugins(registry: flutterViewController)
    DesktopBridge.shared.install(messenger: flutterViewController.engine.binaryMessenger)

    super.awakeFromNib()

    // AppKit lays the buttons out again after every resize, on a change of
    // key window, screen or full screen; each time they go back to our place.
    let center = NotificationCenter.default
    for name in [
      NSWindow.didResizeNotification,
      NSWindow.didEndLiveResizeNotification,
      NSWindow.didBecomeKeyNotification,
      NSWindow.didResignKeyNotification,
      NSWindow.didExitFullScreenNotification,
      NSWindow.didChangeBackingPropertiesNotification,
      NSWindow.didChangeScreenNotification,
    ] {
      center.addObserver(self, selector: #selector(placeTrafficLights), name: name, object: self)
    }
    let fullScreen: [(NSNotification.Name, Selector)] = [
      (NSWindow.willEnterFullScreenNotification, #selector(willEnterFullScreen)),
      (NSWindow.didEnterFullScreenNotification, #selector(didEnterFullScreen)),
      (NSWindow.willExitFullScreenNotification, #selector(willExitFullScreen)),
      (NSWindow.didExitFullScreenNotification, #selector(didExitFullScreen)),
    ]
    for (name, selector) in fullScreen {
      center.addObserver(self, selector: selector, name: name, object: self)
    }
    placeTrafficLights()
    DispatchQueue.main.async { [weak self] in
      self?.placeTrafficLights()
      self?.resumeFullScreenWatch()
    }
  }

  // MARK: Full screen (HUM-383)

  // The buttons vanish with the zoom into full screen, so the room is free
  // from its start; Dart lets the content glide left alongside.
  @objc private func willEnterFullScreen() {
    beginTransition()
    trafficLightState = .fullScreenHidden
  }

  @objc private func didEnterFullScreen() {
    fullScreenTransition = false
    watchFullScreenTitleBar()
    updateReveal()
  }

  // The window comes back with the buttons at their place in the bar, so the
  // content makes room for them while the system zooms out.
  @objc private func willExitFullScreen() {
    unwatchFullScreenTitleBar()
    beginTransition()
    trafficLightState = .windowed
  }

  // Flags the transition and, if no `did` notification ends it in time,
  // derives the state from the style mask: a failed entry leaves the window
  // windowed, a failed exit leaves it in full screen.
  private func beginTransition() {
    fullScreenTransition = true
    transitionEpoch += 1
    let epoch = transitionEpoch
    DispatchQueue.main.asyncAfter(deadline: .now() + MainFlutterWindow.transitionTimeout) {
      [weak self] in
      guard let self = self, self.fullScreenTransition, self.transitionEpoch == epoch else {
        return
      }
      self.fullScreenTransition = false
      if self.styleMask.contains(.fullScreen) {
        self.watchFullScreenTitleBar()
        self.updateReveal()
      } else {
        self.unwatchFullScreenTitleBar()
        self.trafficLightState = .windowed
        self.placeTrafficLights()
      }
    }
  }

  // A window restored in full screen may have entered it before the
  // observers of `awakeFromNib` were in place, so nobody watches its title
  // bar. Called once after `awakeFromNib` and whenever Dart asks.
  func resumeFullScreenWatch() {
    guard styleMask.contains(.fullScreen), !fullScreenTransition,
      titleBarObservations.isEmpty
    else {
      return
    }
    watchFullScreenTitleBar()
    updateReveal()
  }

  @objc private func didExitFullScreen() {
    fullScreenTransition = false
    trafficLightState = .windowed
  }

  // How the reveal is detected. In full screen AppKit moves the title bar,
  // and with it the three buttons, into a window of its own
  // (`NSToolbarFullScreenWindow`). When the pointer reaches the top edge,
  // AppKit slides that title bar down below the menu bar, and up and out of
  // the screen when the pointer leaves. There is no public notification for
  // either. What reliably changes is geometry: AppKit moves the title bar
  // container view inside that window (the same frame Chromium observes for
  // its immersive full screen), and depending on the release it also moves or
  // hides the window itself. So the frames of the container and of the bar
  // are watched with KVO, the window with its move, resize and occlusion
  // notifications, and every time one fires `updateReveal` measures where the
  // close button stands on the screen. Measuring instead of interpreting a
  // single signal keeps the answer right whichever of them a release uses,
  // and also when the menu bar is set to stay in full screen and the title
  // bar never hides.
  private func watchFullScreenTitleBar() {
    unwatchFullScreenTitleBar()
    guard let button = standardWindowButton(.closeButton), let bar = button.superview else {
      return
    }
    var views: [NSView] = [bar]
    if let container = bar.superview {
      views.append(container)
    }
    titleBarObservations = views.map { view in
      view.observe(\.frame, options: [.new]) { [weak self] _, _ in
        DispatchQueue.main.async { self?.updateReveal() }
      }
    }
    guard let titleWindow = button.window, titleWindow !== self else {
      return
    }
    let center = NotificationCenter.default
    for name in [
      NSWindow.didMoveNotification,
      NSWindow.didResizeNotification,
      NSWindow.didChangeOcclusionStateNotification,
    ] {
      titleBarWindowObservers.append(
        center.addObserver(forName: name, object: titleWindow, queue: .main) { [weak self] _ in
          self?.updateReveal()
        })
    }
  }

  private func unwatchFullScreenTitleBar() {
    titleBarObservations.forEach { $0.invalidate() }
    titleBarObservations = []
    titleBarWindowObservers.forEach { NotificationCenter.default.removeObserver($0) }
    titleBarWindowObservers = []
  }

  private func updateReveal() {
    guard styleMask.contains(.fullScreen), !fullScreenTransition else {
      return
    }
    trafficLightState = trafficLightsShown() ? .fullScreenRevealed : .fullScreenHidden
  }

  // Shown means at least half of the close button lies on the screen and
  // inside the window that carries it, and that window is drawn at all.
  private func trafficLightsShown() -> Bool {
    guard let button = standardWindowButton(.closeButton),
      !button.isHiddenOrHasHiddenAncestor,
      let titleWindow = button.window,
      titleWindow.isVisible,
      titleWindow.alphaValue > 0,
      let screen = titleWindow.screen ?? self.screen
    else {
      return false
    }
    let rect = titleWindow.convertToScreen(button.convert(button.bounds, to: nil))
    let shown = rect.intersection(titleWindow.frame).intersection(screen.frame)
    return !shown.isNull && shown.height >= rect.height / 2
  }

  override func layoutIfNeeded() {
    super.layoutIfNeeded()
    placeTrafficLights()
  }

  // The title bar view at the height of the bar, at the top of the window.
  private func fitTitleBar() {
    guard let bar = standardWindowButton(.closeButton)?.superview,
      let container = bar.superview
    else {
      return
    }
    let height = max(MainFlutterWindow.systemTitleBarHeight, headerBarHeight)
    var frame = container.frame
    frame.size.height = height
    frame.origin.y = self.frame.height - height
    if container.frame != frame {
      container.frame = frame
    }
    if bar.frame.height != height {
      bar.setFrameSize(NSSize(width: bar.frame.width, height: height))
    }
  }

  // Moves the three buttons inside the system title bar view. It is 28 pt high,
  // and a button centred on a larger scaled bar would fall out of it and stop
  // being drawn, so the view grows with a taller bar, and nothing
  // above the header bar of the app starts to catch clicks. In full screen the
  // system shows its own bar; there the buttons are left alone.
  @objc private func placeTrafficLights() {
    if styleMask.contains(.fullScreen) {
      return
    }
    fitTitleBar()
    let kinds: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]
    for (index, kind) in kinds.enumerated() {
      guard let button = standardWindowButton(kind), let bar = button.superview else {
        continue
      }
      let size = button.frame.size
      let fromTop = headerBarHeight / 2 - size.height / 2
      let y = bar.isFlipped ? fromTop : bar.bounds.height - fromTop - size.height
      let origin = NSPoint(
        x: MainFlutterWindow.firstButtonLeft + CGFloat(index) * MainFlutterWindow.buttonPitch,
        y: max(0, y)
      )
      if button.frame.origin != origin {
        button.setFrameOrigin(origin)
      }
    }
  }
}
