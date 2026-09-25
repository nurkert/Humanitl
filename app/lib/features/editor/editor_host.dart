/// Die Naht, an der der Editor in einen Pane gehängt wird (HUM-047).
///
/// Der Bildschirm der Warteschlange zeigt den Editor an der Stelle seiner
/// Karte, darf ihn aber nicht kennen: Kein Feature importiert ein anderes
/// (`docs/ARCHITECTURE.md` 5, `tools/check-deps.sh`). Zusammengesetzt wird
/// deshalb in der Shell, die beide kennen darf, und sie reicht [buildEditorPane]
/// als `InspectorEditorBuilder` in den Bildschirm hinein.
///
/// Was der Editor braucht, holt er sich selbst — über `core/ipc` und
/// `core/body`, nie über den Provider eines anderen Features. Dass jedes
/// Feature sein Detail aus einem eigenen Provider liest, ist im Haus schon
/// entschieden (`core/body/body_providers.dart`: „Warteschlange und History
/// lesen ihr Detail aus je eigenem Provider").
///
/// # Drei Dinge, die hier und nicht im Editor stehen
///
/// 1. **Das Senden wartet auf die Antwort.** `decide` wirft eine
///    `DaemonException` — `IPC_003`, wenn die Frist abgelaufen ist, während
///    jemand tippte, `IPC_004` bei einem Rumpf über der Grenze. Ein
///    unbeobachtetes `unawaited` schlösse den Editor, ließe den Befund in
///    `Zone.handleUncaughtError` verschwinden, und der Mensch glaubte, seine
///    Anfrage sei draußen. Es ist nichts draußen. Geschlossen wird deshalb
///    erst nach einer Antwort, und ein Fehler bleibt im Editor stehen.
/// 2. **Der Editor schließt sich mit seinem Fluss.** Ist der Fluss
///    `Recorded`, gibt es nichts mehr zu bearbeiten. Den Entwurf selbst räumt
///    nicht dieser Wirt weg, sondern `DraftNotifier`: Der Wirt lebt nur,
///    solange der Editor offen ist, und nach dem Senden oder nach `Esc` ist er
///    es nicht mehr, wenn `Recorded` kommt.
/// 3. **Nach der Frist wird nicht mehr gesendet.** Der Editor bleibt lesbar,
///    der Knopf ist aus und sagt, warum (HUM-058 liefert das Banner dazu).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/body/body_parser.dart';
import '../../core/body/body_providers.dart';
import '../../core/domain/domain.dart';
import '../../core/ipc/client_providers.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/ipc/flow_events.dart';
import '../../l10n/l10n.dart';
import 'editor_screen.dart';
import 'model/draft.dart';
import 'providers/draft_provider.dart';
import 'providers/editor_detail.dart';

/// Baut den Editor für [flowId]; [onClose] schließt ihn wieder.
///
/// Die Signatur ist die des `InspectorEditorBuilder` der Warteschlange. Sie
/// führt nur Kerntypen, damit der Bildschirm dort sie kennen kann, ohne diese
/// Datei zu importieren. [replaceAll] kommt aus der Pause mit offenen Funden
/// („Pseudonymisieren", HUM-049).
Widget buildEditorPane(
  BuildContext context,
  FlowId flowId,
  VoidCallback onClose, {
  required bool replaceAll,
}) => EditorHost(flowId: flowId, onClose: onClose, replaceAll: replaceAll);

/// Der Editor samt allem, was er zum Aufbau braucht.
class EditorHost extends ConsumerStatefulWidget {
  /// Baut den Wirt.
  const EditorHost({
    required this.flowId,
    required this.onClose,
    this.replaceAll = false,
    super.key,
  });

  /// Der Fluss, der bearbeitet wird.
  final FlowId flowId;

  /// Schließt den Editor; der Entwurf bleibt stehen.
  final VoidCallback onClose;

  /// Wahr, wenn der Editor mit allen offenen Funden ersetzt aufgehen soll.
  final bool replaceAll;

  @override
  ConsumerState<EditorHost> createState() => _EditorHostState();
}

