/// The category Models: endpoint, model, access key and the saved servers
/// (HUM-140, HUM-289, HUM-404).
///
/// The form itself stays `ModelSection` as before; this file only places it
/// in the overlay. Its rework into forms of their own is HUM-405.
library;

import 'package:flutter/widgets.dart';

import '../registry/setting_entry.dart';
import '../widgets/model_section.dart';

/// Draws the whole category: the three rows of the active server and the
/// list of saved servers under them.
Widget buildModelsCategory(SettingsPageData data) =>
    ModelSection(config: data.config, status: data.status);
