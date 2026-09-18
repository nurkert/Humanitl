/// Der Entwurf im Speicher, je Flow (HUM-047).
///
/// `draftProvider(FlowId)` ist der kanonische Name aus `backlog/CONVENTIONS.md`
/// 3.9. Er hält keine Fachlogik: Jede Änderung geht durch die reinen Funktionen
/// in `model/draft_ops.dart`, und dieser Notifier legt nur das Ergebnis ab.
///
/// # Warum der Entwurf hereingereicht und nicht geholt wird
///
/// Ein Feature liest die Provider eines anderen nicht (`docs/ARCHITECTURE.md`
/// 5). Die gehaltene Anfrage, ihr Detail und ihr Rumpf liegen in der
/// Warteschlange (`features/intercept`), die Historie hat dieselben Daten aus
/// einem anderen Provider, und der Editor darf keinen von beiden kennen.
/// Deshalb baut der Aufrufer eine [DraftSource] und ruft [DraftNotifier.load];
/// derselbe Schnitt, den `core/body/body_providers.dart` aus demselben Grund
/// schon macht.
///
/// # Warum `load` einen bestehenden Entwurf nicht überschreibt
///
/// `Esc` schließt den Editor und lässt den Entwurf stehen; wer ihn wieder
/// öffnet, findet seine Ersetzungen vor. Der Bildschirm ruft [load] bei jedem
/// Aufbau, und ein `load`, das jedes Mal von vorn begänne, verlöre genau das.
/// Neu geladen wird nur auf ausdrückliches [reset].
library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart'
    show ProviderSubscription;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/body/body_span.dart';
import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/flow_events.dart';
import '../model/draft.dart';
import '../model/draft_ops.dart' as ops;
import '../model/pseudonym_naming.dart';
import 'session_pseudonyms.dart';

part 'draft_provider.g.dart';

/// Alles, woraus ein Entwurf entsteht.
///
/// Der Aufrufer hat diese Dinge schon: die Zeile aus seiner Warteschlange, das
/// Detail aus seinem Provider, den zerlegten Rumpf aus `core/body`.
class DraftSource {
  /// Baut eine Quelle.
  const DraftSource({
    required this.request,
    required this.findings,
    required this.bodyText,
    required this.bodyKind,
    this.bodyBytes,
    this.aliases = const <String, String>{},
    this.session,
  });

  /// Die Sitzung, zu der die Anfrage gehört.
  ///
  /// Die Zähler der Pseudonyme gelten je Sitzung (`sessionPseudonymsProvider`);
  /// eine neue Sitzung beginnt wieder bei `<EMAIL_1>` und erbt keine Zuordnung.
  final SessionId? session;

  /// Die gehaltene Anfrage: Methode, Ziel, Pfad und Kopfzeilen.
  final HttpRequest request;

  /// Die Funde des `Analyzed`-Ereignisses, in ihrer Reihenfolge.
  final List<Finding> findings;

  /// Der Rumpf als Text, schon ausgepackt und bereinigt.
  final String bodyText;

  /// Was in dem Rumpf steht.
  final BodyKind bodyKind;

  /// Die Bytes, in denen die Funde des Rumpfes liegen.
  ///
  /// Null, wenn der Rumpf nicht als Text vorliegt; dann bekommt kein Fund des
  /// Rumpfes eine Stelle.
  final Uint8List? bodyBytes;

  /// Die Aliasse aus `findings.user_terms`, nach Begriff.
  final Map<String, String> aliases;
}

