/// The saved model servers as cards, and the card that adds one (HUM-405).
///
/// A card says what a server is at a glance: its name, its host, its default
/// model, a star when it is the default, whether a key is stored, and what
/// the last "Check" of this app session found. It sends nothing; a click
/// opens the detail view of that server, where every change is made.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/server_checks.dart';

/// The least width of one card. The grid puts as many in a row as fit at
/// this width and stretches them to fill it, so a row never ends ragged.
const double serverCardMinWidth = 216;

/// The cards of [servers] and, at the end, the card that adds one.
class ServerCards extends StatelessWidget {
  /// Creates the grid.
  const ServerCards({
    required this.servers,
    required this.onOpen,
    required this.onAdd,
    super.key,
  });

  /// The saved servers, in name order.
  final SavedServers servers;

  /// Opens the detail view of a server.
  final ValueChanged<String> onOpen;

  /// Opens the detail view of a new server.
  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    // Der Standard zuerst: Er bedient jede Sitzung ohne eigene Wahl, und wer
    // die Seite öffnet, sucht meist ihn.
    final List<SavedServer> ordered = <SavedServer>[
      ...servers.servers.where((SavedServer server) => server.active),
      ...servers.servers.where((SavedServer server) => !server.active),
    ];
    final double gap = tokens.spacing.x3;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double room = constraints.maxWidth;
        final int columns = ((room + gap) / (serverCardMinWidth + gap))
            .floor()
            .clamp(1, 4);
        final double width = ((room - gap * (columns - 1)) / columns)
            .floorToDouble();
        return Wrap(
          key: const Key('settings-server-cards'),
          spacing: gap,
          runSpacing: gap,
          children: <Widget>[
            for (final SavedServer server in ordered)
              _ServerCard(
                key: ValueKey<String>('settings-server-${server.name}'),
                server: server,
                width: width,
                onOpen: () => onOpen(server.name),
              ),
            _AddCard(onAdd: onAdd, width: width),
          ],
        );
      },
    );
  }
}

/// The host and port of [endpoint], without scheme, user or path.
@visibleForTesting
String serverHost(String endpoint) {
  final Uri? uri = Uri.tryParse(endpoint);
  if (uri == null || uri.host.isEmpty) {
    return endpoint;
  }
  return uri.hasPort ? '${uri.host}:${uri.port}' : uri.host;
}

/// The surface of a card: a raised `bg2` with a hairline, the hairline
/// stronger under the pointer, the whole card a little larger (HHoverScale,
/// HUM-406); a click, `Enter` or `Space` opens it.
class _CardSurface extends StatefulWidget {
  const _CardSurface({
    required this.semanticsLabel,
    required this.onTap,
    required this.child,
    required this.width,
    this.quiet = false,
  });

  final double width;

  final String semanticsLabel;
  final VoidCallback onTap;
  final Widget child;

  /// The card that adds: no raised surface until the pointer is on it.
  final bool quiet;

  @override
  State<_CardSurface> createState() => _CardSurfaceState();
}

