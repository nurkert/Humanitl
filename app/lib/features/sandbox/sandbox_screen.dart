/// The Sandbox section: what the agent gets, and start and stop (HUM-040).
///
/// The person who opens this screen is asking one question -- does the agent
/// get my whole disk? -- and everything on it serves the answer: the sentence
/// says it, the mounts table proves it, the environment says what the agent
/// was told, and the command line at the bottom is the source all three were
/// read from. Nothing here is derived in the application; the daemon answers
/// and the screen shows the answer (ADR-018).
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/time/now.dart';
import '../../core/ui_state/dismissed_findings.dart';
import '../../core/ui/fix_control.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/h_notice_layer.dart';
import '../../core/ui/diagnostic_severity.dart';
import '../../core/ui/h_resizable_panes.dart';
import '../../core/ui/pane_split.dart';
import '../../core/ui/terminal_placement.dart';
import '../../core/ui/ui.dart';
import '../../core/ui/work_dir_picker.dart';
import '../../l10n/l10n.dart';
import 'providers/closed_failures.dart';
import 'providers/sandbox_status_provider.dart';
import 'sandbox_text.dart';
import 'widgets/argv_sheet.dart';
import 'widgets/arrive.dart';
import 'widgets/env_tab.dart';
import 'widgets/isolation_panel.dart';
import 'widgets/log_tab.dart';
import 'widgets/mounts_tab.dart';
import 'widgets/sandbox_header.dart';
import 'widgets/sandbox_tabs.dart';
import 'widgets/sandbox_terminal.dart';
import 'widgets/stop_dialog.dart';

/// How much of the height the terminal keeps.
///
/// The terminal is where the work happens, so it is the larger half even
/// while it is a placeholder: the panes must not move under the person once
/// HUM-042 fills it (CONVENTIONS 4.13, "Vorhersagbarkeit").
const double sandboxTerminalFraction = 0.6;

/// The split of terminal and tabs, remembered across restarts (HUM-305).
///
/// [sandboxTerminalFraction] is where it starts and where a double click on
/// the boundary puts it back.
const PaneSplitSpec sandboxPaneSplit = PaneSplitSpec(
  'paneSplit.sandbox',
  <double>[sandboxTerminalFraction, 1 - sandboxTerminalFraction],
);

/// The smallest height of the terminal pane.
///
/// Banner, a held strip and an exit strip can stand over the emulator at the
/// same time; at this height a few rows of it still show between them.
const double sandboxTerminalMinHeight = 160;

/// The smallest height of the pane with the tabs: the tab bar and about one
/// row below it. Whoever wants the terminal large wants the tabs small, and
/// the tab bar has to stay reachable (HUM-305).
const double sandboxDetailsMinHeight = 72;

/// Below this height the content of a tab scrolls as a whole instead of
/// being squeezed: the environment tab puts a search field above its list,
/// and a squeezed column would paint over its own edge.
const double sandboxDetailsContentFloor = 120;

/// How many skeleton rows the first load draws.
const int sandboxSkeletonRows = 6;

/// The Sandbox section.
class SandboxScreen extends ConsumerStatefulWidget {
  /// Creates the section.
  const SandboxScreen({super.key});

  @override
  ConsumerState<SandboxScreen> createState() => _SandboxScreenState();
}

class _SandboxScreenState extends ConsumerState<SandboxScreen> {
  bool _visible = false;
  bool _asking = false;
  bool _argvOpen = false;

  /// True for the one build in which the terminal comes back from the whole
  /// window (HUM-298).
  ///
  /// One shot: the pane mounts the terminal in that build and hands it the
  /// keyboard; the flag is cleared after the frame, so a later remount or a
  /// session that starts later does not take the focus from wherever it is.
  bool _terminalCameBack = false;

  /// True while the shell actually paints this section. The shell keeps every
  /// section built inside an `IndexedStack`, which wraps each child in a
  /// `Visibility`; reading it here keeps the check inside this feature
  /// (ARCHITECTURE 5).
  bool _isVisible(BuildContext context) => Visibility.of(context);

  void _askStop() => setState(() => _asking = true);

