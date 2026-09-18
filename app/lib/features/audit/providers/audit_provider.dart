/// Was der Audit-Bildschirm vom Daemon hält: der Kopf der Kette, das Ergebnis
/// der Prüfung, die Seiten der Tabelle und der Export (HUM-051).
///
/// Alle vier hängen an denselben vier Aufrufen des Ports (`auditHead`,
/// `auditVerify`, `auditQuery`, `auditExport`); dieser Bildschirm rechnet
/// nichts selbst aus. Die Kette liegt beim Daemon, und eine Oberfläche, die
/// ihren Zustand aus Teilen zusammenreimte, behauptete etwas über eine Datei,
/// die sie nie gelesen hat (ADR-018, `backlog/CONVENTIONS.md` 4.13).
library;

import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `KeepAliveLink` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart' show KeepAliveLink;

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';

/// Der Kopf der Kette: Nummer, Hash, Zahl der Records und der Anker.
///
/// Billig genug, um beim Öffnen des Bildschirms zu laufen; er liest die Datei
/// nicht von vorn. Was er **nicht** beweist, beweist [auditVerifyProvider].
///
/// `autoDispose`: Jedes „Jetzt prüfen" ist eine neue Generation. Die alte
/// fällt, sobald die Karte sie nicht mehr ansieht, statt für die Lebenszeit der
/// Anwendung im Speicher zu bleiben; die aktuelle lebt, solange die Karte sie
/// beobachtet (Review 3).
final auditHeadProvider = FutureProvider.autoDispose.family<AuditHead, int>(
  (Ref ref, int generation) => ref.watch(daemonClientProvider).auditHead(),
);

/// Das Ergebnis der Prüfung der Kette.
///
/// Läuft beim Öffnen des Bildschirms einmal und danach auf Klick; das Ergebnis
/// bleibt stehen, bis der nächste Lauf es ersetzt. [generation] ist der
/// Zähler, den „Jetzt prüfen" hochsetzt — ein Provider, der sich bei jedem
/// Aufbau neu berechnete, prüfte eine lange Kette bei jedem Frame.
///
/// `autoDispose` aus demselben Grund wie [auditHeadProvider].
final auditVerifyProvider = FutureProvider.autoDispose.family<AuditReport, int>(
  (Ref ref, int generation) => ref.watch(daemonClientProvider).auditVerify(),
);

/// Der Zähler, der Kopf und Prüfung neu laufen lässt.
final NotifierProvider<AuditRunNotifier, int> auditRunProvider =
    NotifierProvider<AuditRunNotifier, int>(AuditRunNotifier.new);

/// Der Notifier hinter [auditRunProvider].
class AuditRunNotifier extends Notifier<int> {
  @override
  int build() => 0;

  /// Startet einen neuen Lauf von Kopf und Prüfung.
  void again() => state++;
}

/// Der Filter der Tabelle: Art, Sitzung, Zeitraum.
final NotifierProvider<AuditFilterNotifier, AuditFilter> auditFilterProvider =
    NotifierProvider<AuditFilterNotifier, AuditFilter>(AuditFilterNotifier.new);

/// Der Notifier hinter [auditFilterProvider].
class AuditFilterNotifier extends Notifier<AuditFilter> {
  @override
  AuditFilter build() => AuditFilter.none;

  /// Setzt den Präfix der Art; leer heißt jede Art.
  void setKindPrefix(String prefix) =>
      state = state.copyWith(kindPrefix: prefix);

  /// Setzt die Sitzung; leer heißt jede Sitzung.
  void setSession(String session) => state = state.copyWith(session: session);

  /// Setzt die untere Zeitgrenze; null nimmt sie weg.
  void setFrom(DateTime? from) => state = from == null
      ? state.copyWith(clearFrom: true)
      : state.copyWith(from: from);

  /// Setzt die obere Zeitgrenze; null nimmt sie weg.
  void setTo(DateTime? to) => state = to == null
      ? state.copyWith(clearTo: true)
      : state.copyWith(to: to);

  /// Nimmt jede Einschränkung weg.
  void clear() => state = AuditFilter.none;
}

