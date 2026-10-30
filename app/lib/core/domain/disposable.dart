/// Die Wegwerf-Sandbox: Spiegel von `DisposableChange`, `DisposableChanges`
/// und `DisposableOutcome` (HUM-350, HUM-351).
///
/// Alles hier kommt aus dem Daemon. Pfade sind Anzeige und nie wieder ein
/// Pfad: Der Agent hat die Namen geschrieben, der Daemon hat sie gesäubert,
/// und ausgewählt wird eine Änderung nur über ihre [DisposableChange.id], den
/// Pfad-Hash. Ob eine Änderung einen Konflikt hat oder nie geschrieben wird,
/// entscheidet der Daemon und schreibt den Satz dazu; die Oberfläche zeigt ihn
/// und urteilt nicht selbst (ADR-018).
library;

import 'package:freezed_annotation/freezed_annotation.dart';

part 'disposable.freezed.dart';

/// Wie die Sandbox das Projektverzeichnis einhängt, `sandbox.mode`.
enum SandboxMode {
  /// Der Agent schreibt direkt in den Ordner (oder gar nicht, `ro`).
  direct,

  /// Der Ordner liegt nur lesbar unter einer Schicht im Arbeitsspeicher; die
  /// Änderungen warten nach dem Ende auf eine Entscheidung.
  disposable;

  /// Die Form auf der Leitung, `direct` oder `disposable`.
  String get wire => name;

  /// Der Modus, den [wire] nennt, oder [SandboxMode.direct] für alles andere,
  /// auch einen leeren Wert: Das ist die Vorgabe des Schemas.
  static SandboxMode fromWire(String wire) =>
      wire.trim().toLowerCase() == 'disposable'
      ? SandboxMode.disposable
      : SandboxMode.direct;
}

/// Was mit einem Eintrag geschehen ist, Spiegel von `FileChangeKind`.
///
/// Die Reihenfolge ist die der Leitung ohne `_UNSPECIFIED`, damit
/// `enumFromWire` sie über die Position abbildet.
enum DisposableChangeKind {
  /// Neu angelegt.
  added,

  /// Inhalt geändert.
  modified,

  /// Gelöscht; [DisposableChange.entry] sagt, was er vorher war.
  removed,

  /// Ein Symlink, neu oder mit neuem Ziel.
  symlink,

  /// Nur die Rechte geändert.
  mode,

  /// Eine Art, die dieser Build nicht kennt.
  unknown,
}

/// Eine Änderung der oberen Schicht, Spiegel von `DisposableChange`.
@freezed
abstract class DisposableChange with _$DisposableChange {
  /// Legt eine Änderung an.
  const factory DisposableChange({
    required String id,
    required String path,
    required DisposableChangeKind kind,
    @Default(false) bool mangled,
    @Default('') String entry,
    @Default(0) int size,
    @Default(0) int mode,
    @Default('') String sha256,
    @Default('') String symlinkTarget,
    @Default('') String diff,
    @Default(false) bool binary,
    @Default('') String conflict,
    @Default('') String refused,
    @Default(0) int strippedBits,
    @Default(false) bool hardlinked,
  }) = _DisposableChange;

  const DisposableChange._();

  /// Wahr, wenn der Daemon diese Änderung nie auf den Host schreibt.
  bool get isRefused => refused.isNotEmpty;

  /// Wahr, wenn sich die Datei im Projekt seit dem Start geändert hat; dann
  /// schreibt der Daemon sie nur mit `overwrite`.
  bool get hasConflict => conflict.isNotEmpty;

  /// Wahr, wenn der Daemon einen Text-Diff mitgeschickt hat.
  bool get hasDiff => diff.isNotEmpty;
}

/// Die wartenden Änderungen eines Laufs, Spiegel von `DisposableChanges`.
@freezed
abstract class DisposableChanges with _$DisposableChanges {
  /// Legt die Liste an.
  const factory DisposableChanges({
    required String sandboxId,
    required String workDir,
    @Default(<DisposableChange>[]) List<DisposableChange> changes,
    @Default(false) bool truncated,
  }) = _DisposableChanges;
}

/// Was mit den Änderungen geschehen ist, `DisposableOutcome.outcome`.
enum DisposableOutcomeKind {
  /// Die gewählten sind geschrieben, der Rest verworfen.
  applied,

  /// Alle verworfen.
  discarded,

  /// Das Schreiben brach ab; die geschriebenen stehen in
  /// [DisposableOutcome.applied], die Schicht wartet weiter.
  partial,

  /// Ein Ergebnis, das dieser Build nicht kennt.
  unknown;

  /// Die Art, die [wire] nennt.
  static DisposableOutcomeKind fromWire(String wire) => switch (wire) {
    'applied' => DisposableOutcomeKind.applied,
    'discarded' => DisposableOutcomeKind.discarded,
    'partial' => DisposableOutcomeKind.partial,
    _ => DisposableOutcomeKind.unknown,
  };
}

/// Die Antwort auf Übernehmen oder Verwerfen, Spiegel von
/// `DisposableOutcome`.
@freezed
abstract class DisposableOutcome with _$DisposableOutcome {
  /// Legt das Ergebnis an.
  const factory DisposableOutcome({
    required String sandboxId,
    required DisposableOutcomeKind outcome,
    @Default(<String>[]) List<String> applied,
    @Default(<String>[]) List<String> discarded,
  }) = _DisposableOutcome;
}
