/// Die Karte, mit der der Daemon einen Befund meldet (HUM-045, HUM-106).
///
/// Der Daemon erkennt einen abgelehnten TLS-Handschlag, ordnet ihn dem
/// Werkzeug zu, dem das Zertifikat fehlt, und sagt, welche Variable es braucht.
/// Bis zu diesem Issue endete das im Ereignisstrom. Ein Mensch sah eine
/// Anfrage scheitern und erfuhr den Grund nur im Terminal des Agenten.
///
/// Fünf Festlegungen, die aus dem folgen, was auf dieser Karte steht:
///
/// * **Der Satz gehört dem Daemon, der Titel der Anwendung.** Der `why`-Slot
///   trägt den Satz, den der Daemon geschrieben hat, unübersetzt und
///   unumformuliert; der Titel darüber ist nur der Rahmen (`docs/UX.md` 4.4).
/// * **Der Text ist fremder Text.** In ihm steckt Material aus dem Netz — ein
///   Hostname, der Fehlertext einer Zertifikatsprüfung. Er wird als reiner
///   Text gezeichnet und sonst nichts: kein Markdown, kein Verweis, keine
///   Spanne mit Erkenner. Bereinigt wird er an einer einzigen Stelle, in
///   `providers/diagnostics.dart`, mit derselben Bereinigung, die die
///   Rumpf-Ansichten benutzen.
/// * **Die Karte verspricht nichts, was sie nicht tut.** Für `SetEnv` steht
///   das Abzeichen und darunter die Zeile, die `export KEY=VALUE` in die
///   Zwischenablage legt. Ein Knopf „für die nächste Sitzung setzen" fehlt
///   bewusst: Er bräuchte `SetConfig`, und dieser RPC antwortet bis HUM-069
///   `unimplemented` (`docs/UX.md` 6, vierter weiter geltender Punkt;
///   `backlog/CONVENTIONS.md` 4.13).
/// * **Der Befehl wird gebaut, nicht interpoliert.** Der Wert kommt aus
///   derselben Leitung wie der Satz darüber, und ein Knopf legt ihn in eine
///   Shell. `core/ui/shell_command.dart` quotiert ihn und verweigert die
///   Zeile, wo sie nicht beweisbar genau eine Zuweisung wäre; dann steht dort
///   der Grund und kein Knopf.
/// * **Jede Karte geht einzeln weg.** Ausblenden ist die Antwort „gesehen"
///   und gilt nur für diesen einen Befund; ein zweiter Befund desselben Codes
///   ergibt eine neue Karte, weil zwei Befunde zwei Dinge sind.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/diagnostics.dart';

/// Die Befunde dieser Sitzung, über der Warteschlange.
///
/// Zeichnet gar nichts, solange es keinen gibt: Ein leerer Streifen nähme der
/// Warteschlange eine Zeile für etwas, das nicht da ist.
class DiagnosticStrip extends ConsumerStatefulWidget {
  /// Creates the strip.
  const DiagnosticStrip({super.key});

  @override
  ConsumerState<DiagnosticStrip> createState() => DiagnosticStripState();
}

/// Der Zustand des Streifens.
///
/// Öffentlich allein wegen [pendingArrivals]: Ob die Menge der offenen
/// Ankünfte mitwächst, ist von außen sonst nicht zu sehen, und genau das war
/// einmal ein Leck.
class DiagnosticStripState extends ConsumerState<DiagnosticStrip> {
  /// Die Befunde, die eingetroffen sind, seit dieser Streifen steht, und noch
  /// nicht gezeichnet wurden.
  ///
  /// Nur sie blenden ein. Eine Karte, die beim Scrollen wieder in den
  /// Ausschnitt kommt, ist keine Ankunft, und nichts bewegt sich unter einem
  /// lesenden Auge (`docs/UX.md` 2.8).
  ///
  /// Die Menge wird bei jeder Änderung auf die Einträge zurückgeschnitten, die
  /// es noch gibt. Sonst bliebe jede Id für immer liegen, die nie gezeichnet
  /// wurde — `ListView.builder` baut nur den Ausschnitt, und der Ringpuffer
  /// verdrängt Einträge, die niemand gesehen hat. Das wäre dasselbe Leck,
  /// gegen das [maxSessionDiagnostics] gebaut ist, eine Ebene höher.
  final Set<int> _fresh = <int>{};

