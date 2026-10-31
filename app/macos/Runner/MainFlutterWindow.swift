import Cocoa
import FlutterMacOS

// The window of the app (HUM-325): title Humanitl, 1280 x 720 at the first
// start, and no second title bar. The Flutter header bar is the title bar, as
// on Linux (HUM-255); macOS keeps its traffic lights at the left end of it.
class MainFlutterWindow: NSWindow {
  // Where the traffic lights sit (HUM-342): centred on the 40 pt header bar of
  // the app (`HSize.headerBar`) instead of the 28 pt title bar of the system,
  // 16 pt from the left edge, which leaves a 14 pt gap between the last button
  // and the wordmark. `trafficLightInset` in window_frame.dart leaves
  // room up to 72 pt; the buttons end at 70.
  private static let headerBarHeight: CGFloat = 40
  private static let firstButtonLeft: CGFloat = 16
  private static let buttonPitch: CGFloat = 20

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
    placeTrafficLights()
    DispatchQueue.main.async { [weak self] in self?.placeTrafficLights() }
  }

  override func layoutIfNeeded() {
    super.layoutIfNeeded()
    placeTrafficLights()
  }

  // Moves the three buttons inside the system title bar view, which stays as
  // it is: it is 28 pt high, and a button centred 20 pt below the top edge
  // still lies completely inside it, so nothing above the header bar of the
  // app starts to catch clicks. In full screen the system shows its own bar;
  // there the buttons are left alone.
  @objc private func placeTrafficLights() {
    if styleMask.contains(.fullScreen) {
      return
    }
    let kinds: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]
    for (index, kind) in kinds.enumerated() {
      guard let button = standardWindowButton(kind), let bar = button.superview else {
        continue
      }
      let size = button.frame.size
      let fromTop = MainFlutterWindow.headerBarHeight / 2 - size.height / 2
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
