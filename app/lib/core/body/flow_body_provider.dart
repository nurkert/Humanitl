/// Der Rumpf hinter einem [BodyRef], so wie `GetBody` ihn liefert: geholt,
/// begrenzt und gemerkt.
///
/// Das ist der eine Rumpf-Provider der App (`backlog/CONVENTIONS.md` 3.9).
/// Warteschlange und History holen ihre Bytes hier und nirgends sonst; er
/// liegt in `core`, weil ein Feature kein anderes importiert
/// (`docs/ARCHITECTURE.md` 5), und dort bei der Ansicht statt in `core/ipc`,
/// weil er deren Typen liefert ([RawBody], [BodyKind]): Der Transport
/// importiert nichts aus der Ansicht. Was aus den Bytes wird, entscheidet
/// `body_providers.dart`: erst dort stehen die Kopfzeilen und die Funde der
/// Seite, zu der der Rumpf gehört.
///
/// Zwei Dinge, die dieser Weg gegen einen feindlichen Rumpf tut:
///
/// * **Er hört auf zu lesen.** Über [bodyMaxBytes] wird der Strom abgebrochen;
///   der Rumpf gilt als [BodyKind.tooLarge], und was schon da ist, bleibt
///   lesbar.
/// * **Er verschweigt keinen Abbruch.** Bricht der Strom vor der angekündigten
///   Größe ab, trägt das Ergebnis [RawBody.short] und kommt nie als leerer
///   Rumpf zurück. „Leer" und „nicht lesbar" sind zwei Aussagen, und die
///   zweite darf nie wie die erste aussehen.
/// * **Er sagt weiter, was noch auf den Bytes liegt.** `BodyChunk.encoding_left`
///   landet in [RawBody.encodingLeft]; nur daran erkennt die Ansicht, ob eine
///   Fundstelle in diese Bytes zeigt (HUM-119).
library;

import 'dart:collection';
import 'dart:typed_data';

import 'package:riverpod_annotation/riverpod_annotation.dart';

import 'body_decode.dart';
import 'body_kind.dart';
import '../domain/domain.dart';
import '../ipc/client_providers.dart';
import '../ipc/daemon_client.dart';

part 'flow_body_provider.g.dart';

/// Wie viele Rümpfe der Zwischenspeicher hält.
const int bodyCacheEntries = 32;

/// Wie viele Bytes der Zwischenspeicher hält.
const int bodyCacheBytes = 64 * 1024 * 1024;

/// Der Zwischenspeicher: die letzten [bodyCacheEntries] Rümpfe, höchstens
/// [bodyCacheBytes] zusammen.
///
/// Gemerkt wird über den Digest **und** über alles, was den Verweis sonst noch
/// ausmacht: zwei Anfragen mit demselben Inhalt sind derselbe Rumpf, aber
/// derselbe Digest mit anderem `Content-Type` oder anderer Kürzung ist eine
/// andere Anzeige. Dazu gehört seit HUM-119 auch `decoded`: Derselbe Digest
/// einmal gepackt und einmal entpackt sind zwei Anzeigen, und ohne das Feld
/// im Schlüssel zeigte die eine die Bytes der anderen.
class BodyCache {
  final LinkedHashMap<String, RawBody> _entries =
      LinkedHashMap<String, RawBody>();
  int _bytes = 0;

  /// Wie viele Rümpfe gerade gemerkt sind.
  int get length => _entries.length;

  /// Wie viele Bytes gerade gemerkt sind.
  int get bytes => _bytes;

  /// Der Rumpf zu [key], oder null. Ein Treffer wird der jüngste Eintrag.
  RawBody? read(String key) {
    final RawBody? load = _entries.remove(key);
    if (load != null) {
      _entries[key] = load;
    }
    return load;
  }

  /// Merkt [load] unter [key] und wirft heraus, was über die Grenzen geht.
  void write(String key, RawBody load) {
    final RawBody? old = _entries.remove(key);
    if (old != null) {
      _bytes -= old.weight;
    }
    _entries[key] = load;
    _bytes += load.weight;
    while (_entries.length > bodyCacheEntries ||
        (_bytes > bodyCacheBytes && _entries.length > 1)) {
      final String oldest = _entries.keys.first;
      _bytes -= _entries.remove(oldest)!.weight;
    }
  }
}

/// Der Zwischenspeicher der Sitzung.
@Riverpod(keepAlive: true)
BodyCache bodyCache(Ref ref) => BodyCache();

/// Die Bytes hinter [reference], so wie der Transport sie liefert.
@riverpod
Future<RawBody> flowBody(Ref ref, BodyRef reference) async {
  if (reference.isEmpty) {
    return RawBody.none;
  }
  final BodyCache cache = ref.watch(bodyCacheProvider);
  final String key = cacheKeyOf(reference);
  final RawBody? cached = cache.read(key);
  if (cached != null) {
    return cached;
  }
  final DaemonClient client = ref.watch(daemonClientProvider);
  final BytesBuilder buffer = BytesBuilder(copy: false);
  bool overflowed = false;
  bool ended = false;
  String encodingLeft = '';
  await for (final BodyChunk chunk in client.getBodyChunks(reference)) {
    encodingLeft = chunk.encodingLeft;
    ended = chunk.last;
    buffer.add(chunk.data);
    if (buffer.length > bodyMaxBytes) {
      overflowed = true;
      break;
    }
  }
  final Uint8List received = buffer.takeBytes();
  // Hat der Daemon wirklich ausgepackt? Nur dann zählt `size` etwas anderes
  // als das, was ankommt: die gepackte Länge.
  final bool unpacked =
      reference.decoded &&
      reference.contentEncoding.isNotEmpty &&
      encodingLeft.isEmpty;
  final RawBody raw = RawBody(
    bytes: received,
    encodingLeft: encodingLeft,
    overflowed: overflowed,
    // Zwei Wege, auf denen weniger ankommt als angekündigt, und beide müssen
    // gesagt werden:
    //
    // * Der Strom endet ohne sein letztes Stück. Das ist die einzige Auskunft,
    //   die auch für einen entpackten Rumpf gilt -- dort hat `size` keine
    //   Aussagekraft mehr.
    // * Die Bytes sind die der Leitung und es sind weniger, als der Verweis
    //   nennt. Gilt für jeden rohen Abruf, auch für einen gepackten: `decoded`
    //   ist dann nicht gesetzt, der Daemon schickt `encoding_left` leer, und
    //   trotzdem liegt die Kodierung noch auf den Bytes.
    //
    // Ein Abbruch an [bodyMaxBytes] ist keines von beidem: Dort hat die
    // Ansicht aufgehört zu lesen, nicht der Daemon zu senden.
    short:
        !overflowed &&
        (!ended ||
            (!unpacked &&
                !reference.truncated &&
                received.length < reference.size)),
  );
  cache.write(key, raw);
  return raw;
}

/// Der Schlüssel, unter dem [reference] gemerkt wird.
String cacheKeyOf(BodyRef reference) {
  final StringBuffer buffer = StringBuffer();
  for (final int byte in reference.sha256) {
    buffer.write(byte.toRadixString(16).padLeft(2, '0'));
  }
  return '$buffer:${reference.size}:${reference.truncated}'
      ':${reference.contentType}:${reference.contentEncoding}'
      ':${reference.decoded}';
}
