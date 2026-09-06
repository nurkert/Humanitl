/// The address of the language model, and the one gesture that contacts it
/// (HUM-039, HUM-044).
///
/// # Why this control exists at all
///
/// The endpoint is the one address of this product that traffic reaches
/// **without** passing the queue (`BACKLOG.md` 4.2, side channel two). Typing
/// it is therefore a decision, and the control is built so that the decision
/// is taken once, deliberately:
///
/// - **Nothing is contacted while somebody types.** A probe per keystroke
///   would send `o`, `ol`, `oll`, `olla` into DNS before anybody had decided
///   on a name -- the name would be out on the network before the decision
///   was made. The probe runs on the button and on `Enter`, and on nothing
///   else.
/// - **The button says what it will do.** It is the only control here that
///   opens a connection, and it is labelled for that and not for "save".
/// - **The field never writes anything.** There is no write path into
///   `config.toml` yet (`SetConfig` answers `unimplemented` until HUM-069),
///   so the text lives for this session and the control does not pretend
///   otherwise.
///
/// It lives in `core/ui` because setup and, later, the settings screen show
/// the same field, and no feature imports another one (ARCHITECTURE 5).
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// The endpoint field with its probe button.
class LlmEndpointField extends StatelessWidget {
  /// Creates the field over [controller].
  ///
  /// [onProbe] is called with the trimmed text, never with the raw one, and
  /// never for an empty field: an empty endpoint is not an address, and
  /// asking the daemon about it would only produce a finding this control can
  /// see coming.
  const LlmEndpointField({
    required this.controller,
    required this.label,
    required this.hint,
    required this.probeLabel,
    required this.onProbe,
    this.onChanged,
    this.busy = false,
    this.enabled = true,
    super.key,
  });

  /// The text being edited.
  final TextEditingController controller;

  /// Screen-reader label of the field, already localised.
  final String label;

  /// Placeholder while the field is empty, already localised.
  final String hint;

  /// Label of the button that opens the connection, already localised.
  final String probeLabel;

  /// Called with the trimmed endpoint when somebody asks for a probe.
  final void Function(String endpoint) onProbe;

  /// Called with every text a person types.
  ///
  /// It never contacts anything -- that is the whole point of this control --
  /// but it is what lets a caller drop a result that was measured for another
  /// address (HUM-044).
  final ValueChanged<String>? onChanged;

  /// True while a probe is in flight; the button rests then.
  final bool busy;

  /// False while nothing may be asked at all -- no daemon, for instance.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      children: <Widget>[
        Expanded(
          child: HTextField(
            key: const Key('setup-llm-endpoint'),
            controller: controller,
            semanticsLabel: label,
            hint: hint,
            enabled: enabled && !busy,
            onChanged: onChanged,
            onSubmitted: (_) => _probe(),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        HButton(
          key: const Key('setup-llm-probe'),
          size: HButtonSize.sm,
          onPressed: enabled && !busy ? _probe : null,
          child: Text(probeLabel),
        ),
      ],
    );
  }

  void _probe() {
    final String endpoint = controller.text.trim();
    if (endpoint.isEmpty) {
      return;
    }
    onProbe(endpoint);
  }
}
