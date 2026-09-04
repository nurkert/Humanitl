import 'package:flutter/widgets.dart';

import '../../core/ui/ui.dart';

/// Hosts the design gallery of `packages/ui` as a standalone application.
///
/// Started with `HUMANITL_GALLERY=1 flutter run -d linux` or with the
/// `--gallery` flag. It is the only screen that is allowed to exist before the
/// shell of HUM-019: it renders tokens, not product state, and it is the later
/// basis of the golden tests of HUM-054.
class GalleryScreen extends StatelessWidget {
  /// Creates the gallery application.
  const GalleryScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return WidgetsApp(
      color: HColors.bg0,
      title: 'Humanitl design gallery',
      debugShowCheckedModeBanner: false,
      builder: (BuildContext context, Widget? child) => const HGalleryPage(),
    );
  }
}