/// Der Entwurf zu einem Flow, oder null, solange keiner geladen wurde.
///
/// # Wann der Entwurf verschwindet
///
/// Jede [Replacement] trägt den Originalwert im Klartext — genau den Wert, den
/// der Mensch gerade entfernt hat. Der Provider ist `keepAlive`, also räumt ihn
/// niemand von allein weg. Er räumt sich deshalb **selbst** weg, sobald sein
/// Fluss `Recorded` ist: Der Horcher steht hier und nicht im Editor, weil der
/// Editor nach dem Senden oder nach `Esc` längst geschlossen ist, wenn
/// `Recorded` kommt — erst nach der Antwort des Ziels. Ein Horcher im Widget
/// hörte dann nichts mehr, und der Klartext bliebe bis zum Ende der Anwendung
/// liegen.
///
/// Der Horcher hängt am Container und nicht an `ref.listen`: Riverpod 3 hält
/// die `ref.listen`-Abos eines Providers an, an dem niemand mehr horcht — und
/// nach dem Schließen des Editors horcht an diesem Entwurf niemand mehr. Ist
/// sein Abo der einzige Horcher am Ereignisstrom, ruht dann auch der Strom, und
/// `Recorded` käme nie an. Ein Container-Abo zählt immer als aktiver Horcher.
///
/// Das Abo besteht nur, solange ein Entwurf steht: [load] und [reset] legen es
/// an, das Wegräumen schließt es, `ref.onDispose` ebenso. Sonst hielte jeder je
/// geöffnete Entwurf einen Horcher, und jedes Ereignis liefe durch alle.
///
/// # Wenn `Recorded` verlorengeht
///
/// Eine Lücke beim Wiederverbinden oder ein Neustart des Daemons kann genau
/// dieses eine Ereignis verschlucken. Der Strom meldet jede solche Lücke mit
/// `Lagged` (`flowEventsProvider`); darauf fragt der Entwurf mit `GetFlow` nach
/// seinem Fluss. Kennt der Daemon ihn nicht mehr (`IPC_003`, nach einem
/// Neustart) oder ist er `recorded` oder `failed`, kommt kein `Recorded` mehr,
/// und der Entwurf räumt sich weg. Ein Fluss dazwischen — entschieden, unterwegs
/// oder beantwortet — behält ihn: Sein `Recorded` steht noch aus, und ein
/// abgelaufener Entwurf bleibt lesbar, solange der Editor ihn zeigt.
@Riverpod(keepAlive: true)
class DraftNotifier extends _$DraftNotifier {
  SessionId? _session;
  ProviderSubscription<AsyncValue<FlowEvent>>? _events;

  @override
  Draft? build(FlowId id) {
    ref.onDispose(_stopListening);
    return null;
  }

  /// Legt den Entwurf an, falls noch keiner steht.
  void load(DraftSource source) {
    if (state != null) {
      return;
    }
    _session = source.session;
    state = buildDraft(id, source);
    _listen();
  }

  /// Wirft den Entwurf weg und baut ihn neu aus [source].
  void reset(DraftSource source) {
    _session = source.session;
    state = buildDraft(id, source);
    _listen();
  }

  void _listen() {
    _events ??= ref.container.listen<AsyncValue<FlowEvent>>(
      flowEventsProvider,
      (AsyncValue<FlowEvent>? previous, AsyncValue<FlowEvent> next) {
        final FlowEvent? event = next.value;
        if (event is FlowEventRecorded && event.flowId == id) {
          _discard();
        } else if (event is FlowEventLagged) {
          unawaited(_discardIfSettled());
        }
      },
    );
  }

  void _stopListening() {
    _events?.close();
    _events = null;
  }

  /// Wirft den Entwurf samt seinen Originalwerten weg.
  void _discard() {
    state = null;
    _stopListening();
  }

  /// Nach einer Lücke: Wirft den Entwurf weg, wenn sein `Recorded` nicht mehr
  /// kommen kann.
  Future<void> _discardIfSettled() async {
    final FlowState flowState;
    try {
      flowState = (await ref.read(daemonClientProvider).getFlow(id))
          .summary
          .state;
    } on DaemonException catch (error) {
      // Ein Fluss, den der Daemon nicht kennt, ist nach einem Neustart weg.
      // Jeder andere Fehler sagt nichts über den Fluss; die nächste Lücke
      // fragt wieder.
      if (ref.mounted && error.diagnostic.code == DiagnosticCodes.flowNotHeld) {
        _discard();
      }
      return;
    }
    if (!ref.mounted) {
      return;
    }
    if (flowState == FlowState.recorded || flowState == FlowState.failed) {
      _discard();
    }
  }

  /// Ersetzt einen Fund durch [pseudonym].
  void replace(int index, String pseudonym) =>
      _map((Draft draft) => ops.replaceFinding(draft, index, pseudonym));

  /// Ersetzt jeden offenen Fund mit demselben Wert.
  void replaceAllOfValue(String valueHash, String pseudonym) =>
      _map((Draft draft) => ops.replaceAllOfValue(draft, valueHash, pseudonym));

  /// Ersetzt jeden offenen Fund; die Namen kommen aus dem Stand der Sitzung.
  void replaceAllOpen({
    Map<String, String> aliases = const <String, String>{},
  }) {
    final PseudonymNaming naming = _naming(aliases);
    _map((Draft draft) => ops.replaceAllOpen(draft, naming));
    _remember(naming);
  }

