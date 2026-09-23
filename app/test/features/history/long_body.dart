// Ein JSON-Rumpf, dessen Baum mehr als zehn Zeilen hat (HUM-153).
//
// Die Rümpfe des History-Szenarios haben zwei Schlüssel; ihr Baum ist drei
// Zeilen hoch und sagt nichts darüber, wie viel vom Rumpf ohne Scrollen zu
// sehen ist. Dieser hier ersetzt den Anfrage-Rumpf eines Flows im Fake,
// ohne `fake_daemon_client.dart` anzufassen: Verweis und Bytes passen
// zueinander, so wie der Fake sie selbst ablegt.

import 'dart:convert';
import 'dart:typed_data';

import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

/// Die Schlüssel der obersten Ebene, in der Reihenfolge des Rumpfs.
///
/// Zwölf, damit der aufgeklappte Wurzelknoten mit ihnen dreizehn Zeilen
/// ergibt: mehr, als bei 1400 × 900 hineinpassen müssen.
const List<String> longJsonKeys = <String>[
  'title',
  'head',
  'base',
  'draft',
  'milestone',
  'labels',
  'assignees',
  'reviewers',
  'maintainer_can_modify',
  'repository',
  'client',
  'request_id',
];

/// Die Bytes des Rumpfs: ein Pull Request, wie ein Agent ihn anlegt.
Uint8List longJsonBody() => Uint8List.fromList(
  utf8.encode(
    jsonEncode(<String, Object?>{
      'title': 'Split the history detail into two columns',
      'head': 'feature/history-split',
      'base': 'main',
      'draft': false,
      'milestone': 4,
      'labels': <String>['ui', 'history'],
      'assignees': <String>['niko'],
      'reviewers': <String>['codex', 'antigravity'],
      'maintainer_can_modify': true,
      'repository': 'acme/widgets',
      'client': 'opencode/1.18.25',
      'request_id': '5f0c2a9e-1b7d-4c3e-9a41-7d2e8b6f0c11',
    }),
  ),
);

/// Legt [longJsonBody] als Anfrage-Rumpf des Flows [id] ab.
///
/// Der Verweis behält seinen Schlüssel und bekommt Größe und Art der neuen
/// Bytes; der Fake liefert unter demselben Schlüssel dann genau diese. Die
/// Zeile bekommt dieselbe Größe: Der Recorder schreibt `request_size` aus
/// dem Rumpf (`daemon/crates/recorder/src/writer.rs`), und Kopf und
/// Rumpftitel dürfen sich nicht widersprechen (`backlog/CONVENTIONS.md`
/// 4.13).
void recordLongJsonRequest(FakeDaemonClient client, FlowId id) {
  final FlowDetail detail = client.state.details[id]!;
  final Uint8List bytes = longJsonBody();
  final BodyRef reference = detail.request!.body.copyWith(
    size: bytes.length,
    contentType: 'application/json',
  );
  client.state.bodies[reference.sha256
          .map((int byte) => byte.toRadixString(16).padLeft(2, '0'))
          .join()] =
      bytes;
  final Flow row = client.state.flows[id]!.copyWith(requestSize: bytes.length);
  client.state.flows[id] = row;
  client.state.details[id] = detail.copyWith(
    summary: row,
    request: detail.request!.copyWith(body: reference),
  );
}