  void _cancelStop() => setState(() => _asking = false);

  void _confirmStop() {
    setState(() => _asking = false);
    unawaited(ref.read(sandboxStatusProvider.notifier).stop());
  }

  void _showArgv() => setState(() => _argvOpen = true);

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool visible = _isVisible(context);
    if (visible && !_visible) {
      // A session may have started next door -- from the command line, or in
      // a second window. One call catches up with all of it.
      WidgetsBinding.instance.addPostFrameCallback(
        (Duration _) => ref.read(sandboxStatusProvider.notifier).refresh(),
      );
    }
    _visible = visible;
    ref.listen<TerminalPlacement>(terminalPlacementProvider, (
      TerminalPlacement? previous,
      TerminalPlacement next,
    ) {
      if (next == TerminalPlacement.pane && previous != next) {
        setState(() => _terminalCameBack = true);
      }
    });

    if (_terminalCameBack) {
      WidgetsBinding.instance.addPostFrameCallback(
        (Duration _) => _terminalCameBack = false,
      );
    }

    final AsyncValue<SandboxStatus> snapshot = ref.watch(sandboxStatusProvider);
    final SandboxStatus status = snapshot.value ?? const SandboxStatus();
    return ColoredBox(
      color: tokens.colors.bg0,
      child: Stack(
        fit: StackFit.expand,
        children: <Widget>[
          // Der `LayoutBuilder` steht **über** der Spalte und nicht darin:
          // Eine vertikale Spalte reicht ihren Kindern `maxHeight:
          // double.infinity`, und ein Deckel, der daraus ein Drittel rechnet,
          // ist wieder unendlich — also keiner. Genau das ist im Review von
          // Antigravity aufgefallen, nachdem der erste Entwurf den Deckel
          // innen hatte und der Bildschirm trotzdem übergelaufen wäre.
          LayoutBuilder(
            builder: (BuildContext context, BoxConstraints constraints) =>
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    SandboxHeader(status: status, onAskStop: _askStop),
                    // Befunde schweben oben rechts über Terminal und
                    // Reitern, direkt unter dem Kopf mit dem Startknopf, an
                    // dem sie entstanden sind (HUM-302). Sie nehmen dem
                    // Terminal keine Zeile: Als Band über ihm ließen sie es
                    // bei jedem Kommen und Gehen schrumpfen und wachsen, und
                    // die Oberfläche des Agenten darin brach neu um. Über
                    // einem Drittel der Höhe rollen sie in sich (HUM-068).
                    Expanded(
                      child: HNoticeLayer(
                        maxHeight: constraints.maxHeight * _diagnosticsShare,
                        // Und nie über die unteren Zeilen des Terminals: Dort
                        // tippt der Mensch, auch wenn das Terminal auf seine
                        // kleinste Höhe gezogen ist (HUM-302, Review).
                        maxHeightFor: (double region) => _noticeRoomAbove(
                          context,
                          region,
                          ref.read(paneSplitProvider(sandboxPaneSplit)),
                        ),
                        notices: _Diagnostics(status: status),
                        child: switch (snapshot) {
                          AsyncError(:final Object error) => _CannotRead(
                            error: error,
                          ),
                          _ => HWait(
                            loading: snapshot.isLoading && !snapshot.hasValue,
                            skeleton: Padding(
                              padding: EdgeInsets.all(tokens.spacing.x3),
                              child: HSkeleton(
                                rows: sandboxSkeletonRows,
                                rowHeight: tokens.sizes.rowHistory,
                              ),
                            ),
                            child: _Body(
                              status: status,
                              onShowArgv: _showArgv,
                              cameBack: _terminalCameBack,
                            ),
                          ),
                        },
                      ),
                    ),
                    const HHairline(),
                    SandboxStatusBar(status: status, onShowArgv: _showArgv),
                  ],
                ),
          ),
          if (_argvOpen)
            Align(
              alignment: Alignment.centerRight,
              child: ArgvSheet(
                argv: status.argvPreview,
                onClose: () => setState(() => _argvOpen = false),
              ),
            ),
          if (_asking)
            StopDialog(onCancel: _cancelStop, onConfirm: _confirmStop),
        ],
      ),
    );
  }
}

