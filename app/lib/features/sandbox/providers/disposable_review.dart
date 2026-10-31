/// Die Entscheidung über die Änderungen einer Wegwerf-Sandbox (HUM-351).
///
/// Der Daemon hält die Schicht, liest sie, vergleicht sie mit dem Projekt und
/// schreibt sie; diese Datei hält nur, was der Mensch gerade ausgewählt hat,
/// und schickt die gewählten Pfad-Hashes zurück (ADR-018). Ob eine Änderung
/// einen Konflikt hat oder nie geschrieben wird, steht in der Antwort des
/// Daemons und wird hier nicht ausgerechnet.
///
/// # Überschreiben gilt je Datei, nie global
///
/// Eine Datei mit Konflikt ist zunächst nicht gewählt, und gewählt wird sie
/// nur über eine Bestätigung genau für sie
/// ([DisposableReviewNotifier.confirmOverwrite]). Zur Bestätigung gehört der
/// Satz des Daemons, auf den sie sich bezog: Ändert er sich oder verschwindet
/// der Konflikt beim nächsten Lesen, fällt die Bestätigung weg, und eine
/// Datei, die erst nach der Auswahl einen Konflikt bekam, fällt aus der
/// Auswahl. Auf die Leitung geht jede bestätigte Kennung einzeln
/// (`ApplyChanges.overwrite_ids`), nie der Schalter `overwrite`; der Daemon
/// überschreibt damit nur, was hier genau so bestätigt wurde, auch wenn eine
/// andere Datei zwischen dem letzten Lesen und dem Übernehmen einen Konflikt
/// bekommt.
///
/// # Was der Agent geschrieben hat, wird ersetzt, nie gelöscht
///
/// Pfade, Symlink-Ziele, Diffs und die Sätze zu Konflikt und Ablehnung
/// enthalten Namen und Inhalte, die der Agent gewählt hat. Der Daemon
/// maskiert sie schon; die App ersetzt zusätzlich jedes unsichtbare oder
/// richtungsdrehende Zeichen einzeln ([sanitizeChanges]), damit eine Lücke in
/// einer der beiden Masken nicht reicht, um eine Zeile anders aussehen zu
/// lassen, als sie geschrieben wird.
library;

import 'dart:async';

import 'package:flutter/foundation.dart' show immutable, listEquals;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/body/body_span.dart';
import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/text/sanitize_diagnostic.dart';
import 'sandbox_status_provider.dart';

part 'disposable_review.g.dart';

/// Was ein Übernehmen schickt, so wie die Rückfrage es gezeigt hat: der Lauf,
/// die gewählten Kennungen und darunter die, deren Überschreiben einzeln
/// bestätigt ist.
///
/// Der Lauf gehört dazu. Verwirft ein anderer Client Lauf A und hinterlässt
/// Lauf B im selben Ordner dieselben Pfade, tragen seine Änderungen dieselben
/// Kennungen; ohne die Sandbox-Kennung schickte eine Rückfrage über A die
/// Inhalte von B, die niemand gesehen hat.
@immutable
final class ChosenApply {
  /// Legt die Auswahl an.
  const ChosenApply({
    required this.sandboxId,
    required this.ids,
    required this.overwriteIds,
  });

  /// `ApplyChanges.sandbox_id`: der Lauf, dessen Liste gezeigt wurde.
  final String sandboxId;

  /// `ApplyChanges.ids`, in der Reihenfolge der Liste.
  final List<String> ids;

  /// `ApplyChanges.overwrite_ids`.
  final List<String> overwriteIds;

  @override
  bool operator ==(Object other) =>
      other is ChosenApply &&
      other.sandboxId == sandboxId &&
      listEquals(other.ids, ids) &&
      listEquals(other.overwriteIds, overwriteIds);

  @override
  int get hashCode =>
      Object.hash(sandboxId, Object.hashAll(ids), Object.hashAll(overwriteIds));
}

/// Was das Panel zeigt: die wartenden Änderungen, die Auswahl und das
/// Ergebnis der letzten Entscheidung.
@immutable
final class DisposableReview {
  /// Legt einen Stand an.
  const DisposableReview({
    this.changes,
    this.selected = const <String>{},
    this.overwrite = const <String, String>{},
    this.outcome,
    this.notice,
    this.busy = false,
    this.open = false,
    this.arrivals = 0,
  });