/// Welche Zeitfelder gerade einen Text halten, der keine Zeit ist.
///
/// Solange eines darin steht, fängt kein Export an: Der Filter hält dann die
/// alte Grenze, das Feld zeigt eine andere, und eine Datei über den alten
/// Zeitraum wäre eine andere als die, die der Mensch vor sich sieht
/// (`backlog/CONVENTIONS.md` 4.13).
///
/// `autoDispose` und beobachtet von der Filterleiste, die die Felder trägt:
/// Die Menge lebt so lange wie die Felder, deren Zustand sie hält. Ohne das
/// bliebe die Marke eines Feldes stehen, das es nicht mehr gibt, und jeder
/// spätere Export lehnte ab, bis die Anwendung neu startet (Review 3).
final auditRangeInvalidProvider =
    NotifierProvider.autoDispose<AuditRangeInvalidNotifier, Set<String>>(
      AuditRangeInvalidNotifier.new,
    );

/// Der Notifier hinter [auditRangeInvalidProvider].
class AuditRangeInvalidNotifier extends Notifier<Set<String>> {
  @override
  Set<String> build() => const <String>{};

  /// Vermerkt, ob das Feld [id] gerade unlesbar ist.
  void mark(String id, {required bool invalid}) {
    if (state.contains(id) == invalid) {
      return;
    }
    state = invalid
        ? <String>{...state, id}
        : <String>{...state.where((String other) => other != id)};
  }
}

/// Wie viele Records eine Seite holt.
const int auditPageRows = auditPageSize;

/// Wie viele Records der Bildschirm höchstens auf einmal im Speicher hält.
///
/// Die Kette wächst ohne Grenze, das Fenster darauf nicht. Ist es voll, sagt
/// die Fußzeile das, statt still weiterzuladen — dasselbe Versprechen wie in
/// der History (`backlog/sprint-2.md`, HUM-032).
const int auditMaxRows = 2000;

/// Die geladenen Records, ihre Fortsetzung und was dabei schiefging.
@immutable
class AuditRecordsState {
  /// Legt einen Zustand an.
  const AuditRecordsState({
    this.rows = const <AuditRecordRow>[],
    this.cursor = '',
    this.total = 0,
    this.loading = false,
    this.loadingMore = false,
    this.failure,
  });

  /// Noch nichts geladen.
  static const AuditRecordsState empty = AuditRecordsState(loading: true);

  /// Die Records, jüngster zuerst.
  final List<AuditRecordRow> rows;

  /// Wo die nächste Seite beginnt; leer, wenn es keine gibt.
  final String cursor;

  /// Wie viele Records der Filter trifft, wie der Daemon sie gezählt hat.
  final int total;

  /// Wahr, solange die erste Seite unterwegs ist.
  final bool loading;

  /// Wahr, solange eine weitere Seite unterwegs ist.
  final bool loadingMore;

  /// Warum die letzte Abfrage scheiterte, oder null.
  final Diagnostic? failure;

  /// Wahr, wenn es eine weitere Seite gibt und das Fenster noch Platz hat.
  bool get hasMore => cursor.isNotEmpty && rows.length < auditMaxRows;

  /// Wahr, wenn das Fenster voll ist und der Rest nur über den Filter
  /// erreichbar bleibt.
  bool get windowFull => rows.length >= auditMaxRows && cursor.isNotEmpty;

  /// Wahr, wenn die Abfrage nichts geliefert hat.
  bool get isEmpty => rows.isEmpty && !loading;

  /// Eine Kopie mit den genannten Feldern. [clearFailure] nimmt den Befund
  /// weg, weil null überall sonst „unverändert" heißt.
  AuditRecordsState copyWith({
    List<AuditRecordRow>? rows,
    String? cursor,
    int? total,
    bool? loading,
    bool? loadingMore,
    Diagnostic? failure,
    bool clearFailure = false,
  }) => AuditRecordsState(
    rows: rows ?? this.rows,
    cursor: cursor ?? this.cursor,
    total: total ?? this.total,
    loading: loading ?? this.loading,
    loadingMore: loadingMore ?? this.loadingMore,
    failure: clearFailure ? null : (failure ?? this.failure),
  );