/// The terminal above, the four tabs below, and a boundary between them
/// that can be dragged (HUM-305).
class _Body extends ConsumerWidget {
  const _Body({
    required this.status,
    required this.onShowArgv,
    required this.cameBack,
  });

  final SandboxStatus status;

  /// True once the terminal has come back from the whole window: then it
  /// takes the keyboard again when its pane shows it (HUM-298).
  final bool cameBack;

  /// Opens the command the daemon built; the isolation tab leads there too.
  final VoidCallback onShowArgv;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    // Steht das Terminal woanders (über dem ganzen Fenster, HUM-298), bleibt
    // sein Platz hier leer, in der Farbe des Terminals: Zwei Ansichten
    // derselben Sitzung würden beide ihre Größe melden und beide Tasten
    // nehmen. Die Sitzung selbst und ihr Rücklauf leben im Provider und
    // überstehen den Wechsel.
    final bool here =
        ref.watch(terminalPlacementProvider) == TerminalPlacement.pane;
    final PaneSplit split = ref.read(
      paneSplitProvider(sandboxPaneSplit).notifier,
    );
    // Terminal oben, Reiter unten, und die Naht dazwischen lässt sich ziehen
    // (HUM-305): Wer das Terminal groß will, zieht sie nach unten, und die
    // Reiter rollen in sich, statt Platz zu beanspruchen. Die Größe des
    // Terminals folgt dem Zug über den Weg, den jede Größenänderung nimmt:
    // `TerminalView` misst sich neu, und der Emulator meldet Spalten und
    // Zeilen an den Daemon.
    return HResizablePanes(
      axis: Axis.vertical,
      ratios: ref.watch(paneSplitProvider(sandboxPaneSplit)),
      minWidths: const <double>[
        sandboxTerminalMinHeight,
        sandboxDetailsMinHeight,
      ],
      onRatiosChanged: split.set,
      onDragEnd: split.commit,
      onReset: split.reset,
      splitterSemanticsLabel: l10n.sandboxSplitter,
      children: <Widget>[
        if (here)
          SandboxTerminal(focusOnMount: cameBack)
        else
          const _TerminalElsewhere(),
        _Details(status: status, onShowArgv: onShowArgv),
      ],
    );
  }
}

/// Where the terminal stands while another host shows it: an empty area in
/// the colour of the terminal, and while it floats a button that puts it
/// back here (HUM-300). Over the whole window nobody sees this pane.
class _TerminalElsewhere extends ConsumerWidget {
  const _TerminalElsewhere();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final bool floating =
        ref.watch(terminalPlacementProvider) == TerminalPlacement.floating;
    return ColoredBox(
      key: const Key('sandbox-terminal-elsewhere'),
      color: tokens.terminal.background,
      child: floating
          ? Center(
              child: HButton(
                key: const Key('sandbox-terminal-dock'),
                variant: HButtonVariant.secondary,
                size: HButtonSize.sm,
                onPressed: ref.read(terminalPlacementProvider.notifier).dock,
                child: Text(context.l10n.sandboxTerminalDockHere),
              ),
            )
          : null,
    );
  }
}

/// The tab bar and the open tab: the lower pane of the section.
class _Details extends ConsumerWidget {
  const _Details({required this.status, required this.onShowArgv});

