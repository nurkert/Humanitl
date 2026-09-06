// Der Zustandsspeicher der Oberflaeche fuer Tests (HUM-044).
//
// `UiStateFile.resolve()` zeigt auf `$XDG_STATE_HOME/humanitl/ui-state.json`,
// also im Test auf das Heimatverzeichnis des Menschen, der ihn laufen laesst.
// Zwei Gruende, warum kein Test das anfassen darf:
//
//  1. Ein Test schriebe sonst in die Sitzung eines Menschen, und der Hinweis
//     des ersten gehaltenen Requests waere danach fuer ihn verbraucht.
//  2. Ohne die Datei gilt „noch nicht gesehen", und der Hinweis erschiene in
//     jedem Test und in jedem Golden, der eine gehaltene Anfrage zeigt. Das
//     ist der richtige Zustand beim allerersten Start und der falsche fuer
//     einen Test, der von etwas anderem handelt.
//
// Jedes Geruest legt deshalb `uiStateOverride` vor die eigenen Overrides. Wer
// den Hinweis pruefen will, setzt `seen: false` und ueberschreibt damit die
// Vorgabe.

import 'dart:io';

import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/features/intercept/providers/coach_mark.dart';

/// Wie viele Speicher dieser Testlauf schon ausgegeben hat.
///
/// Jeder `seen: false` bekommt eine eigene Datei: Der Hinweis schreibt seine
/// Marke hinein, sobald er einmal auf dem Schirm war, und ein zweiter Test in
/// derselben Datei faende sie vor.
int _next = 0;

/// Das Verzeichnis dieses Testlaufs.
///
/// Ohne `addTearDown`, damit die Funktion auch ausserhalb eines Testrumpfs
/// aufgerufen werden darf -- die Goldens bauen ihre Provider in einer
/// Hilfsfunktion. Es liegt unter dem temporaeren Verzeichnis des Systems und
/// traegt die Prozessnummer, also raeumt es der Rechner mit auf.
final Directory _dir = Directory.systemTemp.createTempSync('humanitl-ui-state');

/// Ein Zustandsspeicher in einer eigenen Datei.
///
/// [seen] schreibt die Marke hinein; ohne sie gibt es die Datei nicht, und der
/// Hinweis gilt als noch nicht gezeigt.
Override uiStateOverride({bool seen = true}) {
  _next++;
  final File file = File('${_dir.path}/$_next-${UiStateFile.fileName}');
  if (seen) {
    file.writeAsStringSync('{"$coachMarkSeenKey": true}');
  }
  return uiStateFileProvider.overrideWithValue(UiStateFile(file.path));
}