  /// Die wartenden Änderungen, oder null, wenn nichts wartet.
  final DisposableChanges? changes;

  /// Die Kennungen, die übernommen werden sollen.
  final Set<String> selected;

  /// Die Kennungen mit Konflikt, deren Überschreiben jemand einzeln bestätigt
  /// hat, jede mit dem Satz des Daemons, den die Bestätigung sah. Immer eine
  /// Teilmenge von [selected].
  final Map<String, String> overwrite;

  /// Was die letzte Entscheidung ergab, oder null.
  final DisposableOutcome? outcome;

  /// Der Befund des Daemons zur letzten Entscheidung: `SANDBOX_031`, wenn eine
  /// Prüfung vor dem ersten Schreiben scheiterte, oder der Grund eines
  /// `partial`.
  final Diagnostic? notice;

  /// Wahr, solange eine Entscheidung unterwegs ist.
  final bool busy;

  /// Wahr, solange das Panel offen ist.
  final bool open;

  /// Zählt, wie oft der Strom eines Starts Änderungen gebracht hat. Die Shell
  /// wechselt bei jedem neuen Wert auf die Sandbox-Seite, damit das Panel
  /// ohne Klick erscheint.
  final int arrivals;

  /// Wahr, wenn Änderungen auf eine Entscheidung warten.
  bool get waiting => changes != null;

  /// Die Zeilen in der Reihenfolge des Daemons.
  List<DisposableChange> get rows =>
      changes?.changes ?? const <DisposableChange>[];

  /// Die gewählten Kennungen in der Reihenfolge der Liste.
  List<String> get chosenIds => <String>[
    for (final DisposableChange change in rows)
      if (selected.contains(change.id)) change.id,
  ];

  /// Was „Auswahl übernehmen" jetzt schicken würde.
  ChosenApply get chosen {
    final List<String> ids = chosenIds;
    return ChosenApply(
      sandboxId: changes?.sandboxId ?? '',
      ids: ids,
      overwriteIds: <String>[
        for (final String id in ids)
          if (overwrite.containsKey(id)) id,
      ],
    );
  }

  /// Was „Alle übernehmen" jetzt schicken würde: jede Kennung, nichts zum
  /// Überschreiben.
  ChosenApply get all => ChosenApply(
    sandboxId: changes?.sandboxId ?? '',
    ids: <String>[for (final DisposableChange change in rows) change.id],
    overwriteIds: const <String>[],
  );

  /// Wahr, wenn „Alle übernehmen" dasselbe heißt wie jede Zeile: keine wird
  /// abgelehnt, keine hat einen Konflikt, und die Liste ist nicht
  /// abgeschnitten (der Daemon lehnt „alle" dann ab).
  bool get canApplyAll =>
      waiting &&
      !changes!.truncated &&
      rows.isNotEmpty &&
      rows.every(
        (DisposableChange change) => !change.isRefused && !change.hasConflict,
      );

  /// Wahr, wenn das Panel etwas zu zeigen hat.
  bool get hasContent => waiting || outcome != null || notice != null;

  /// Ein Stand mit den genannten Feldern ersetzt. [changes], [outcome] und
  /// [notice] werden über die `clear`-Schalter auf null gesetzt.
  DisposableReview copyWith({
    DisposableChanges? changes,
    bool clearChanges = false,
    Set<String>? selected,
    Map<String, String>? overwrite,
    DisposableOutcome? outcome,
    bool clearOutcome = false,
    Diagnostic? notice,
    bool clearNotice = false,
    bool? busy,
    bool? open,
    int? arrivals,
  }) => DisposableReview(
    changes: clearChanges ? null : changes ?? this.changes,
    selected: selected ?? this.selected,
    overwrite: overwrite ?? this.overwrite,
    outcome: clearOutcome ? null : outcome ?? this.outcome,
    notice: clearNotice ? null : notice ?? this.notice,
    busy: busy ?? this.busy,
    open: open ?? this.open,
    arrivals: arrivals ?? this.arrivals,
  );
}

/// Die Vorauswahl einer Liste: jede Änderung, die geschrieben werden darf und
/// keinen Konflikt hat. Abgelehnte Zeilen sind nie wählbar, Zeilen mit
/// Konflikt nur über eine Bestätigung.
Set<String> defaultSelection(DisposableChanges changes) => <String>{
  for (final DisposableChange change in changes.changes)
    if (!change.isRefused && !change.hasConflict) change.id,
};