  final SandboxStatus status;
  final VoidCallback onShowArgv;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final SandboxTab tab = ref.watch(sandboxTabChoiceProvider);
    final Widget content = switch (tab) {
      SandboxTab.mounts => MountsTab(status: status),
      SandboxTab.env => EnvTab(status: status),
      SandboxTab.isolation => IsolationPanel(
        status: status,
        onShowArgv: onShowArgv,
      ),
      SandboxTab.log => const LogTab(),
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SandboxTabs<SandboxTab>(
          selected: tab,
          onSelect: ref.read(sandboxTabChoiceProvider.notifier).go,
          entries: <SandboxTabEntry<SandboxTab>>[
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.mounts,
              label: l10n.sandboxTabMounts,
              key: const Key('sandbox-tab-mounts'),
            ),
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.env,
              label: l10n.sandboxTabEnv,
              key: const Key('sandbox-tab-env'),
            ),
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.isolation,
              label: l10n.sandboxTabIsolation,
              key: const Key('sandbox-tab-isolation'),
            ),
            SandboxTabEntry<SandboxTab>(
              value: SandboxTab.log,
              label: l10n.sandboxTabLog,
              key: const Key('sandbox-tab-log'),
            ),
          ],
        ),
        const HHairline(),
        Expanded(
          child: LayoutBuilder(
            builder: (BuildContext context, BoxConstraints constraints) =>
                constraints.maxHeight >= sandboxDetailsContentFloor
                ? content
                : SingleChildScrollView(
                    key: const Key('sandbox-details-scroll'),
                    child: SizedBox(
                      height: sandboxDetailsContentFloor,
                      child: content,
                    ),
                  ),
          ),
        ),
      ],
    );
  }
}

/// Wie viele Zeilen des Terminals unten frei bleiben, wenn Befunde darüber
/// schweben: die Eingabezeile und die eine darüber (HUM-302, Review).
const int sandboxTerminalFreeRows = 2;

/// Die Zeilenhöhe des Emulators: `TerminalStyle` rechnet die Schriftgröße
/// mal 1,2.
const double _terminalLineFactor = 1.2;

/// Wie hoch die Befunde über dem Terminal höchstens werden: die Höhe, die
/// das Terminal in einer Region von [region] Pixeln bei [ratios] bekommt,
/// abzüglich der [sandboxTerminalFreeRows] unteren Zeilen, des Innenabstands
/// des Emulators und des Abstands der Hinweise zur Oberkante.
double _noticeRoomAbove(
  BuildContext context,
  double region,
  List<double> ratios,
) {
  final HTokens tokens = HTheme.of(context);
  final double terminal = snapPaneWidths(
    resolvePaneWidths(
      math.max(0, region - HSize.border),
      ratios,
      const <double>[sandboxTerminalMinHeight, sandboxDetailsMinHeight],
    ),
  ).first;
  final double row = MediaQuery.textScalerOf(context)
      .scale((tokens.typography.mono13.fontSize ?? 13) * _terminalLineFactor);
  return terminal -
      sandboxTerminalFreeRows * row -
      tokens.spacing.x2 -
      tokens.spacing.x2;
}

/// Wie viel Höhe die Befunde höchstens einnehmen, bevor sie in sich rollen.
///
/// Ein Drittel: genug für zwei Karten nebeneinanderliegender Ursachen, wenig
/// genug, dass vom Terminal darunter das meiste sichtbar bleibt.
const double _diagnosticsShare = 1 / 3;

/// Every finding of the last operation, newest last, floating at the top
/// right right under the header (HUM-302).
///
/// A finding is anchored where the action was: the start button is above it,
/// and the reason it did not start stands directly below (`docs/UX.md` 4.4).
/// Each card closes on its own. A finding that stands stays closed while its
/// code and cause stay the same (`dismissedFindingsProvider`); a failure of
/// the last operation and a failed folder dialog stay closed only until the
/// next operation (`closedFailuresProvider`).
class _Diagnostics extends ConsumerWidget {
  const _Diagnostics({required this.status});

