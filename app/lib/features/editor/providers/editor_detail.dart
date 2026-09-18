/// Das Detail einer Anfrage, wie der Editor es liest (HUM-047).
///
/// Ein eigener Provider und keine Anleihe bei der Warteschlange: Kein Feature
/// liest die Provider eines anderen (`docs/ARCHITECTURE.md` 5). Dieselbe
/// Entscheidung steht schon in `core/body/body_providers.dart` — „Warteschlange
/// und History lesen ihr Detail aus je eigenem Provider" —, und der Editor ist
/// der dritte Leser desselben RPC.
///
/// Der Preis ist ein zweiter `GetFlow` je Fluss, den jemand bearbeitet. Das ist
/// ein Aufruf pro geöffnetem Editor über einen Unix-Socket, und er kauft die
/// Richtung der Abhängigkeiten frei.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';

part 'editor_detail.g.dart';

/// Kopfzeilen, Query, Rumpf-Verweis und Funde der Anfrage [id].
@riverpod
Future<FlowDetail> editorDetail(Ref ref, FlowId id) =>
    ref.watch(daemonClientProvider).getFlow(id);