/// [changes], so wie das Panel sie zeigen darf: jedes Zeichen, das der Agent
/// in Pfad, Ziel, Diff oder einen Satz gelegt haben kann, einzeln ersetzt,
/// keines gelöscht. Die Kennung bleibt, wie sie ist: Sie geht an den Daemon
/// zurück.
DisposableChanges sanitizeChanges(DisposableChanges changes) =>
    changes.copyWith(
      workDir: sanitizeBodyText(changes.workDir),
      changes: <DisposableChange>[
        for (final DisposableChange change in changes.changes)
          change.copyWith(
            path: sanitizeBodyText(change.path),
            entry: sanitizeBodyText(change.entry),
            sha256: sanitizeBodyText(change.sha256),
            symlinkTarget: sanitizeBodyText(change.symlinkTarget),
            diff: sanitizeBodyText(change.diff),
            conflict: sanitizeBodyText(change.conflict),
            refused: sanitizeBodyText(change.refused),
          ),
      ],
    );

/// Die Auswahl nach einem neuen Lesen derselben Liste (HUM-351).
///
/// Gewählt bleibt, was noch da ist, noch geschrieben werden darf und keinen
/// Konflikt hat, den niemand bestätigt hat. Eine Bestätigung bleibt nur,
/// solange der Daemon denselben Konflikt mit demselben Satz meldet; ein
/// verschwundener oder geänderter Konflikt nimmt sie weg, und mit ihr die
/// Datei aus der Auswahl, wenn sie nun einen anderen Konflikt hat.
(Set<String>, Map<String, String>) reconcileSelection(
  DisposableChanges found,
  Set<String> selected,
  Map<String, String> overwrite,
) {
  final Map<String, String> kept = <String, String>{};
  final Set<String> chosen = <String>{};
  for (final DisposableChange change in found.changes) {
    final String? confirmed = overwrite[change.id];
    if (confirmed != null &&
        change.hasConflict &&
        !change.isRefused &&
        change.conflict == confirmed) {
      kept[change.id] = confirmed;
    }
    if (!selected.contains(change.id) || change.isRefused) {
      continue;
    }
    if (change.hasConflict && !kept.containsKey(change.id)) {
      continue;
    }
    chosen.add(change.id);
  }
  return (chosen, kept);
}

/// Die Entscheidung über eine Wegwerf-Sandbox, wie der Mensch sie gerade
/// trifft.
@Riverpod(keepAlive: true)
class DisposableReviewNotifier extends _$DisposableReviewNotifier {
  @override
  DisposableReview build() {
    ref.watch(daemonClientProvider);
    // Nach jeder (Wieder-)Verbindung fragen, ob Änderungen warten: Der Strom
    // des Starts, der sie gebracht hätte, ist mit der alten Verbindung weg.
    ref.listen<AsyncValue<DaemonInfo>>(daemonInfoProvider, (
      AsyncValue<DaemonInfo>? _,
      AsyncValue<DaemonInfo> next,
    ) {
      if (next.value?.offersDisposable ?? false) {
        scheduleMicrotask(() => unawaited(fetch()));
      }
    }, fireImmediately: true);
    return const DisposableReview();
  }

  bool get _offered =>
      ref.read(daemonInfoProvider).value?.offersDisposable ?? false;

  /// Zählt, was den Stand der Liste neu setzt: eine Ankunft auf dem Strom
  /// eines Starts und jede Entscheidung. Eine Antwort von [fetch] gilt nur,
  /// solange der Zähler noch der ist, mit dem sie gefragt wurde; sonst brächte
  /// eine späte Antwort eine verworfene Liste zurück oder nähme eine gerade
  /// angekommene weg.
  int _generation = 0;

  /// Zählt die Fragen von [fetch]; nur die jüngste darf den Stand setzen.
  int _fetches = 0;

  /// Die Läufe, deren Panel schon einmal von selbst aufging. Nach einer
  /// Wiederverbindung geht es für denselben Lauf nicht noch einmal auf, wenn
  /// jemand es geschlossen hat.
  final Set<String> _revealed = <String>{};

