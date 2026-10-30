/// Der Modus, den der Daemon für den nächsten Start ohne Wahl nennt, im Kern
/// (HUM-357).
///
/// Die Sandbox-Seite liest ihn aus ihrer Momentaufnahme, die Einstellungen
/// brauchen denselben Stand, und kein Feature importiert ein anderes
/// (ARCHITECTURE 5). Die Shell hält beide und schreibt jeden neuen Stand der
/// Momentaufnahme hier hinein, wie beim Projektordner (HUM-295).
library;

import 'package:flutter/foundation.dart' show immutable;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../domain/domain.dart';

part 'sandbox_mode_feed.g.dart';

/// Was der Daemon aus `sandbox.mode` für dieses System macht, Spiegel von
/// `SandboxEvent.Status.sandbox_mode` und `sandbox_mode_notice`.
@immutable
final class SandboxModeResolution {
  /// Legt den Stand an; beides fehlt, solange der Daemon nichts sagte.
  const SandboxModeResolution({this.mode, this.notice});

  /// `direct` oder `disposable`, oder null.
  final SandboxMode? mode;

  /// Warum der Modus nicht ist, was `sandbox.mode` verlangt, oder null.
  final Diagnostic? notice;

  @override
  bool operator ==(Object other) =>
      other is SandboxModeResolution &&
      other.mode == mode &&
      other.notice == notice;

  @override
  int get hashCode => Object.hash(mode, notice);
}

/// Der letzte Stand, den die Shell aus der Momentaufnahme weitergab.
@Riverpod(keepAlive: true)
class ResolvedSandboxMode extends _$ResolvedSandboxMode {
  @override
  SandboxModeResolution build() => const SandboxModeResolution();

  /// Übernimmt einen neuen Stand.
  void set(SandboxModeResolution value) {
    if (value != state) {
      state = value;
    }
  }
}