  @override
  bool operator ==(Object other) =>
      other is AuditRecordsState &&
      listEquals(other.rows, rows) &&
      other.cursor == cursor &&
      other.total == total &&
      other.loading == loading &&
      other.loadingMore == loadingMore &&
      other.failure == failure;

  @override
  int get hashCode => Object.hash(
    Object.hashAll(rows),
    cursor,
    total,
    loading,
    loadingMore,
    failure,
  );
}

/// Die Records zu einem Filter, seitenweise über den Cursor des Daemons.
///
/// Eine Familie über dem Filter: Ein neuer Filter ist eine neue Abfrage und
/// keine Änderung an der alten, und die alte darf ihre Antwort nicht mehr in
/// die neue Liste legen.
///
/// `autoDispose`: Ein Filter, den niemand mehr ansieht, gibt seine bis zu
/// [auditMaxRows] Records frei. Ohne das bliebe jede je gewählte Kombination
/// aus Art, Sitzung und Zeitraum für die Lebenszeit der Anwendung im Speicher.
final auditRecordsProvider = NotifierProvider.autoDispose
    .family<AuditRecordsNotifier, AuditRecordsState, AuditFilter>(
      AuditRecordsNotifier.new,
    );

/// Der Notifier hinter [auditRecordsProvider].
class AuditRecordsNotifier extends Notifier<AuditRecordsState> {
  /// Legt den Notifier für [filter] an; riverpod reicht das Argument der
  /// Familie an diesen Konstruktor.
  AuditRecordsNotifier(this.filter);

  /// Welche Records dieser Notifier holt.
  final AuditFilter filter;

  /// Welcher Ladevorgang der aktuelle ist. Eine Antwort auf eine ältere
  /// Abfrage wird verworfen, statt in eine Liste zu wandern, in die sie nicht
  /// gehört.
  int _generation = 0;

  @override
  AuditRecordsState build() {
    // „Jetzt prüfen" liest die Kette neu, also auch die Tabelle: Sonst nennte
    // die Karte eine neue Zahl von Records über Zeilen vom alten Stand.
    ref.watch(auditRunProvider);
    // Erst nach dem Aufbau: Wer in `build` den Zustand setzt, setzt ihn, bevor
    // es einen gibt, und riverpod bricht darauf ab.
    unawaited(Future<void>.microtask(_load));
    return AuditRecordsState.empty;
  }

  /// Holt die erste Seite neu.
  Future<void> reload() => _load();

  Future<void> _load() async {
    final int generation = ++_generation;
    state = state.copyWith(loading: true, clearFailure: true);
    try {
      final AuditPage page = await ref
          .read(daemonClientProvider)
          .auditQuery(filter, limit: auditPageRows);
      if (generation != _generation) {
        return;
      }
      state = AuditRecordsState(
        rows: page.rows,
        cursor: page.nextCursor,
        total: page.total,
      );
    } on Object catch (error) {
      if (generation != _generation) {
        return;
      }
      state = state.copyWith(
        loading: false,
        failure: DaemonConnection.diagnosticOf(error),
      );
    }
  }

  /// Holt die nächste Seite, wenn es eine gibt und keine unterwegs ist.
  Future<void> loadMore() async {
    final AuditRecordsState current = state;
    if (!current.hasMore || current.loadingMore || current.loading) {
      return;
    }
    final int generation = _generation;
    state = current.copyWith(loadingMore: true);
    try {
      final AuditPage page = await ref
          .read(daemonClientProvider)
          .auditQuery(filter, limit: auditPageRows, cursor: current.cursor);
      if (generation != _generation) {
        return;
      }
      state = state.copyWith(
        rows: <AuditRecordRow>[...state.rows, ...page.rows],
        cursor: page.nextCursor,
        total: page.total,
        loadingMore: false,
      );
    } on Object catch (error) {
      if (generation != _generation) {
        return;
      }
      state = state.copyWith(
        loadingMore: false,
        failure: DaemonConnection.diagnosticOf(error),
      );
    }
  }
}

/// Wer nach dem Ordner fragt, in den der Daemon schreibt.
///
/// Gibt den gewählten Ordner zurück oder null, wenn abgebrochen wurde.
typedef AuditFolderChooser = Future<String?> Function({
  required String dialogTitle,
});

