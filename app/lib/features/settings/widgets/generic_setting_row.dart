/// The row of a catalog entry without a builder of its own (HUM-534): drawn
/// from its control alone, so a new entry of the daemon's catalog appears
/// without a change in the app.
///
/// The value is the one `GetConfig` answered; what is written goes through
/// `SetConfig` as text, and the daemon decides whether it is valid (ADR-018).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../settings_scope.dart';
import 'setting_controls.dart';
import 'setting_row.dart';

/// A row drawn from [entry]'s control: a switch, a choice, or a text field
/// written when it is left or submitted.
class GenericSettingRow extends StatefulWidget {
  /// Creates the row of [entry] over [config].
  const GenericSettingRow({
    required this.entry,
    required this.config,
    super.key,
  });

  /// The catalog entry.
  final SettingsCatalogEntry entry;

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  State<GenericSettingRow> createState() => _GenericSettingRowState();
}

class _GenericSettingRowState extends State<GenericSettingRow> {
  late final TextEditingController _text = TextEditingController(text: _shown);

  String get _shown => switch (widget.config.at(widget.entry.id)) {
    null => '',
    final String text => text,
    final Object other => '$other',
  };

  /// Whether the field has the keyboard; while it has, a new answer of the
  /// daemon does not overwrite what is being typed.
  bool _editing = false;

  /// A `config_changed` brings a new [ConfigValues] (HUM-534): the field
  /// follows it while nobody types, so leaving it later does not write the
  /// old value back.
  @override
  void didUpdateWidget(GenericSettingRow oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_editing && _text.text != _shown) {
      _text.text = _shown;
    }
  }

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final String text = _text.text.trim();
    if (text == _shown) {
      return;
    }
    await SettingsScope.maybeOf(context)
        ?.write(widget.entry.id, text.isEmpty ? '-' : text);
  }

  @override
  Widget build(BuildContext context) {
    final SettingsCatalogEntry entry = widget.entry;
    final Key key = ValueKey<String>('settings-generic-${entry.id}');
    final Widget control = switch (entry.control) {
      SettingControl.toggle => OnOffControl(
        key: key,
        value: widget.config.at(entry.id) == true,
        onChanged: (bool on) => writeSetting(context, entry.id, '$on'),
      ),
      SettingControl.choice => HSegmented<String>(
        key: key,
        options: <HSegmentOption<String>>[
          for (final SettingsChoice choice in entry.choices)
            HSegmentOption<String>(value: choice.value, label: choice.title),
        ],
        selected: _shown,
        onSelect: (String value) => writeSetting(context, entry.id, value),
      ),
      _ => Focus(
        onFocusChange: (bool focused) {
          _editing = focused;
          if (!focused) {
            unawaited(_save());
          }
        },
        child: HTextField(
          key: key,
          controller: _text,
          semanticsLabel: entry.title,
          digitsOnly: entry.control == SettingControl.number,
          onSubmitted: (String _) => unawaited(_save()),
        ),
      ),
    };
    return SettingRow(
      settingId: entry.id,
      fill:
          entry.control != SettingControl.toggle &&
          entry.control != SettingControl.choice,
      child: control,
    );
  }
}