  /// Lässt einen Fund stehen.
  void ignore(int index) =>
      _map((Draft draft) => ops.ignoreFinding(draft, index));

  /// Pseudonymisiert eine Auswahl (`Ctrl+R`).
  void replaceSelection(
    DraftLocation location,
    int start,
    int end,
    String kindLabel, {
    Map<String, String> aliases = const <String, String>{},
  }) {
    final PseudonymNaming naming = _naming(aliases);
    _map(
      (Draft draft) =>
          ops.replaceSelection(draft, location, start, end, kindLabel, naming),
    );
    _remember(naming);
  }

  /// Übernimmt freie Eingabe im Rumpf.
  ///
  /// Der Entwurf wird `dirty`, und die Funde, die noch offen im Rumpf stehen,
  /// verlieren ihre Stelle nicht: Sie werden verschoben, soweit das eindeutig
  /// ist, und sonst ignoriert. Ein erneuter Scan über den getippten Text
  /// gehört zum Debounce des Bildschirms und nicht hierher.
  void setBody(String body) => _map(
    (Draft draft) => draft.copyWith(
      body: body,
      dirty: true,
      findings: _shifted(draft, body),
      replacements: _reglowed(draft, body),
    ),
  );

  /// Setzt Name und Wert einer Kopfzeile.
  ///
  /// Die Zeile bleibt editierbar, auch wenn jemand ihr den Namen einer
  /// gesperrten Kopfzeile gibt — sonst stünde eine Zeile da, die sich weder
  /// umbenennen noch löschen ließe. Stattdessen sperrt [ops.checkHeaders] das
  /// Senden und nennt die Zeile.
  void setHeader(int i, String name, String value) => _map((Draft draft) {
    if (i < 0 || i >= draft.headers.length || draft.headers[i].locked) {
      return draft;
    }
    final List<HeaderEntry> headers = List<HeaderEntry>.of(draft.headers);
    headers[i] = headers[i].copyWith(name: name, value: value);
    return draft.copyWith(headers: headers, dirty: true);
  });

  /// Hängt eine leere Kopfzeile an.
  void addHeader() => _map(
    (Draft draft) => draft.copyWith(
      headers: <HeaderEntry>[
        ...draft.headers,
        const HeaderEntry(name: '', value: ''),
      ],
      dirty: true,
    ),
  );

  /// Entfernt eine Kopfzeile, außer sie ist gesperrt.
  void removeHeader(int i) => _map((Draft draft) {
    if (i < 0 || i >= draft.headers.length || draft.headers[i].locked) {
      return draft;
    }
    final List<HeaderEntry> headers = List<HeaderEntry>.of(draft.headers)
      ..removeAt(i);
    return draft.copyWith(headers: headers, dirty: true);
  });

  /// Setzt Pfad und Query.
  void setPathAndQuery(String value) =>
      _map((Draft draft) => draft.copyWith(pathAndQuery: value, dirty: true));

  /// Setzt die Methode; sie steht immer in Großbuchstaben.
  void setMethod(String method) => _map(
    (Draft draft) => draft.copyWith(method: method.toUpperCase(), dirty: true),
  );

  void _map(Draft Function(Draft draft) change) {
    final Draft? draft = state;
    if (draft != null) {
      state = change(draft);
    }
  }

  /// Der Namensgeber der **Sitzung**, nicht der dieses Entwurfs.
  ///
  /// Zähler und Zuordnung liegen in `sessionPseudonymsProvider`, damit
  /// derselbe Wert in zwei gehaltenen Anfragen dasselbe Pseudonym bekommt und
  /// `<EMAIL_2>` nie neben einem zweiten `<EMAIL_1>` steht, der jemand anderen
  /// meint. Was der Entwurf selbst trägt (`pseudonyms`, `counters`), ist sein
  /// eigener Anteil daran; die Mapping-Leiste liest ihn.
  ///
  /// Die eigene Zuordnung des Entwurfs kommt dazu: Sie trägt die Schlüssel der
  /// Auswahlen aus `Ctrl+R`, die die Sitzung nie zu sehen bekommt (siehe
  /// [SessionPseudonyms.remember]).
  PseudonymNaming _naming(Map<String, String> aliases) => ref
      .read(sessionPseudonymsProvider.notifier)
      .naming(
        session: _session,
        aliases: aliases,
        local: state?.pseudonyms ?? const <String, String>{},
      );

