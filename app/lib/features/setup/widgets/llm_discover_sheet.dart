/// The sheet that searches the local network for model servers (HUM-076).
///
/// # It explains before it acts
///
/// A network scan is a thing a person should understand before it happens, so
/// the sheet opens with the sentence and the button, and with nothing running.
/// The sentence names the ports and the boundary — the own `/24` — because
/// that is exactly what an IDS in a company network will see and what a
/// colleague may ask about. Opening this sheet contacts nothing; the button
/// does.
///
/// # The motion is the progress
///
/// The daemon streams answers, not percentages, so this sheet does not show
/// one. While the search runs, a hairline sweeps under the headline: enough
/// motion to say "this is working", too little to claim a position. Every
/// found server arrives the way a request arrives in the queue — eight pixels
/// from above in 180 ms — and it stays where it landed. Both movements run
/// through [HReducedMotion] and become still when the system asks for less
/// motion; the sweep then does not run at all, because a loop is the one
/// thing reduced motion is about.
///
/// # Everything in a row came from the network
///
/// Host, port and model names are what an unauthenticated machine on the LAN
/// answered. They are drawn as text and as nothing else: no link, no path, no
/// command. A server that answered neither API is listed as what it is and
/// cannot be taken over — a click would otherwise put an address into the
/// configuration that has never answered as a model server.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/discover_provider.dart';
import '../setup_text.dart';

/// The ports the search asks, for the sentence above the button.
///
/// The same four the daemon uses (`humanitl_proxy::DEFAULT_PORTS`). They stand
/// here as text because the sentence is a promise about what leaves this
/// machine, and a promise with a placeholder nobody filled would be worse than
/// none.
const List<int> discoverPorts = <int>[11434, 1234, 8000, 8080];

/// The search sheet.
class LlmDiscoverSheet extends ConsumerStatefulWidget {
  /// Creates the sheet.
  const LlmDiscoverSheet({
    required this.onClose,
    required this.onPick,
    super.key,
  });

  /// Closes the sheet. A running search is stopped with it.
  final VoidCallback onClose;

  /// Called with the endpoint somebody took over.
  final void Function(String endpoint) onPick;

  @override
  ConsumerState<LlmDiscoverSheet> createState() => _LlmDiscoverSheetState();
}

