/// The address and the model of a saved model server's form (HUM-405,
/// HUM-414): the address with "Check" and what it says by itself, the model
/// as free text with the list the last check fetched.
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/pasted_text.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/server_checks.dart';
import 'server_form.dart';
import 'server_notes.dart';

/// The address with "Check" beside it, and under it where the address
/// points, what changed elsewhere and what the last check found.
class ServerAddressField extends ConsumerWidget {
  /// The address of [form].
  const ServerAddressField({
    required this.form,
    required this.onCheck,
    super.key,
  });

  /// The form the field belongs to.
  final ServerForm form;

  /// "Check".
  final Future<void> Function() onCheck;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String endpoint = form.endpointText;
    final ServerPlace? place = serverPlace(endpoint);
    final SavedServer? server = form.server;
    ref.watch(serverChecksProvider);
    final ServerCheck? last = server == null
        ? null
        : ref.read(serverChecksProvider.notifier).of(server.name, endpoint);
    final bool fresh = form.probe == null && form.probeFailure == null;
    Widget note(Key key, HGlyph glyph, Color color, Color text, String says) =>
        Padding(
          padding: EdgeInsets.only(top: tokens.spacing.x1),
          child: ServerNote(
            key: key,
            glyph: glyph,
            color: color,
            textColor: text,
            text: says,
          ),
        );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              child: HTextField(
                key: const Key('settings-endpoint'),
                controller: form.endpoint,
                enabled: form.savedWithoutKey == null && !form.busy,
                semanticsLabel: l10n.settingsEndpointLabel,
                hint: l10n.settingsEndpointHint,
                inputFormatters: const <TextInputFormatter>[
                  PasteDebrisFormatter(),
                ],
              ),
            ),
            SizedBox(width: tokens.spacing.x2),
            HButton(
              key: const Key('settings-check'),
              semanticsLabel:
                  '${l10n.settingsCheck}. ${l10n.settingsServerCheckHint}',
              onPressed: form.probing || endpoint.isEmpty
                  ? null
                  : () => unawaited(onCheck()),
              child: Text(
                form.probing ? l10n.settingsChecking : l10n.settingsCheck,
              ),
            ),
          ],
        ),
        SizedBox(height: tokens.spacing.x1),
        if (place != null)
          Text(
            switch (place.kind) {
              ServerPlaceKind.thisMachine => l10n.settingsServerPlaceLocal,
              ServerPlaceKind.localNetwork => l10n.settingsServerPlaceLan(
                place.host,
              ),
              ServerPlaceKind.remote => l10n.settingsServerPlaceRemote(
                place.host,
              ),
            },
            key: const Key('settings-endpoint-place'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        if (place != null && place.plainOverInternet)
          note(
            const Key('settings-endpoint-plain-http'),
            HGlyph.triangleAlert,
            tokens.colors.warning,
            tokens.colors.warningText,
            l10n.settingsServerPlainHttp,
          ),
        if (form.endpointElsewhere case final String elsewhere)
          note(
            const Key('settings-endpoint-elsewhere'),
            HGlyph.info,
            tokens.colors.fg1,
            tokens.colors.fg1,
            l10n.settingsServerChangedElsewhere(displayEndpoint(elsewhere)),
          ),
        if (fresh && last != null)
          note(
            const Key('settings-endpoint-last-check'),
            last.ok ? HGlyph.check : HGlyph.triangleAlert,
            last.ok ? tokens.stateText.allowed : tokens.stateText.error,
            last.ok ? tokens.colors.fg1 : tokens.stateText.error,
            l10n.settingsServerLastCheck(
              last.ok
                  ? l10n.settingsServerCheckedModels(last.models)
                  : l10n.settingsServerCheckFailed,
            ),
          )
        else if (fresh)
          Text(
            l10n.settingsServerCheckHint,
            style: tokens.typography.ui12.tinted(tokens.colors.fg2),
          ),
      ],
    );
  }
}

/// The model as free text, with the list the last check fetched to choose
/// from; "List unavailable" when the server answered without one.
class ServerModelField extends StatefulWidget {
  /// The model of [form].
  const ServerModelField({required this.form, super.key});

  /// The form the field belongs to.
  final ServerForm form;

  @override
  State<ServerModelField> createState() => _ServerModelFieldState();
}

class _ServerModelFieldState extends State<ServerModelField> {
  final HContextMenuController _menu = HContextMenuController();
  final GlobalKey _anchor = GlobalKey();

  void _open() {
    final RenderObject? box = _anchor.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.hasSize) {
      return;
    }
    _menu.open(box.localToGlobal(Offset(0, box.size.height + 2)));
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final ServerForm form = widget.form;
    final LlmProbe? probe = form.probe;
    final List<String> offered = probe?.shownModels ?? const <String>[];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              child: HTextField(
                key: const Key('settings-model'),
                controller: form.model,
                enabled: form.savedWithoutKey == null && !form.busy,
                semanticsLabel: l10n.settingsModelLabel,
                hint: l10n.settingsModelHint,
              ),
            ),
            if (offered.isNotEmpty) ...<Widget>[
              SizedBox(width: tokens.spacing.x2),
              HContextMenu(
                controller: _menu,
                semanticsLabel: l10n.settingsModelPick,
                itemsBuilder: () => <HMenuItem>[
                  for (final String name in offered)
                    HMenuItem(
                      label: name,
                      enabled: name != form.model.text.trim(),
                      onSelected: () => form.pickModel(name),
                    ),
                ],
                child: KeyedSubtree(
                  key: _anchor,
                  child: HButton(
                    key: const Key('settings-model-pick'),
                    onPressed: _open,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        Text(l10n.settingsModelPick),
                        SizedBox(width: tokens.spacing.x1),
                        RotatedBox(
                          quarterTurns: 1,
                          child: HGlyphIcon(
                            HGlyph.chevronRight,
                            size: 12,
                            color: tokens.colors.fg1,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ],
          ],
        ),
        if (probe != null && probe.models.isEmpty)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: Text(
              l10n.settingsModelListUnavailable,
              key: const Key('settings-model-no-list'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
      ],
    );
  }
}