  /// Der Strom eines Starts hat [changes] gebracht: Das Panel geht auf, und
  /// die Shell bringt die Sandbox-Seite nach vorn.
  void arrived(DisposableChanges changes) {
    _generation++;
    final DisposableChanges clean = sanitizeChanges(changes);
    _revealed.add(clean.sandboxId);
    state = DisposableReview(
      changes: clean,
      selected: defaultSelection(clean),
      open: true,
      arrivals: state.arrivals + 1,
    );
  }

  /// Fragt den Daemon, ob Änderungen warten (`Sandbox(Changes)`), etwa nach
  /// einer Wiederverbindung oder wenn die Seite sichtbar wird.
  ///
  /// Dieselbe Liste wie zuvor behält ihre Auswahl nach
  /// [reconcileSelection]. Eine neue Liste öffnet das Panel einmal von selbst.
  /// Ein Befund heißt: nichts wartet (oder die Sandbox läuft noch); dann
  /// verschwindet eine alte Liste, ein Ergebnis bleibt stehen.
  Future<void> fetch() async {
    if (!_offered || state.busy) {
      return;
    }
    final int asked = _generation;
    final int mine = ++_fetches;
    DisposableChanges? found;
    try {
      await for (final SandboxUpdate update
          in ref.read(daemonClientProvider).sandboxChanges()) {
        if (update case SandboxUpdateChanges(
          :final DisposableChanges changes,
        )) {
          found = sanitizeChanges(changes);
        }
      }
    } on DaemonException {
      return;
    }
    // Nur die jüngste Frage zählt: Eine ältere Antwort „es wartet etwas"
    // überschriebe sonst eine neuere „nichts wartet".
    if (state.busy || asked != _generation || mine != _fetches) {
      return;
    }
    if (found == null) {
      if (state.waiting) {
        state = state.copyWith(
          clearChanges: true,
          selected: const <String>{},
          overwrite: const <String, String>{},
        );
      }
      return;
    }
    final DisposableChanges? before = state.changes;
    if (before != null && before.sandboxId == found.sandboxId) {
      final (Set<String> selected, Map<String, String> overwrite) =
          reconcileSelection(found, state.selected, state.overwrite);
      state = state.copyWith(
        changes: found,
        selected: selected,
        overwrite: overwrite,
      );
      return;
    }
    // Eine Liste, die noch niemand gesehen hat, etwa nach einer
    // Wiederverbindung: Das Panel geht einmal von selbst auf.
    final bool reveal = _revealed.add(found.sandboxId);
    state = state.copyWith(
      changes: found,
      selected: defaultSelection(found),
      overwrite: const <String, String>{},
      open: reveal || state.open,
    );
  }

  /// Wählt die Änderung [id] ab oder, wenn sie ohne Bestätigung wählbar ist,
  /// an. Eine abgelehnte Zeile bleibt abgewählt; eine Zeile mit Konflikt wird
  /// hier nie gewählt, dafür gibt es [confirmOverwrite].
  void toggle(String id, {required bool on}) {
    final DisposableChange? change = _row(id);
    if (change == null) {
      return;
    }
    if (!on) {
      state = state.copyWith(
        selected: <String>{...state.selected}..remove(id),
        overwrite: <String, String>{...state.overwrite}..remove(id),
      );
      return;
    }
    if (change.isRefused || change.hasConflict) {
      return;
    }
    state = state.copyWith(selected: <String>{...state.selected, id});
  }

  /// Wählt die Datei [id] mit Konflikt, nachdem jemand bestätigt hat, dass
  /// sie überschrieben werden darf. Nur für genau diese Datei, und nur für
  /// den Konflikt [sawConflict] des Laufs [sandboxId], den die Rückfrage
  /// gezeigt hat; meldet der Daemon inzwischen einen anderen Konflikt oder
  /// keinen, oder steht ein anderer Lauf mit demselben Pfad da, wird nichts
  /// gewählt und die Antwort ist falsch.
  bool confirmOverwrite(
    String id, {
    required String sawConflict,
    required String sandboxId,
  }) {
    final DisposableChange? change = _row(id);
    if (!ref.read(perChangeOverwriteProvider) ||
        state.changes?.sandboxId != sandboxId ||
        change == null ||
        change.isRefused ||
        !change.hasConflict ||
        change.conflict != sawConflict) {
      return false;
    }
    state = state.copyWith(
      selected: <String>{...state.selected, id},
      overwrite: <String, String>{...state.overwrite, id: change.conflict},
    );
    return true;
  }

