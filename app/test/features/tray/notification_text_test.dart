// Text aus dem Dateisystem im Körper einer Benachrichtigung (HUM-539): ohne
// Steuerzeichen und ohne Markup, das ein Dienst als Auszeichnung läse.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/tray/attention_text.dart';

void main() {
  test('control characters go, markup becomes entities, the rest stays', () {
    expect(
      plainNotificationText('/home/dev/a\nb\tc\u001b[31m'),
      '/home/dev/abc[31m',
    );
    expect(plainNotificationText('/x/\u0085y'), '/x/y');
    expect(
      plainNotificationText('<a href="x">&</a>'),
      '&lt;a href="x"&gt;&amp;&lt;/a&gt;',
    );
    expect(plainNotificationText('/home/dev/repo '), '/home/dev/repo ');
    expect(plainNotificationText('/home/dev/münchen'), '/home/dev/münchen');
    // Trenner und Schreibrichtung ordnen die Anzeige sonst unsichtbar um.
    expect(plainNotificationText('/a\u2028b\u202ec\u2066d'), '/abcd');
  });
}