  final SandboxStatus status;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Ein Ordnerdialog, der nicht aufging, steht hier mit den Befunden des
    // Daemons (`UI_003`, HUM-282): Der Knopf im Kopf hat dann nichts getan,
    // und ohne Karte wüsste niemand, warum und wie es anders geht. Nur
    // solange der Ordner wählbar ist: Läuft eine Sitzung, etwa über
    // `humanitl run`, ist der Ausweg schon genommen, und die Karte stünde
    // nur im Weg.
    final bool locked = status.isUp || status.isBusy;
    final Diagnostic? dialogFailure = locked
        ? null
        : ref.watch(folderDialogFailureProvider);
    final List<Diagnostic> diagnostics = <Diagnostic>[
      ...status.diagnostics,
      ?dialogFailure,
    ];
    // Zwei Arten zu schließen (HUM-302, Review). Ein Befund, der steht,
    // ist für seinen Gegenstand geschlossen, nicht für die eine Antwort:
    // `LLM_006` kommt mit jeder Abfrage des Status wieder, solange der
    // Endpunkt derselbe ist. Ein Fehlschlag der letzten Operation und ein
    // Ordnerdialog, der nicht aufging, sind Ereignisse: geschlossen bis zur
    // nächsten Operation, damit ein Start, der wieder scheitert, das sagt.
    ref.watch(dismissedFindingsProvider);
    ref.watch(closedFailuresProvider);
    final DismissedFindings standing = ref.read(
      dismissedFindingsProvider.notifier,
    );
    final ClosedFailures events = ref.read(closedFailuresProvider.notifier);
    bool hidden(Diagnostic diagnostic) => identical(diagnostic, dialogFailure)
        ? events.coversDialog(diagnostic)
        : isOperationFailure(diagnostic)
        ? events.coversFailure(diagnostic)
        : standing.covers(diagnostic);
    void close(Diagnostic diagnostic) => identical(diagnostic, dialogFailure)
        ? events.closeDialog(diagnostic)
        : isOperationFailure(diagnostic)
        ? events.closeFailure(diagnostic)
        : standing.dismiss(diagnostic);
    if (diagnostics.every(hidden)) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Die Abhilfe eines abgelehnten Projektordners ist ein anderer Ordner
    // (`SANDBOX_006`, HUM-262). Der Knopf öffnet dieselbe Auswahl wie der
    // Picker im Kopf und ist gesperrt wie er: Solange die Sandbox läuft oder
    // ein Start oder Stopp unterwegs ist, ändert sich der Ordner nicht.
    final VoidCallback? chooseFolder = locked
        ? null
        : () => unawaited(
            chooseWorkDir(
              ref,
              (String dir) => unawaited(
                ref.read(sandboxStatusProvider.notifier).chooseWorkDir(dir),
              ),
            ),
          );
    return HNoticeColumn(
      children: <Widget>[
        // Der Schlüssel trägt den Platz **und** den Code: Zwei Befunde
        // desselben Codes in einer Sitzung sind kein Sonderfall — zwei
        // verbotene Mounts, zwei Symlinks aus dem Projekt heraus —, und ein
        // Schlüssel aus dem Code allein ließ den Bildschirm mit „Duplicate
        // keys" abstürzen (gefunden beim Bau des Deckels, HUM-068). Der Platz
        // ist der in der ganzen Liste, auch wenn davor einer geschlossen ist.
        for (final (int place, Diagnostic diagnostic) in diagnostics.indexed)
          if (!hidden(diagnostic))
            if (DiagnosticL10n.resolve(diagnostic, l10n)
                case final DiagnosticText text)
              SandboxArrive(
                key: ValueKey<String>('diagnostic-$place-${diagnostic.code}'),
                child: HDiagnosticCard(
                  code: diagnostic.code,
                  severityLabel: severityLabel(l10n, diagnostic.severity),
                  color: severityColor(tokens, diagnostic.severity),
                  title: text.title,
                  // The daemon's own sentence. The application writes the
                  // title, never the reason (`docs/UX.md` 4.4). A folder
                  // dialog that failed is built by this client: its reason
                  // is the translated sentence with the way around, and the
                  // platform's raw text stands under it, as on the setup card
                  // (HUM-282).
                  why: identical(diagnostic, dialogFailure)
                      ? text.why
                      : text.cause,
                  detail: identical(diagnostic, dialogFailure)
                      ? diagnostic.why
                      : null,
                  docsUrl: diagnostic.docsUrl,
                  fix: FixControl(
                    fix: diagnostic.fix,
                    onChooseFolder: chooseFolder,
                    chooseFolderLocked: locked
                        ? l10n.sandboxWorkDirLocked
                        : null,
                    diagnosticCode: diagnostic.code,
                  ),
                  width: double.infinity,
                  dense: true,
                  dismiss: HNoticeDismiss(
                    label: l10n.sandboxDiagnosticDismiss,
                    buttonKey: ValueKey<String>(
                      'sandbox-diagnostic-dismiss-$place-${diagnostic.code}',
                    ),
                    onPressed: () => close(diagnostic),
                  ),
                ),
              ),
      ],
    );
  }
}