  DisposableChange? _row(String id) {
    for (final DisposableChange change in state.rows) {
      if (change.id == id) {
        return change;
      }
    }
    return null;
  }

  /// Übernimmt genau [asked], die Auswahl, die die Rückfrage gezeigt hat.
  ///
  /// Hat sich die Auswahl seitdem geändert (ein neues Lesen nahm eine
  /// Bestätigung weg, eine Zeile fiel heraus), wird nichts geschickt und die
  /// Antwort ist falsch: Die Rückfrage muss neu fragen. Ohne Auswahl geschieht
  /// nichts, denn eine leere Liste hieße für den Daemon „alle".
  bool applyChosen(ChosenApply asked) {
    if (state.changes == null ||
        asked.ids.isEmpty ||
        asked != state.chosen ||
        (asked.overwriteIds.isNotEmpty &&
            !ref.read(perChangeOverwriteProvider))) {
      return false;
    }
    _send(asked);
    return true;
  }

  /// Übernimmt jede Änderung, wie die Rückfrage sie gezeigt hat: [asked] muss
  /// noch derselbe Lauf mit denselben Kennungen sein, und
  /// [DisposableReview.canApplyAll] muss gelten; überschrieben wird nichts.
  /// Sonst wird nichts geschickt und die Antwort ist falsch.
  bool applyAll(ChosenApply asked) {
    if (state.changes == null ||
        !state.canApplyAll ||
        asked.ids.isEmpty ||
        asked != state.all) {
      return false;
    }
    _send(asked);
    return true;
  }

  void _send(ChosenApply asked) => unawaited(
    _decide(
      (DaemonClient client) => client.applyChanges(
        sandboxId: asked.sandboxId,
        ids: asked.ids,
        overwriteIds: asked.overwriteIds,
      ),
    ),
  );

  /// Verwirft alle wartenden Änderungen, wie die Rückfrage sie gezeigt hat:
  /// [asked] muss noch derselbe Lauf mit denselben Kennungen sein. Ist
  /// inzwischen ein anderer Lauf da oder hat ein neues Lesen Zeilen
  /// hinzugefügt oder entfernt, geschieht nichts und die Antwort ist falsch.
  bool discard(ChosenApply asked) {
    final DisposableChanges? changes = state.changes;
    if (changes == null || asked != state.all) {
      return false;
    }
    unawaited(_discard(changes));
    return true;
  }

  Future<void> _discard(DisposableChanges changes) {
    return _decide(
      (DaemonClient client) =>
          client.discardChanges(sandboxId: changes.sandboxId),
    );
  }

  /// Schickt eine Entscheidung und legt ab, was der Daemon antwortet.
  ///
  /// Nur ein Befund heißt: nichts ist geschrieben, die Liste und die Auswahl
  /// bleiben für einen zweiten Versuch. `partial` heißt: ein Teil ist
  /// geschrieben, die Schicht wartet weiter, und die Liste wird neu gelesen.
  /// `applied` und `discarded` beenden die Entscheidung.
  Future<void> _decide(
    Stream<SandboxUpdate> Function(DaemonClient) request,
  ) async {
    if (state.busy) {
      return;
    }
    _generation++;
    state = state.copyWith(busy: true, clearNotice: true, clearOutcome: true);
    Diagnostic? notice;
    DisposableOutcome? outcome;
    try {
      await for (final SandboxUpdate update in request(
        ref.read(daemonClientProvider),
      )) {
        switch (update) {
          case SandboxUpdateDiagnostic(:final Diagnostic diagnostic):
            notice ??= sanitizeDiagnostic(diagnostic);
          case SandboxUpdateChangesOutcome(
            outcome: final DisposableOutcome answer,
          ):
            outcome = answer;
          default:
            break;
        }
      }
    } on DaemonException catch (error) {
      notice = sanitizeDiagnostic(error.diagnostic);
    }
    _generation++;
    final DisposableOutcome? result = outcome;
    if (result == null) {
      state = state.copyWith(busy: false, notice: notice);
      return;
    }
    if (result.outcome == DisposableOutcomeKind.partial) {
      state = state.copyWith(busy: false, outcome: result, notice: notice);
      await fetch();
      return;
    }
    state = DisposableReview(
      outcome: result,
      notice: notice,
      open: state.open,
      arrivals: state.arrivals,
    );
  }

