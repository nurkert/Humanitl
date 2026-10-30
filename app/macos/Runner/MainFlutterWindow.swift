import Cocoa
import FlutterMacOS

// The window of the app (HUM-315): title Humanitl, 1280 x 720 at the first
// start, and no second title bar. The Flutter header bar is the title bar, as
// on Linux (HUM-255); macOS keeps its traffic lights at the left end of it.
class MainFlutterWindow: NSWindow {
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

    super.awakeFromNib()
  }
}
