/// Entry point of the Humanitl desktop application.
library;

import 'package:flutter/widgets.dart';

void main() {
  runApp(const HumanitlApp());
}

/// Placeholder root widget; replaced by the real shell in HUM-019.
class HumanitlApp extends StatelessWidget {
  /// Creates the placeholder root widget.
  const HumanitlApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const ColoredBox(color: Color(0xFF0F1115));
  }
}
