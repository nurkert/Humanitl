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
/// andere Anzeige.
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
  await for (final Uint8List chunk in client.getBody(reference)) {
    buffer.add(chunk);
    if (buffer.length > bodyMaxBytes) {
      overflowed = true;
      break;
    }
  }
  final Uint8List received = buffer.takeBytes();
  final RawBody raw = RawBody(
    bytes: received,
    overflowed: overflowed,
    short: !reference.truncated && received.length < reference.size,
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
      ':${reference.contentType}';
}