  /// Übernimmt in die Sitzung, was [naming] vergeben hat.
  void _remember(PseudonymNaming naming) =>
      ref.read(sessionPseudonymsProvider.notifier).remember(_session, naming);

  /// Die Funde des Rumpfes nach freier Eingabe.
  ///
  /// Solange der getippte Text den Treffer noch genau so enthält, wandert der
  /// Fund mit; sonst wird er ignoriert. Eine Markierung auf einer Stelle, an
  /// der der Wert nicht mehr steht, ist schlimmer als keine — sie ersetzte
  /// beim nächsten Klick fremden Text (`core/body/body_span.dart` sagt
  /// dasselbe über Markierungen im falschen Byteraum).
  List<FindingView> _shifted(Draft draft, String body) => <FindingView>[
    for (final FindingView view in draft.findings)
      if (view.location.kind != FindingLocation.body)
        view
      else if (view.isOpen)
        _shiftOne(view, draft.body, body)
      else
        view,
  ];

  /// Der Diff-Glow nach freier Eingabe.
  ///
  /// Ein Glow steht auf Offsets, und ein Zeichen vor ihm verschiebt ihn. Bliebe
  /// er stehen, leuchtete nach einem Tastendruck Text, den niemand ersetzt hat
  /// — und die Stelle, die wirklich ersetzt wurde, leuchtete nicht mehr. Wo das
  /// Pseudonym noch genau einmal im Rumpf steht, wandert der Glow mit; sonst
  /// fällt er weg. Die Zuordnung selbst bleibt: Der Wert ist ersetzt, auch wenn
  /// niemand mehr sagen kann, wo.
  List<Replacement> _reglowed(Draft draft, String body) => <Replacement>[
    for (final Replacement done in draft.replacements)
      if (done.location.kind != FindingLocation.body)
        done
      else if (done.end <= body.length &&
          body.substring(done.start, done.end) == done.pseudonym)
        done
      else if (_onlyAt(body, done.pseudonym) >= 0)
        done.copyWith(
          start: _onlyAt(body, done.pseudonym),
          end: _onlyAt(body, done.pseudonym) + done.pseudonym.length,
        ),
  ];

  /// Die Stelle von [needle] in [text], wenn es genau eine gibt; sonst `-1`.
  int _onlyAt(String text, String needle) {
    if (needle.isEmpty) {
      return -1;
    }
    final int at = text.indexOf(needle);
    return at >= 0 && text.indexOf(needle, at + 1) < 0 ? at : -1;
  }

  FindingView _shiftOne(FindingView view, String before, String after) {
    if (view.start >= before.length || view.end > before.length) {
      return view.copyWith(status: FindingStatus.ignored);
    }
    final String needle = before.substring(view.start, view.end);
    if (needle.isEmpty) {
      return view.copyWith(status: FindingStatus.ignored);
    }
    final int at = after.indexOf(needle);
    if (at < 0 || after.indexOf(needle, at + 1) >= 0) {
      // Verschwunden oder mehrdeutig geworden: keine Stelle mehr.
      return view.copyWith(status: FindingStatus.ignored);
    }
    return view.copyWith(start: at, end: at + needle.length);
  }
}