/// Der Ordner-Dialog des Schreibtischs.
///
/// Gefragt wird nach einem **Ordner**, nicht nach einer Datei. Der
/// Speichern-Dialog von `file_picker` verlangt Bytes und legt die Datei selbst
/// an; mit null Bytes hätte er eine vorhandene Datei, die jemand zum
/// Überschreiben ausgewählt hat, geleert, bevor der Daemon auch nur gefragt
/// war — und bei einem gescheiterten Export bliebe eine leere Datei zurück.
/// Außerdem schreibt der Daemon nie in eine Datei, die schon da ist
/// (HUM-156). Diese Anwendung schreibt deshalb nichts: Sie wählt einen Ordner,
/// hängt einen Namen an, der dort noch frei ist, und überlässt dem Daemon das
/// Schreiben. Die Kette liegt bei ihm, und ein JSONL-Export, der bytegleich
/// mit der Quelle sein soll, darf nicht durch einen zweiten Kodierer laufen.
final Provider<AuditFolderChooser> auditFolderChooserProvider =
    Provider<AuditFolderChooser>(
      (Ref ref) =>
          ({required String dialogTitle}) =>
              FilePicker.getDirectoryPath(dialogTitle: dialogTitle),
    );

/// Ob unter einem Pfad schon etwas liegt. Ersetzbar in Tests.
typedef AuditPathTaken = bool Function(String path);

/// Die Prüfung gegen das Dateisystem.
final Provider<AuditPathTaken> auditPathTakenProvider =
    Provider<AuditPathTaken>(
      (Ref ref) =>
          (String path) =>
              FileSystemEntity.typeSync(path, followLinks: false) !=
              FileSystemEntityType.notFound,
    );

/// Der erste freie Pfad für [fileName] in [folder]: der Name selbst, sonst
/// `name-2.ext`, `name-3.ext` und so weiter.
///
/// Null, wenn in tausend Versuchen keiner frei war; dann schreibt niemand
/// irgendwohin, statt eine Datei zu überschreiben.
String? auditFreeExportPath(
  String folder,
  String fileName,
  AuditPathTaken taken,
) {
  final int dot = fileName.lastIndexOf('.');
  final String stem = dot <= 0 ? fileName : fileName.substring(0, dot);
  final String suffix = dot <= 0 ? '' : fileName.substring(dot);
  final String base = folder.endsWith(Platform.pathSeparator)
      ? folder
      : '$folder${Platform.pathSeparator}';
  for (int n = 1; n < 1000; n++) {
    final String candidate = n == 1 ? '$base$fileName' : '$base$stem-$n$suffix';
    if (!taken(candidate)) {
      return candidate;
    }
  }
  return null;
}

/// In welcher Phase ein Export steht.
enum AuditExportPhase {
  /// Noch keiner gelaufen.
  idle,

  /// Der Dialog steht offen oder der Daemon schreibt.
  running,

  /// Der Daemon hat geschrieben.
  done,

  /// Es wurde abgebrochen; nichts wurde geschrieben.
  cancelled,

  /// Er ist gescheitert.
  failed,
}

/// Was der letzte Export getan hat.
@immutable
class AuditExportState {
  /// Legt einen Zustand an.
  const AuditExportState({
    this.phase = AuditExportPhase.idle,
    this.path = '',
    this.records = 0,
    this.failure,
  });

  /// Noch nichts exportiert.
  static const AuditExportState idle = AuditExportState();

  /// Die Phase.
  final AuditExportPhase phase;

  /// Die Datei, die der Daemon geschrieben hat.
  final String path;

  /// Wie viele Records hineingingen.
  final int records;

  /// Warum er scheiterte, oder null.
  final Diagnostic? failure;

  /// Wahr, solange er läuft.
  bool get running => phase == AuditExportPhase.running;

  @override
  bool operator ==(Object other) =>
      other is AuditExportState &&
      other.phase == phase &&
      other.path == path &&
      other.records == records &&
      other.failure == failure;

  @override
  int get hashCode => Object.hash(phase, path, records, failure);
}

