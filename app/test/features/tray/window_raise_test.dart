// Ob der Schreibtisch das Fenster der App selbst nach vorn holen lässt
// (HUM-539): auf Wayland nicht, dann sagt eine Meldung, dass das
// Startfenster wartet.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/tray/platform/window_port.dart';

void main() {
  test('X11 and macOS raise the window, Wayland does not unless GTK runs on '
      'X11', () {
    expect(
      WindowManagerPort.raisesItselfIn(false, const <String, String>{
        'WAYLAND_DISPLAY': 'wayland-1',
      }),
      isTrue,
    );
    expect(
      WindowManagerPort.raisesItselfIn(true, const <String, String>{
        'DISPLAY': ':0',
      }),
      isTrue,
    );
    expect(
      WindowManagerPort.raisesItselfIn(true, const <String, String>{
        'WAYLAND_DISPLAY': 'wayland-1',
      }),
      isFalse,
    );
    expect(
      WindowManagerPort.raisesItselfIn(true, const <String, String>{
        'WAYLAND_DISPLAY': 'wayland-1',
        'GDK_BACKEND': 'x11',
      }),
      isTrue,
    );
    expect(
      WindowManagerPort.raisesItselfIn(true, const <String, String>{
        'WAYLAND_DISPLAY': 'wayland-1',
        'GDK_BACKEND': 'wayland,x11',
      }),
      isFalse,
    );
  });
}