  /// Wie viele Ankünfte noch auf ihr Einblenden warten.
  @visibleForTesting
  int get pendingArrivals => _fresh.length;

  @override
  Widget build(BuildContext context) {
    final List<SessionDiagnostic> found = ref.watch(diagnosticsProvider);
    ref.listen(diagnosticsProvider, (
      List<SessionDiagnostic>? previous,
      List<SessionDiagnostic> next,
    ) {
      final Set<int> before = <int>{
        for (final SessionDiagnostic entry in previous ?? const []) entry.id,
      };
      final Set<int> live = <int>{
        for (final SessionDiagnostic entry in next) entry.id,
      };
      for (final int id in live) {
        if (!before.contains(id)) {
          _fresh.add(id);
        }
      }
      _fresh.retainWhere(live.contains);
    });
    final int dropped = ref.read(diagnosticsProvider.notifier).dropped;
    // Die Verlustmeldung überlebt das Ausblenden der letzten Karte. Wer alles
    // weggeklickt hat, hat den Verlust nicht gesehen, und ihn dann verschwinden
    // zu lassen wäre wieder das stille Wegwerfen, das dieses Issue verhindert
    // (`backlog/CONVENTIONS.md` 4.13).
    if (found.isEmpty && dropped == 0) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return RepaintBoundary(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (dropped > 0)
            Padding(
              padding: EdgeInsets.fromLTRB(
                tokens.spacing.x3,
                tokens.spacing.x2,
                tokens.spacing.x3,
                0,
              ),
              child: Text(
                l10n.interceptDiagnosticDropped(dropped),
                key: const Key('intercept-diagnostic-dropped'),
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            ),
          // `ListView.builder` und nicht `Column` in einem
          // `SingleChildScrollView`: Bis zu [maxSessionDiagnostics] Karten
          // stünden dort, und jede von ihnen misst sich über `IntrinsicHeight`
          // in jedem Frame neu. Gebaut wird, was zu sehen ist.
          if (found.isNotEmpty)
            Flexible(
              child: ListView.builder(
                shrinkWrap: true,
                padding: EdgeInsets.zero,
                itemCount: found.length,
                itemBuilder: (BuildContext context, int index) {
                  final SessionDiagnostic entry = found[index];
                  return DiagnosticCard(
                    key: ValueKey<String>('intercept-diagnostic:${entry.id}'),
                    entry: entry,
                    animate: _fresh.contains(entry.id),
                    onShown: () => _fresh.remove(entry.id),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}

/// Ein Befund des Daemons.
class DiagnosticCard extends ConsumerWidget {
  /// Creates the card for [entry].
  const DiagnosticCard({
    required this.entry,
    this.animate = false,
    this.onShown,
    super.key,
  });

  /// Der Befund, für den die Karte steht.
  final SessionDiagnostic entry;

  /// Wahr, wenn dieser Befund gerade eingetroffen ist.
  final bool animate;

  /// Meldet dem Streifen, dass diese Karte gezeichnet wurde.
  final VoidCallback? onShown;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Diagnostic diagnostic = entry.diagnostic;
    final FixAction? fix = diagnostic.fix;
    return _DiagnosticArrival(
      animate: animate,
      onShown: onShown,
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x2),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            // `Expanded` statt der Vorgabebreite der Karte: Die Warteschlange
            // ist ab `HSize.paneMinQueue` schmal, und eine feste Breite von
            // 560 px liefe dort über den Rand.
            Expanded(
              child: HDiagnosticCard(
                code: diagnostic.code,
                severityLabel: severityLabel(l10n, diagnostic.severity),
                color: severityColor(tokens, diagnostic.severity),
                title: l10n.interceptDiagnosticTitle,
                why: diagnostic.why,
                // Kein leerer Slot: Ein Befund ohne Vorschlag bekommt keine
                // Zeile, die aussieht, als fehlte dort etwas.
                fix: fix == null
                    ? null
                    // Eigener Schlüssel je Karte: Zwei Befunde mit Vorschlag
                    // stehen im selben Streifen, und ein geteilter Schlüssel
                    // machte beide Knöpfe ununterscheidbar.
                    : FixControl(
                        fix: fix,
                        copyKey: ValueKey<String>(
                          'intercept-diagnostic-copy-${entry.id}',
                        ),
                      ),
                docsUrl: diagnostic.docsUrl,
              ),
            ),
            SizedBox(width: tokens.spacing.x2),
            HIconButton(
              key: ValueKey<String>('intercept-diagnostic-dismiss-${entry.id}'),
              glyph: HGlyph.close,
              semanticsLabel: l10n.interceptDiagnosticDismiss,
              onPressed: () =>
                  ref.read(diagnosticsProvider.notifier).dismiss(entry.id),
            ),
          ],
        ),
      ),
    );
  }
}

/// Das Einblenden eines Befundes: kein Weg, nur Deckkraft über
/// [HMotion.arrive] auf [HMotion.enter].
///
/// Die Bewegungstabelle führt den Fehler ohne Strecke (`docs/UX.md` 2.2): Ein
/// Befund kommt nicht von irgendwoher, er steht plötzlich da, und eine
/// Richtung würde etwas behaupten, das er nicht hat.
///
/// Zwei Dinge, die eine Animation in diesem Programm immer tut
/// (`docs/UX.md` 2.10 und 7):
///
/// * **Der Controller läuft mit [AnimationBehavior.preserve].** Der
///   Linux-Embedder meldet `disableAnimations`, und die Vorgabe skalierte die
///   180 Millisekunden auf neun. Was dann bliebe, wäre kein Einblenden mehr,
///   sondern ein Sprung mit einer Behauptung im Doc-Kommentar.
/// * **Der Wrapper verlässt den Baum, sobald er fertig ist.** Ein
///   `FadeTransition`, der stehen bleibt, kostet seine Schicht in jedem Frame,
///   für immer. Danach steht das Kind nackt da.
class _DiagnosticArrival extends StatefulWidget {
  const _DiagnosticArrival({
    required this.animate,
    required this.onShown,
    required this.child,
  });

  final bool animate;
  final VoidCallback? onShown;
  final Widget child;

  @override
  State<_DiagnosticArrival> createState() => _DiagnosticArrivalState();
}

class _DiagnosticArrivalState extends State<_DiagnosticArrival>
    with SingleTickerProviderStateMixin {
  /// Ob diese Karte einblendet, entschieden beim ersten Bau und danach nie
  /// wieder: Ein zweiter Durchlauf wäre keine Ankunft mehr.
  late final bool _animate = widget.animate;
  AnimationController? _controller;
  CurvedAnimation? _curve;
  bool _done = false;

  @override
  void initState() {
    super.initState();
    widget.onShown?.call();
    if (!_animate) {
      _done = true;
      return;
    }
    final AnimationController controller = AnimationController(
      vsync: this,
      duration: HMotion.arrive,
      // Ohne das kürzt Flutter auf fünf Prozent, sobald die Plattform
      // `disableAnimations` meldet (`docs/UX.md` 2.10).
      animationBehavior: AnimationBehavior.preserve,
    );
    _controller = controller;
    _curve = CurvedAnimation(parent: controller, curve: HMotion.enter);
    controller
      ..addStatusListener(_finish)
      ..forward();
  }

  void _finish(AnimationStatus status) {
    if (status == AnimationStatus.completed && mounted && !_done) {
      setState(() => _done = true);
    }
  }

  @override
  void dispose() {
    _curve?.dispose();
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final CurvedAnimation? curve = _curve;
    if (_done || curve == null) {
      return widget.child;
    }
    return FadeTransition(opacity: curve, child: widget.child);
  }
}