class _EditorHostState extends ConsumerState<EditorHost> {
  Diagnostic? _failure;
  bool _sending = false;
  bool _overdue = false;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    _watchTheFlow();
    final FlowDetail? detail = ref
        .watch(editorDetailProvider(widget.flowId))
        .value;
    final HttpRequest? request = detail?.request;
    if (detail == null || request == null) {
      // Solange das Detail unterwegs ist, steht nichts da. Einen leeren
      // Entwurf zu bauen hieße, eine Anfrage ohne Kopfzeilen und ohne Rumpf zu
      // zeigen, und der erste Tastendruck darin schriebe in eine Anfrage, die
      // es so nie gab.
      return const SizedBox.shrink();
    }
    final BodySource source = BodySource.of(
      request.body,
      headers: request.headers,
      findings: detail.findings,
    );
    final ParsedBody? body = ref.watch(parsedBodyProvider(source)).value;
    if (body == null) {
      return const SizedBox.shrink();
    }
    return EditorScreen(
      key: Key('editor-${widget.flowId.value}'),
      flowId: widget.flowId,
      source: DraftSource(
        request: request,
        findings: detail.findings,
        bodyText: body.text?.text ?? '',
        bodyKind: body.kind,
        bodyBytes: body.text == null ? null : body.bytes,
        // `findings.user_terms` steht in der Konfiguration des Daemons, und
        // die hat in dieser Anwendung noch keinen Leser: `GetConfig` bekommt
        // seinen Dart-Client mit dem Settings-Bildschirm (HUM-069). Bis dahin
        // bleibt der Alias-Weg von `PseudonymNaming` ungenutzt, und ein
        // Nutzerbegriff bekommt `<TERM_n>` statt seines Alias.
        aliases: const <String, String>{},
        session: detail.summary.sessionId,
      ),
      replaceAllOnOpen: widget.replaceAll,
      canSend: !_overdue && !_sending,
      sendDisabledReason: _overdue ? l10n.editorTimedOut : '',
      failure: _failure,
      onClose: widget.onClose,
      onSend: _send,
      onBlock: _block,
      sendRefused: detail.summary.sendRefusal != null,
    );
  }

  /// Räumt den Entwurf weg, sobald es nichts mehr zu bearbeiten gibt.
  ///
  /// Gehört zu diesem Fluss und zu keinem anderen. `TimedOut` schließt den
  /// Editor nicht — der Entwurf bleibt lesbar, bis jemand ihn verlässt —, aber
  /// er nimmt das Senden weg; `Recorded` heißt, der Fluss ist abgeschlossen,
  /// und was noch im Speicher liegt, liegt dort umsonst.
  void _watchTheFlow() {
    ref.listen<AsyncValue<FlowEvent>>(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      final FlowEvent? event = next.value;
      if (event == null) {
        return;
      }
      switch (event) {
        case FlowEventTimedOut(:final FlowId flowId)
            when flowId == widget.flowId:
          if (mounted && !_overdue) {
            setState(() => _overdue = true);
          }
        case FlowEventRecorded(:final FlowId flowId)
            when flowId == widget.flowId:
          // Nur schließen. Den Entwurf räumt `DraftNotifier` selbst weg: Er
          // hört auf `Recorded` auch dann, wenn dieser Editor längst
          // geschlossen ist — nach dem Senden und nach `Esc` ist er das, bevor
          // `Recorded` kommt.
          widget.onClose();
        case _:
          break;
      }
    });
  }

  Future<void> _send(EditedRequest edited, List<Replacement> replacements) =>
      _decide(Decision.allowEdited(request: edited));

  /// „Blockieren" aus der Pause mit offenen Funden (HUM-161).
  ///
  /// Derselbe Weg wie das Senden: Er wartet auf die Antwort, und ein Befund
  /// bleibt im Editor stehen.
  Future<void> _block() => _decide(const Decision.block());

  Future<void> _decide(Decision decision) async {
    if (_sending) {
      return;
    }
    setState(() {
      _sending = true;
      _failure = null;
    });
    try {
      await ref.read(daemonClientProvider).decide(widget.flowId, decision);
    } on DaemonException catch (error) {
      // Der Editor bleibt stehen und zeigt, warum nichts hinausging. Ihn zu
      // schließen hieße, eine Anfrage für gesendet auszugeben, die der Daemon
      // gerade abgelehnt hat (`docs/UX.md` 4.4).
      if (mounted) {
        setState(() {
          _sending = false;
          _failure = error.diagnostic;
        });
      }
      return;
    }
    if (mounted) {
      setState(() => _sending = false);
      widget.onClose();
    }
  }
}