/// The status bar of the section: session, uptime, and the way to the proof.
class SandboxStatusBar extends StatelessWidget {
  /// Creates the bar for [status]. [onShowArgv] opens the command sheet.
  const SandboxStatusBar({
    required this.status,
    required this.onShowArgv,
    super.key,
  });

  /// What the daemon last said.
  final SandboxStatus status;

  /// Opens the command sheet.
  final VoidCallback onShowArgv;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SessionId? session = status.sessionId;
    final DateTime? started = status.startedAt;
    return Container(
      constraints: const BoxConstraints(minHeight: HSize.statusBar),
      color: tokens.colors.bg1,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
      child: Row(
        children: <Widget>[
          Text(
            session == null
                ? l10n.sandboxSessionNone
                : l10n.sandboxSession(session.short),
            style: tokens.typography.mono11.tinted(tokens.colors.fg2),
          ),
          if (started != null) ...<Widget>[
            SizedBox(width: tokens.spacing.x3),
            _Uptime(startedAt: started),
          ],
          const Spacer(),
          HButton(
            key: const Key('sandbox-show-argv'),
            variant: HButtonVariant.ghost,
            size: HButtonSize.sm,
            onPressed: onShowArgv,
            child: Text(l10n.sandboxShowArgv),
          ),
        ],
      ),
    );
  }
}

/// How long the sandbox has been up, counted in whole seconds.
///
/// Its own widget, so the second that ticks rebuilds this text and nothing
/// else (`docs/UX.md` 7). The digits jump; they never roll (2.9).
///
/// # Die Sekunde kommt aus der Uhr des Programms, nie aus einem eigenen Timer
///
/// [nowProvider] ist die eine UI-Uhr, und ein eigener `Timer.periodic` neben
/// ihr wäre nicht nur einer zu viel, sondern der falsche: Bricht die
/// Verbindung, hängt die Shell `FrozenSections` über diesen Abschnitt und
/// ersetzt darin genau diesen Provider durch einen, der stehen bleibt. Der
/// Abschnitt wird dabei umgehängt und nicht zerstört (die `GlobalKey`s in
/// `ShellScreen._sections`), also überlebte ein privater Timer den Bruch und
/// zählte eine Laufzeit weiter, während das Banner darüber sagt, dass alles
/// darunter ein Schnappschuss ist (`docs/UX.md` 4.2, Fall 4). Eine Laufzeit,
/// die in einem eingefrorenen Bild weiterläuft, behauptet eine Messung, die
/// niemand gemacht hat (CONVENTIONS 4.13).
///
/// Beobachtet wird über ein `.select` auf den fertigen Text: Die Uhr
/// veröffentlicht alle 250 ms, die Anzeige ändert sich einmal je Sekunde, und
/// nur diese Änderung baut das Widget neu (`docs/UX.md` 7, Gatter „eine
/// Sekunde Uhr").
class _Uptime extends ConsumerWidget {
  const _Uptime({required this.startedAt});

  final DateTime startedAt;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String uptime = ref.watch(
      nowProvider.select(
        (DateTime now) => sandboxUptimeText(now.difference(startedAt)),
      ),
    );
    return Text(
      l10n.sandboxUptime(uptime),
      key: const Key('sandbox-uptime'),
      style: tokens.typography.mono11.tinted(tokens.colors.fg2),
    );
  }
}

/// The daemon did not answer. Its sentence, and the way to ask again.
class _CannotRead extends ConsumerWidget {
  const _CannotRead({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String why = error is DaemonException
        ? (error as DaemonException).diagnostic.why
        : '$error';
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            l10n.sandboxCannotRead(why),
            style: tokens.typography.ui13.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x3),
          HButton(
            key: const Key('sandbox-retry'),
            variant: HButtonVariant.secondary,
            onPressed: () =>
                unawaited(ref.read(sandboxStatusProvider.notifier).refresh()),
            child: Text(l10n.sandboxRetry),
          ),
        ],
      ),
    );
  }
}