  /// Öffnet das Panel.
  void show() => state = state.copyWith(open: true);

  /// Schließt das Panel. Ein Ergebnis ohne wartende Änderungen ist damit
  /// gelesen und verschwindet; wartende Änderungen bleiben und sind über die
  /// Sandbox-Seite wieder zu erreichen.
  void hide() {
    if (state.busy) {
      state = state.copyWith(open: false);
      return;
    }
    state = state.waiting
        ? state.copyWith(open: false, clearOutcome: true, clearNotice: true)
        : DisposableReview(arrivals: state.arrivals);
  }
}

/// Der Modus, den der Schalter „Wegwerf" für den nächsten Start gewählt hat,
/// oder null, solange niemand ihn angefasst hat (HUM-351).
///
/// Null heißt: `Start.sandbox_mode` bleibt leer, und der Daemon nimmt
/// `sandbox.mode` aus seiner Konfiguration, denselben Wert, den der Schalter
/// dann anzeigt. So schickt ein Start nie einen Wert, den der Mensch nicht
/// gewählt hat und der Daemon nicht ohnehin nähme. Eine Wahl, die der
/// Vorgabe gleicht ([defaultSandboxModeProvider]), geht ebenfalls nicht mit
/// (HUM-357): Sonst würde aus der Vorgabe `auto`, die auf `direct`
/// zurückfallen darf, ein ausdrückliches `disposable`, das ablehnt.
@Riverpod(keepAlive: true)
class SandboxModeChoice extends _$SandboxModeChoice {
  @override
  SandboxMode? build() => null;

  /// Wählt [mode] für den nächsten Start.
  void choose(SandboxMode mode) => state = mode;
}

/// Der Modus, den ein Start ohne Wahl des Schalters bekommt (HUM-357): ein
/// ausdrückliches `sandbox.mode` aus `GetConfig`, sonst, für die Vorgabe
/// `auto`, was der Daemon daraus für dieses System macht
/// (`SandboxStatus.effectiveSandboxMode`), sonst `direct`.
///
/// Der ausdrückliche Wert geht vor die Momentaufnahme, weil diese erst beim
/// nächsten Plan nachzieht: Wer im Formular gerade `direct` gewählt hat, sieht
/// den Schalter sofort aus. Über `auto` entscheidet allein der Daemon; die App
/// rät nicht, ob das System ein Overlay kann (ADR-018).
@Riverpod(keepAlive: true)
SandboxMode defaultSandboxMode(Ref ref) => sandboxModeDefault(
  ref.watch(configProvider).value,
  ref.watch(sandboxStatusProvider).value,
);

/// Die Regel von [defaultSandboxModeProvider] für einen Leser, der den
/// Provider nicht beobachten darf: der Start selbst, dessen Zustand die
/// Momentaufnahme ist.
SandboxMode sandboxModeDefault(ConfigValues? config, SandboxStatus? status) =>
    config?.sandboxMode ?? status?.effectiveSandboxMode ?? SandboxMode.direct;

/// Der Modus, mit dem der nächste Start läuft: die Wahl des Schalters, sonst
/// [defaultSandboxModeProvider] (HUM-351, HUM-357).
///
/// Der Daemon lehnt nur einen Wegwerf-Start ab, solange Änderungen warten
/// (`SANDBOX_030`); ein direkter Start geht. Die Startknöpfe sperren deshalb
/// nur, wenn dieser Modus `disposable` ist.
@Riverpod(keepAlive: true)
SandboxMode nextSandboxMode(Ref ref) =>
    ref.watch(sandboxModeChoiceProvider) ??
    ref.watch(defaultSandboxModeProvider);

/// Ob der Daemon eine Bestätigung je Datei kennt (`ApplyChanges.overwrite_ids`,
/// Minor 26, HUM-351).
///
/// Ein Daemon mit Minor 25 nennt `sandbox.disposable`, liest das Feld aber
/// nicht und bricht an jedem Konflikt ab. Die App bietet dann kein
/// Überschreiben an und fällt nie auf den Schalter `overwrite` zurück, der
/// jede gewählte Datei überschriebe.
@Riverpod(keepAlive: true)
bool perChangeOverwrite(Ref ref) =>
    (ref.watch(daemonInfoProvider).value?.protoMinor ?? 0) >= 26;