/// Baut einen Entwurf aus [source]; die eine Stelle, an der Byte-Versätze in
/// Code-Unit-Versätze umgerechnet werden.
///
/// Für den Rumpf tut das `byteToCharOffsets` aus `core/body/body_span.dart`,
/// derselbe Dekodierer, der den Text auch zeichnet. Für Kopfzeilen und Query
/// rechnet dieselbe Funktion über deren UTF-8-Bytes: Der Daemon sucht dort
/// genauso in Bytes, und ein Umlaut im `user-agent` verschöbe sonst jede
/// Markierung dahinter.
Draft buildDraft(FlowId id, DraftSource source) {
  final HttpRequest request = source.request;
  final List<HeaderEntry> headers = <HeaderEntry>[
    for (final Header header in request.headers) HeaderEntry.of(header),
  ];
  final List<FindingView> findings = <FindingView>[];
  final List<BodyFinding> inBody = mapBodyFindings(
    source.bodyBytes ?? Uint8List(0),
    source.findings,
    place: source.bodyBytes != null,
  );
  final Map<int, BodyFinding> byIndex = <int, BodyFinding>{
    for (final BodyFinding finding in inBody) finding.index: finding,
  };
  for (int i = 0; i < source.findings.length; i++) {
    final Finding finding = source.findings[i];
    final DraftLocation location = finding.location == FindingLocation.header
        ? DraftLocation.header(
            finding.headerName,
            index: _headerOf(headers, finding),
          )
        : DraftLocation.of(finding);
    final ({int start, int end}) span = switch (finding.location) {
      FindingLocation.body => (
        start: byIndex[i]?.charStart ?? 0,
        end: byIndex[i]?.charEnd ?? 0,
      ),
      FindingLocation.header || FindingLocation.query => charSpanOfBytes(
        _textOf(location, headers, request.pathAndQuery),
        finding.spanStart,
        finding.spanEnd,
      ),
    };
    findings.add(
      FindingView(
        index: i,
        finding: finding,
        location: location,
        start: span.start,
        end: span.end,
        status: finding.resolved || span.end <= span.start
            ? FindingStatus.ignored
            : FindingStatus.open,
        valueHash: valueHashHex(finding),
        originalStart: span.start,
        originalEnd: span.end,
      ),
    );
  }
  return Draft(
    flowId: id,
    method: request.methodLabel,
    scheme: request.scheme,
    authority: request.authority,
    pathAndQuery: request.pathAndQuery,
    bodyKind: source.bodyKind,
    body: source.bodyText,
    headers: headers,
    findings: findings,
  );
}

/// Welche der gleichnamigen Kopfzeilen dieser Fund meint.
///
/// Der Daemon durchsucht jeden Wert einzeln, schickt aber nur den Namen mit
/// (`FindingLocation::Header(name)`). Bei mehreren `Set-Cookie` oder `Via` ist
/// der Ort damit auf der Leitung mehrdeutig, und geraten wird hier so gut es
/// geht: Genommen wird die erste gleichnamige, deren Wert den Bereich des
/// Fundes überhaupt enthalten **kann**. Nur wenn keine passt, fällt die Wahl
/// auf die erste; eine Ersetzung trifft dann zwar die falsche Zeile, aber
/// immer noch **genau eine** — und nicht, wie zuvor, alle.
int _headerOf(List<HeaderEntry> headers, Finding finding) {
  final String name = finding.headerName.toLowerCase();
  for (int i = 0; i < headers.length; i++) {
    if (headers[i].name.toLowerCase() != name) {
      continue;
    }
    if (utf8.encode(headers[i].value).length >= finding.spanEnd) {
      return i;
    }
  }
  return headers.indexWhere(
    (HeaderEntry entry) => entry.name.toLowerCase() == name,
  );
}

/// Der Text eines Ortes, bevor es einen [Draft] gibt.
String _textOf(
  DraftLocation location,
  List<HeaderEntry> headers,
  String pathAndQuery,
) => switch (location.kind) {
  FindingLocation.query =>
    pathAndQuery.contains('?')
        ? pathAndQuery.substring(pathAndQuery.indexOf('?') + 1)
        : '',
  // Über [DraftLocation.indexIn] und nicht über den Namen: Bei mehreren
  // gleichnamigen Kopfzeilen wäre der erste Wert der falsche, und der Bereich
  // des Fundes zeigte dann in einen Text, in dem er nie stand.
  FindingLocation.header => _valueAt(headers, location.indexIn(headers)),
  FindingLocation.body => '',
};

/// Der Wert an [at], oder ein leerer Text.
String _valueAt(List<HeaderEntry> headers, int at) =>
    at < 0 || at >= headers.length ? '' : headers[at].value;

/// Byte-Versätze in Code-Unit-Versätze über den UTF-8-Bytes von [text].
///
/// Für einen reinen ASCII-Wert — der Normalfall in einer Kopfzeile — sind
/// beide Zahlen gleich; für alles andere sind sie es nicht, und genau daran
/// verschiebt sich sonst jede Markierung hinter dem ersten Umlaut.
({int start, int end}) charSpanOfBytes(
  String text,
  int byteStart,
  int byteEnd,
) {
  final Uint8List bytes = Uint8List.fromList(utf8.encode(text));
  if (byteStart >= bytes.length) {
    return (start: 0, end: 0);
  }
  final int from = byteStart.clamp(0, bytes.length);
  final int to = byteEnd.clamp(from, bytes.length);
  final Map<int, int> chars = byteToCharOffsets(bytes, <int>[from, to]);
  return (start: chars[from] ?? 0, end: chars[to] ?? chars[from] ?? 0);
}
