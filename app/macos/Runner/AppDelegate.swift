import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  // The app outlives its window (HUM-373): daemon and agent run on, and a held
  // request has to be announced by the menu bar item, the Dock badge and the
  // notification. Quit ends it, from the menu bar or with Cmd-Q.
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return false
  }

  // A click on the Dock icon with the window closed brings the window back.
  override func applicationShouldHandleReopen(
    _ sender: NSApplication, hasVisibleWindows flag: Bool
  ) -> Bool {
    if !flag {
      mainFlutterWindow?.makeKeyAndOrderFront(nil)
    }
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