class _CardSurfaceState extends State<_CardSurface> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color fill = widget.quiet && !_hovered
        ? tokens.colors.bg1
        : tokens.colors.bg2;
    final Color edge = _hovered ? tokens.colors.lineStrong : tokens.colors.line;
    return Semantics(
      button: true,
      label: widget.semanticsLabel,
      excludeSemantics: true,
      child: FocusableActionDetector(
        mouseCursor: SystemMouseCursors.click,
        onShowHoverHighlight: (bool on) => setState(() => _hovered = on),
        onShowFocusHighlight: (bool on) => setState(() => _focused = on),
        shortcuts: const <ShortcutActivator, Intent>{
          SingleActivator(LogicalKeyboardKey.enter): ActivateIntent(),
          SingleActivator(LogicalKeyboardKey.space): ActivateIntent(),
        },
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent _) {
              widget.onTap();
              return null;
            },
          ),
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: HHoverScale(
            hovered: _hovered,
            child: HFocusRing.inline(
              visible: _focused,
              radius: tokens.radii.card,
              child: AnimatedContainer(
                duration: HMotion.press,
                curve: HMotion.enter,
                width: widget.width,
                constraints: const BoxConstraints(minHeight: 144),
                decoration: BoxDecoration(
                  color: fill,
                  border: Border.all(color: edge),
                  borderRadius: BorderRadius.circular(tokens.radii.card),
                ),
                padding: EdgeInsets.all(tokens.spacing.x3),
                child: widget.child,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// One saved server.
class _ServerCard extends ConsumerWidget {
  const _ServerCard({
    required this.server,
    required this.width,
    required this.onOpen,
    super.key,
  });

  final double width;

  final SavedServer server;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final String name = server.name;
    ref.watch(serverChecksProvider);
    final ServerCheck? check = ref
        .read(serverChecksProvider.notifier)
        .of(name, server.endpoint);
    final String host = server.endpoint.isEmpty
        ? l10n.settingsServerNoAddress
        : serverHost(server.endpoint);
    return _CardSurface(
      semanticsLabel: <String>[
        l10n.settingsServerOpen(name),
        if (server.active) l10n.settingsServerDefault,
        host,
        if (server.model.isNotEmpty) server.model,
        if (server.hasKey)
          l10n.settingsServerKeyStoredShort
        else
          l10n.settingsServerKeyNoneShort,
      ].join(', '),
      onTap: onOpen,
      width: width,
      child: _CardBody(server: server, host: host, check: check),
    );
  }
}

/// What a card of a saved server shows; also the invisible template that
/// gives the card "Add server" the same height (HUM-405, review).
class _CardBody extends StatelessWidget {
  const _CardBody({
    required this.server,
    required this.host,
    required this.check,
  });

  final SavedServer server;
  final String host;
  final ServerCheck? check;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String name = server.name;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              child: Text(
                name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tokens.typography.mono12.medium.tinted(
                  tokens.colors.fg0,
                ),
              ),
            ),
            if (server.active) ...<Widget>[
              SizedBox(width: tokens.spacing.x2),
              HGlyphIcon(
                HGlyph.star,
                key: ValueKey<String>('settings-server-star-$name'),
                size: 12,
                color: tokens.colors.accent,
              ),
              SizedBox(width: tokens.spacing.x1),
              Text(
                l10n.settingsServerDefault,
                key: ValueKey<String>('settings-server-default-$name'),
                style: tokens.typography.ui11.medium.tinted(
                  tokens.colors.accentText,
                ),
              ),
            ],
          ],
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(
          host,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: tokens.typography.mono11.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x3),
        Text(
          server.model.isEmpty ? l10n.settingsServerNoModel : server.model,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: server.model.isEmpty
              ? tokens.typography.ui12.tinted(tokens.colors.fg2)
              : tokens.typography.ui13.tinted(tokens.colors.fg0),
        ),
        SizedBox(height: tokens.spacing.x3),
        const HHairline(),
        SizedBox(height: tokens.spacing.x2),
        Row(
          children: <Widget>[
            HGlyphIcon(
              HGlyph.lock,
              size: 12,
              color: server.hasKey ? tokens.colors.fg1 : tokens.colors.fg2,
            ),
            SizedBox(width: tokens.spacing.x1),
            Expanded(
              child: Text(
                server.hasKey
                    ? l10n.settingsServerKeyStoredShort
                    : l10n.settingsServerKeyNoneShort,
                key: ValueKey<String>('settings-server-key-$name'),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tokens.typography.ui11.tinted(
                  server.hasKey ? tokens.colors.fg1 : tokens.colors.fg2,
                ),
              ),
            ),
          ],
        ),
        SizedBox(height: tokens.spacing.x1),
        _CheckLine(name: name, check: check),
      ],
    );
  }
}

/// What the last check of this app session found, at the right of the card.
class _CheckLine extends StatelessWidget {
  const _CheckLine({required this.name, required this.check});

  final String name;
  final ServerCheck? check;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final ServerCheck? check = this.check;
    final (HGlyph? glyph, String text, Color color) = switch (check) {
      null => (null, l10n.settingsServerUnchecked, tokens.colors.fg2),
      ServerCheck(ok: true) => (
        HGlyph.check,
        l10n.settingsServerCheckedModels(check.models),
        tokens.stateText.allowed,
      ),
      _ => (
        HGlyph.triangleAlert,
        l10n.settingsServerCheckFailed,
        tokens.stateText.error,
      ),
    };
    return Row(
      key: ValueKey<String>('settings-server-checked-$name'),
      children: <Widget>[
        HGlyphIcon(glyph ?? HGlyph.minus, size: 12, color: color),
        SizedBox(width: tokens.spacing.x1),
        Flexible(
          child: Text(
            text,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tokens.typography.ui11.tinted(color),
          ),
        ),
      ],
    );
  }
}

/// The card at the end that adds a server.
class _AddCard extends StatelessWidget {
  const _AddCard({required this.onAdd, required this.width});

  final double width;

  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return KeyedSubtree(
      key: const Key('settings-server-add'),
      child: _CardSurface(
        semanticsLabel: l10n.settingsServerAdd,
        onTap: onAdd,
        width: width,
        quiet: true,
        // Ein unsichtbarer Abdruck einer Serverkarte gibt die Höhe vor; jede
        // Zeile des Inhalts ist einzeilig, also sind alle Karten gleich hoch,
        // bei jeder Textgröße.
        child: Stack(
          children: <Widget>[
            const ExcludeSemantics(
              child: Visibility(
                visible: false,
                maintainSize: true,
                maintainAnimation: true,
                maintainState: true,
                child: _CardBody(
                  server: SavedServer(name: 'x', models: <String>['x']),
                  host: 'x',
                  check: null,
                ),
              ),
            ),
            Positioned.fill(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: <Widget>[
                  HGlyphIcon(HGlyph.plus, size: 16, color: tokens.colors.fg1),
                  SizedBox(height: tokens.spacing.x2),
                  Text(
                    l10n.settingsServerAdd,
                    style: tokens.typography.ui13.medium.tinted(
                      tokens.colors.fg0,
                    ),
                  ),
                  SizedBox(height: tokens.spacing.x1),
                  Text(
                    l10n.settingsServerAddCardHint,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