class _LlmDiscoverSheetState extends ConsumerState<LlmDiscoverSheet>
    with SingleTickerProviderStateMixin {
  late final AnimationController _sweep = AnimationController(
    vsync: this,
    duration: HMotion.breathe,
  );

  @override
  void dispose() {
    _sweep.dispose();
    super.dispose();
  }

  /// Starts and stops the sweep with the search.
  ///
  /// A controller that keeps ticking behind a finished search would burn a
  /// frame budget for a state that no longer exists (`docs/UX.md` 6.4), and a
  /// loop under reduced motion is exactly what that setting asks us not to
  /// run: the line then stays still and the count next to the button carries
  /// the progress alone.
  void _follow({required bool running, required bool reduced}) {
    if (running && !reduced && !_sweep.isAnimating) {
      _sweep.repeat();
    } else if ((!running || reduced) && _sweep.isAnimating) {
      _sweep.stop();
      _sweep.value = 0;
    }
  }

  void _close() {
    ref.read(setupLlmDiscoverProvider.notifier).stop();
    widget.onClose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final LlmDiscoverState state = ref.watch(setupLlmDiscoverProvider);
    _follow(running: state.running, reduced: HReducedMotion.of(context));

    return HModal(
      title: Text(l10n.setupLlmDiscoverTitle),
      onDismiss: _close,
      width: 520,
      scrimSemanticsLabel: l10n.setupLlmDiscoverClose,
      actions: <Widget>[
        if (state.running)
          HButton(
            key: const Key('setup-discover-stop'),
            variant: HButtonVariant.secondary,
            onPressed: ref.read(setupLlmDiscoverProvider.notifier).stop,
            child: Text(l10n.setupLlmDiscoverCancel),
          )
        else
          HButton(
            key: const Key('setup-discover-close'),
            variant: HButtonVariant.secondary,
            onPressed: _close,
            child: Text(l10n.setupLlmDiscoverClose),
          ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            l10n.setupLlmDiscoverExplain(
              discoverPorts.map((int port) => '$port').join(', '),
            ),
            style: tokens.typography.ui13.tinted(tokens.colors.fg1),
          ),
          SizedBox(height: tokens.spacing.x3),
          _Sweep(controller: _sweep, running: state.running),
          SizedBox(height: tokens.spacing.x3),
          Row(
            children: <Widget>[
              HButton(
                key: const Key('setup-discover-start'),
                autofocus: true,
                onPressed: state.running
                    ? null
                    : ref.read(setupLlmDiscoverProvider.notifier).start,
                child: Text(l10n.setupLlmDiscoverStart),
              ),
              SizedBox(width: tokens.spacing.x3),
              Expanded(
                child: Text(
                  state.running
                      ? l10n.setupLlmDiscoverRunning(state.servers.length)
                      : state.isIdle
                      ? ''
                      : l10n.setupLlmDiscoverDone(state.servers.length),
                  key: const Key('setup-discover-status'),
                  style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                ),
              ),
            ],
          ),
          if (state.failure case final Diagnostic failure) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            _Failure(failure: failure),
          ],
          // Das Blatt wächst, wenn der erste Server antwortet. `AnimatedSize`
          // macht daraus eine Bewegung statt eines Sprungs; unter reduzierter
          // Bewegung ist die Dauer null und das Blatt steht sofort in seiner
          // neuen Größe.
          AnimatedSize(
            duration: HReducedMotion.displace(context, HMotion.arrive),
            curve: HMotion.enter,
            alignment: Alignment.topCenter,
            child: state.servers.isEmpty
                ? const SizedBox(width: double.infinity)
                : Padding(
                    padding: EdgeInsets.only(top: tokens.spacing.x3),
                    child: _Servers(
                      servers: state.servers,
                      onPick: widget.onPick,
                    ),
                  ),
          ),
          if (state.isEmpty) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            Text(
              l10n.setupLlmDiscoverEmpty,
              key: const Key('setup-discover-empty'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg2),
            ),
          ],
        ],
      ),
    );
  }
}

/// Why the search could not run.
///
/// A refused network (`LLM_008`) is not a defect of a server but a boundary of
/// this product, and the card says so in the words of the daemon rather than
/// in a summary of them.
class _Failure extends StatelessWidget {
  const _Failure({required this.failure});

  final Diagnostic failure;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final (String, String) text = setupDiagnosticText(l10n, failure);
    return HDiagnosticCard(
      key: const Key('setup-discover-failure'),
      code: failure.code,
      severityLabel: setupSeverityLabel(l10n, failure.severity),
      color: setupSeverityColor(tokens, failure.severity),
      title: text.$1,
      why: text.$2,
      detail: text.$2 == failure.why ? null : failure.why,
      fix: FixControl(
        fix: failure.fix,
        copyKey: const Key('setup-fix-copy-discover'),
      ),
      docsUrl: failure.docsUrl,
      width: 460,
    );
  }
}

/// The hairline that says "this is working".
///
/// Not a percentage: the daemon streams answers, and a bar that filled itself
/// on a timer would be a claim nobody measured. A short segment sweeps across
/// the full width and starts again, so the movement carries no position.
class _Sweep extends StatelessWidget {
  const _Sweep({required this.controller, required this.running});

  final AnimationController controller;
  final bool running;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return SizedBox(
      height: 2,
      child: DecoratedBox(
        decoration: BoxDecoration(color: tokens.colors.line),
        child: running
            ? AnimatedBuilder(
                animation: controller,
                builder: (BuildContext context, Widget? child) =>
                    FractionallySizedBox(
                      widthFactor: 0.25,
                      alignment: Alignment(controller.value * 2 - 1, 0),
                      child: child,
                    ),
                child: DecoratedBox(
                  decoration: BoxDecoration(color: tokens.colors.accent),
                ),
              )
            : null,
      ),
    );
  }
}