/// Der Export des Audit-Logs.
///
/// `autoDispose`: Die Meldung des letzten Exports gehört zu dem Besuch des
/// Bildschirms, in dem er lief; wer später wiederkommt, sieht keine alte
/// Bestätigung und keinen alten Fehler. Ein laufender Export hält den Provider
/// mit einem `keepAlive` am Leben, bis er fertig ist; wer mitten im Export den
/// Bildschirm verlässt, bricht ihn nicht ab ([AuditExportNotifier.run]).
final auditExportProvider =
    NotifierProvider.autoDispose<AuditExportNotifier, AuditExportState>(
      AuditExportNotifier.new,
    );

/// Der Notifier hinter [auditExportProvider].
class AuditExportNotifier extends Notifier<AuditExportState> {
  @override
  AuditExportState build() => AuditExportState.idle;

  /// Fragt nach einem Pfad und lässt den Daemon dorthin schreiben.
  ///
  /// [filter] liefert den Zeitraum: Was die Tabelle zeigt, ist, was die Datei
  /// enthält. Art und Sitzung schränken den Export **nicht** ein — der
  /// Vertrag kennt für den Export nur den Zeitraum, und eine Oberfläche, die
  /// hier mehr verspräche, als sie an den Daemon weitergibt, schriebe eine
  /// Datei mit mehr Zeilen, als sie angekündigt hat.
  Future<void> run({
    required AuditExportFormat format,
    required AuditFilter filter,
    required String dialogTitle,
    DateTime? now,
  }) async {
    if (state.running) {
      return;
    }
    // Solange der Export läuft, darf der Provider nicht fallen: Sonst käme der
    // Aufruf nicht mehr beim Daemon an, oder niemand meldete sein Ergebnis.
    final KeepAliveLink running = ref.keepAlive();
    try {
      await _export(
        format: format,
        filter: filter,
        dialogTitle: dialogTitle,
        now: now,
      );
    } finally {
      running.close();
    }
  }

  Future<void> _export({
    required AuditExportFormat format,
    required AuditFilter filter,
    required String dialogTitle,
    DateTime? now,
  }) async {
    state = const AuditExportState(phase: AuditExportPhase.running);
    final String fileName = auditExportFileName(
      format,
      now: now ?? DateTime.now(),
    );
    try {
      final String? folder = await ref.read(auditFolderChooserProvider)(
        dialogTitle: dialogTitle,
      );
      if (folder == null || folder.isEmpty) {
        state = const AuditExportState(phase: AuditExportPhase.cancelled);
        return;
      }
      final String? path = auditFreeExportPath(
        folder,
        fileName,
        ref.read(auditPathTakenProvider),
      );
      if (path == null) {
        state = AuditExportState(
          phase: AuditExportPhase.failed,
          failure: Diagnostic(
            code: DiagnosticCodes.capabilityUnavailable,
            severity: Severity.error,
            why:
                'no free file name for $fileName in $folder after 1000 '
                'attempts; nothing was written',
            fix: FixAction.copyCommand(command: 'ls -la $folder'),
          ),
        );
        return;
      }
      final AuditExport written = await ref
          .read(daemonClientProvider)
          .auditExport(
            format: format,
            outPath: path,
            from: filter.from,
            to: filter.to,
          );
      state = AuditExportState(
        phase: AuditExportPhase.done,
        path: written.path,
        records: written.records,
      );
    } on Object catch (error) {
      state = AuditExportState(
        phase: AuditExportPhase.failed,
        failure: DaemonConnection.diagnosticOf(error),
      );
    }
  }

  /// Räumt die Meldung des letzten Exports weg.
  void dismiss() => state = AuditExportState.idle;
}

/// Der Name, den der Dialog vorschlägt: was darin steht und wann es genommen
/// wurde, damit zwei Exporte einander nicht überschreiben.
String auditExportFileName(AuditExportFormat format, {required DateTime now}) {
  final DateTime at = now.toUtc();
  String two(int value) => value.toString().padLeft(2, '0');
  final String stamp =
      '${at.year.toString().padLeft(4, '0')}${two(at.month)}${two(at.day)}'
      'T${two(at.hour)}${two(at.minute)}${two(at.second)}Z';
  return 'humanitl-audit-$stamp.${format.fileExtension}';
}