/// The servers, in the order they answered.
class _Servers extends StatelessWidget {
  const _Servers({required this.servers, required this.onPick});

  final List<LlmServer> servers;
  final void Function(String endpoint) onPick;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return ConstrainedBox(
      constraints: const BoxConstraints(maxHeight: 260),
      child: SingleChildScrollView(
        child: Column(
          key: const Key('setup-discover-servers'),
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            for (final LlmServer server in servers) ...<Widget>[
              _ServerRow(
                key: Key('setup-discover-row-${server.host}-${server.port}'),
                server: server,
                onPick: onPick,
              ),
              SizedBox(height: tokens.spacing.x2),
            ],
          ],
        ),
      ),
    );
  }
}

/// One found server.
class _ServerRow extends StatefulWidget {
  const _ServerRow({required this.server, required this.onPick, super.key});

  final LlmServer server;
  final void Function(String endpoint) onPick;

  @override
  State<_ServerRow> createState() => _ServerRowState();
}

class _ServerRowState extends State<_ServerRow>
    with SingleTickerProviderStateMixin {
  late final AnimationController _arrive = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  )..forward();

  /// Einmal gebaut und einmal entsorgt. Eine Kurve je `build` hinge sich mit
  /// jedem Bild neu an den Controller.
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _arrive,
    curve: HMotion.enter,
  );

  @override
  void dispose() {
    _curve.dispose();
    _arrive.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final LlmServer server = widget.server;
    // Ein Bruchteil der Zeile und keine Pixelzahl: `SlideTransition` misst in
    // der Größe seines Kindes. Von oben, wie jede ankommende Zeile dieses
    // Produkts (`docs/UX.md` 2.2), und unter reduzierter Bewegung null.
    final double slide =
        HReducedMotion.distance(context, HMotion.arriveOffset) / HSize.row;
    return FadeTransition(
      opacity: _curve,
      child: SlideTransition(
        position: _curve.drive(
          Tween<Offset>(begin: Offset(0, -slide), end: Offset.zero),
        ),
        child: HPanel(
          child: Padding(
            padding: EdgeInsets.all(tokens.spacing.x3),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        server.endpoint,
                        style: tokens.typography.mono12.tinted(
                          tokens.colors.fg0,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      SizedBox(height: tokens.spacing.x1),
                      Wrap(
                        spacing: tokens.spacing.x2,
                        runSpacing: tokens.spacing.x1,
                        children: <Widget>[
                          HBadge(text: _product(server, l10n)),
                          if (server.latencyMs > 0)
                            HBadge(
                              text: l10n.setupLlmDiscoverLatency(
                                server.latencyMs,
                              ),
                            ),
                          // Ein Modellname aus dem Netz ist Text und nur Text.
                          for (final String model in server.shownModels)
                            HBadge(text: model),
                          if (server.hiddenModels > 0)
                            HBadge(
                              text: l10n.setupLlmMoreModels(
                                server.hiddenModels,
                              ),
                            ),
                        ],
                      ),
                    ],
                  ),
                ),
                SizedBox(width: tokens.spacing.x3),
                if (server.isUsable)
                  HButton(
                    key: Key(
                      'setup-discover-use-${server.host}-${server.port}',
                    ),
                    variant: HButtonVariant.secondary,
                    onPressed: () => widget.onPick(server.endpoint),
                    child: Text(l10n.setupLlmDiscoverUse),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  /// What the server is, in the words of the search.
  String _product(LlmServer server, AppLocalizations l10n) {
    if (server.authRequired) {
      return l10n.setupLlmDiscoverAuth;
    }
    return switch (server.flavor) {
      LlmFlavor.ollama => 'ollama',
      LlmFlavor.openAiCompatible => 'openai',
      LlmFlavor.unknown => l10n.setupLlmDiscoverUnidentified,
    };
  }
}
