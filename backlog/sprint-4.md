# Sprint 4 · Trusted Editor (M4)

Ziel des Sprints: Der Nutzer kann eine gehaltene Anfrage pseudonymisieren, bevor sie rausgeht. Alles, was passiert, landet in einer prüfbaren Audit-Kette. Die App spricht Deutsch und Englisch, ist über einen generierten Settings-Screen vollständig konfigurierbar und lässt sich als `.deb` und AppImage installieren. Demo-Skript M4 (HUM-055) ist am Sprintende in CI grün.

Voraussetzungen aus früheren Sprints: `humanitl-core` mit `Finding`, `Diagnostic`, `FlowState` (HUM-004, HUM-063), `humanitl-findings` Tier 1 (HUM-025), `humanitl-recorder` mit Schema aus BACKLOG.md 3.4 (HUM-026), `humanitl-config` mit Schema und Tiers (HUM-062), CLI-Grundgerüst (HUM-064), Intercept-Screen mit Aktionsleiste (HUM-020, HUM-028), Body-Ansichten (HUM-030).

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-047 | Pseudonymisierungs-Editor | L | HUM-025, HUM-028, HUM-030 |
| HUM-048 | Pseudonym-Mapping und Schlüsselverwaltung | M | HUM-026, HUM-047 |
| HUM-049 | Senden mit offenen Findings | S | HUM-047, HUM-048, HUM-062 |
| HUM-050 | Audit-Hash-Kette | M | HUM-004, HUM-026, HUM-048 |
| HUM-051 | Audit-Screen | S | HUM-050 |
| HUM-052 | i18n Deutsch und Englisch | M | HUM-019 |
| HUM-069 | Settings-Screen mit Progressive Disclosure | L | HUM-062, HUM-052 |
| HUM-070 | CLI config, audit, daemon | S | HUM-064, HUM-050, HUM-062 |
| HUM-077 | Ein-Klick-Installation | M | HUM-053, HUM-075 |
| HUM-078 | Paritäts-Tabelle und CI-Check | S | HUM-070, HUM-059 |
| HUM-079 | Rücktausch von Pseudonymen in Text-Antworten | M | HUM-048 |
| HUM-053 | Packaging deb, AppImage, systemd | M | HUM-070 |
| HUM-054 | Golden- und Widget-Tests | M | HUM-047, HUM-052, HUM-069 |
| HUM-055 | Demo-Skript M4 | S | alle oben |

Proto-Ergänzungen in diesem Sprint (Minor-Version `humanitl.v1` bleibt, neue RPCs sind additiv): `Pseudonyms`, `Config` (falls nicht schon in HUM-062 definiert, siehe Fallstricke von HUM-069), Erweiterung von `DecideRequest` um `acknowledged_findings` und `ignore_always`.

---

> **Abgleich 2026-09-02**: Config-Gruppe `limits.*` (HUM-057) ist die Heimat aller Caps; `hold.body_cap_bytes`, `preview.cap_bytes`, `ipc.event_buffer` bleiben als serde-Aliase gültig. Settings-Screen (HUM-069) rendert `limits` als eigene Gruppe. Fake-Szenarien in Flutter über `--dart-define=HUMANITL_FAKE=<scenario>`. `packages/ui` hat `HModal` (HUM-008), alle Dialoge gehen darüber. Hilfs-Crate `daemon/xtask` für Doku-Generierung.

## HUM-047 · Pseudonymisierungs-Editor
Sprint: 4 · Größe: L · Abhängigkeiten: HUM-025, HUM-028, HUM-030 · Blockiert: HUM-048, HUM-049, HUM-055

### Kontext
Setzt BACKLOG.md Abschnitt 5 (Signature-Element „Diff-Glow", Editor-UX) und ADR-005 um. Der Nutzer sieht in einer gehaltenen Anfrage Kundendaten oder Secrets und will sie ersetzen, bevor die Anfrage rausgeht. Bisher (HUM-028) gibt es nur „Allow" und „Block"; „Edit+Allow" öffnet noch nichts. Dieses Issue liefert den Editor als eigenen Bereich im Intercept-Screen und den `AllowEdited`-Pfad im Daemon.

### Ziel
Aus der Aktionsleiste öffnet `E` (oder der Button „Edit") für den selektierten Flow den Editor anstelle der Request-Karte. Links steht das Original (read-only), rechts der editierbare Entwurf. Eine Findings-Leiste über dem Editor listet alle Funde nach Typ. Ein Klick auf „Alle durch Pseudonyme ersetzen" ersetzt jeden Fund durch einen stabilen Platzhalter, ersetzte Stellen leuchten mit Diff-Glow. Jeder Fund kann einzeln ersetzt, für alle Vorkommen ersetzt oder ignoriert werden. Eine manuelle Textauswahl kann mit `Ctrl+R` pseudonymisiert werden. Der Button „Editierte Version senden" schickt den Entwurf als `AllowEdited` an den Daemon; der Daemon prüft, dass Host und Port unverändert sind, berechnet `Content-Length` neu und leitet weiter. Die Queue-Zeile und die History-Zeile tragen danach den Chip „Edited".

### Nicht-Ziel
- Persistente Speicherung und Verschlüsselung des Mappings, Mapping-Panel, Export: HUM-048. In diesem Issue lebt das Mapping nur im `draftProvider` im Speicher.
- Der Warn-Flow beim Senden mit offenen Findings: HUM-049.
- Editieren von Binär-Bodies, Multipart-Bodies mit Datei-Teilen, Bodies über `preview.cap_bytes`: Editor zeigt Hinweis, Button deaktiviert.
- Response-Editing: nicht im MVP.

### Betroffene Pfade
- `app/lib/features/editor/editor_screen.dart` (neu)
- `app/lib/features/editor/widgets/findings_rail.dart` (neu)
- `app/lib/features/editor/widgets/draft_editor.dart` (neu)
- `app/lib/features/editor/widgets/original_view.dart` (neu)
- `app/lib/features/editor/widgets/header_table.dart` (neu)
- `app/lib/features/editor/widgets/mapping_strip.dart` (neu, minimal, wird in HUM-048 ausgebaut)
- `app/lib/features/editor/providers/draft_provider.dart` (neu)
- `app/lib/features/editor/model/draft.dart` (neu, freezed)
- `app/lib/features/editor/model/draft_ops.dart` (neu, reine Funktionen)
- `app/lib/features/editor/model/pseudonym_naming.dart` (neu)
- `app/lib/features/intercept/widgets/action_bar.dart` (ändern: `EditIntent` öffnet Editor, Button-Zustand „Editierte Version senden")
- `app/lib/features/intercept/intercept_screen.dart` (ändern: Mittel-Pane zeigt Editor statt Karte, wenn `editorOpenProvider` true)
- `app/lib/core/domain/http_request.dart` (ändern oder neu: `HttpRequestDraft` Serialisierung nach Proto)
- `app/packages/ui/lib/src/h_editor_decorations.dart` (neu)
- `daemon/crates/proxy/src/edit.rs` (neu)
- `daemon/crates/proxy/src/hold.rs` (ändern: `Decision::AllowEdited` verarbeiten)
- `daemon/crates/ipc/src/decide.rs` (ändern: `HttpRequestProto` nach `HttpRequest` mit Validierung)
- `proto/humanitl/v1/humanitl.proto` (ändern: `DecideRequest.edited`, `HttpRequestProto`)
- `app/l10n/app_en.arb`, `app/l10n/app_de.arb` (ändern, Schlüssel mit Präfix `editor`)

### Spezifikation

**Dart-Modell `Draft`** (`draft.dart`, freezed):

```dart
@freezed
class Draft with _$Draft {
  const factory Draft({
    required FlowId flowId,
    required String method,                 // editierbar, Uppercase
    required String scheme,                 // gesperrt
    required String host,                   // gesperrt (Authority)
    required int port,                      // gesperrt
    required String pathAndQuery,           // editierbar
    required List<HeaderEntry> headers,     // editierbar, außer locked
    required String body,                   // dekodierter Text, UTF-8
    required BodyKind bodyKind,             // text | json | form | binary | tooLarge
    required List<FindingView> findings,    // aus Analyzed-Event, mit Status
    required List<Replacement> replacements,// angewandte Ersetzungen in Reihenfolge
    required Map<String, String> pseudonyms,// valueHashHex -> pseudonym (Session-Mapping, HUM-048 macht es persistent)
    @Default(false) bool dirty,
  }) = _Draft;
}

@freezed
class HeaderEntry with _$HeaderEntry {
  const factory HeaderEntry({required String name, required String value, @Default(false) bool locked}) = _HeaderEntry;
}

enum FindingStatus { open, replaced, ignored }

@freezed
class FindingView with _$FindingView {
  const factory FindingView({
    required int index,                     // Index im Analyzed-Event, für acknowledged_findings
    required Finding finding,               // core-Spiegel: kind, span, location, tier, valueHashHex, displayPrefix
    required FindingStatus status,
    String? pseudonym,
  }) = _FindingView;
}

@freezed
class Replacement with _$Replacement {
  const factory Replacement({
    required FindingLocation location,      // header(name) | query | body
    required int start,                     // Offsets im aktuellen Entwurfstext des Ortes
    required int end,
    required String original,
    required String pseudonym,
    required String valueHashHex,
  }) = _Replacement;
}
```

Gesperrte Header (`locked: true`, im UI grau mit Schloss-Icon): `host`, `content-length`, `transfer-encoding`, `content-encoding`, `expect`, `connection`, `upgrade`, `proxy-*`. Der Rest ist editierbar, inklusive Löschen und Hinzufügen.

**Reine Funktionen** (`draft_ops.dart`, keine Provider-Abhängigkeit, vollständig unit-testbar):

```dart
/// Ersetzt genau ein Finding. Verschiebt Spans aller nachfolgenden Findings am selben Ort um die Längendifferenz.
Draft replaceFinding(Draft d, int findingIndex, String pseudonym);

/// Ersetzt alle offenen Findings mit demselben valueHash am selben und an anderen Orten.
Draft replaceAllOfValue(Draft d, String valueHashHex, String pseudonym);

/// Ersetzt alle offenen Findings; Pseudonyme über PseudonymNaming vergeben, gleiche valueHash = gleiches Pseudonym.
Draft replaceAllOpen(Draft d, PseudonymNaming naming);

/// Markiert ein Finding als ignoriert (bleibt in der Liste, zählt nicht als offen).
Draft ignoreFinding(Draft d, int findingIndex);

/// Manuelle Ersetzung einer Auswahl im Body (oder Header-Wert). Erzeugt ein synthetisches Finding mit kind Custom(kindLabel).
Draft replaceSelection(Draft d, FindingLocation loc, int start, int end, String kindLabel, PseudonymNaming naming);

/// Anzahl offener Findings (status == open).
int openFindings(Draft d);

/// Body-Text nach Ersetzungen; für JSON-Bodies zusätzlich Validitätsprüfung.
({String body, String? jsonError}) renderBody(Draft d);
```

Regel für Span-Verschiebung: Nach einer Ersetzung an Position `[s, e)` mit neuer Länge `n` werden alle Findings am selben `location` mit `start >= e` um `n - (e - s)` verschoben. Findings, die sich mit `[s, e)` überlappen, werden auf `ignored` gesetzt (Überlappung ist ein Regex-Artefakt, nie zwei echte Werte). Wird der Body vom Nutzer frei editiert (Tastatureingabe außerhalb einer Ersetzung), werden alle offenen Body-Findings neu berechnet: der Editor ruft nach 300 ms Debounce `findingsProvider.rescan(body)` (lokaler Dart-Regex-Scan, gleiche Regex-Quellen wie `humanitl-findings` Tier `Regex`; Tier `Checksum` bleibt daemon-seitig und wird beim Senden neu geprüft).

**Pseudonym-Namensschema** (`pseudonym_naming.dart`):

```dart
class PseudonymNaming {
  PseudonymNaming({required Map<String, String> existing, required Map<String, int> counters});
  /// Liefert das bestehende Pseudonym für valueHash oder vergibt <TYPE_n> mit n = counters[TYPE]+1.
  String nameFor(FindingKind kind, String valueHashHex);
  static String typeLabel(FindingKind kind); // EMAIL, IBAN, CARD, PHONE, IPV4, JWT, API_KEY, TERM, CUSTOM
}
```

Format ist exakt `<TYPE_n>` mit ASCII-Spitzklammern, `n` beginnt bei 1, Zähler pro Typ und Session. Bei `UserTerm` mit hinterlegtem Alias (Projekt-Setting `findings.user_terms = [{term, alias}]`, kommt aus HUM-025) wird der Alias verwendet, z. B. `Client-A`, sonst `<TERM_n>`.

**Editor-Widget-Baum** (`editor_screen.dart`):

```
EditorScreen(flowId)
└─ Column
   ├─ EditorHeader                         Method-Badge, Authority (gesperrt, Schloss), Pfad (editierbar, Mono), „Zurück" (Esc)
   ├─ FindingsRail                         horizontale Chip-Leiste: [EMAIL ×2] [IBAN ×1] [API_KEY ×1] · Buttons „Alle ersetzen" · „Nächstes" (springt)
   ├─ HSplitView (ResizablePanel, 50/50, min 320 je Seite)
   │  ├─ OriginalView                       re_editor read-only, gleiche Zeilenumbrüche, Findings unterstrichen
   │  └─ DraftEditor                        re_editor editierbar
   │       ├─ Tabs: Body | Headers | Query
   │       ├─ Body: CodeEditor mit Findings- und Glow-Decorations
   │       ├─ Headers: HeaderTable (Name/Wert, locked-Zeilen grau, + Hinzufügen, Löschen)
   │       └─ Query: Key/Value-Tabelle aus pathAndQuery, Änderung schreibt pathAndQuery zurück
   ├─ MappingStrip                          eingeklappt: „Mapping (3)" · ausgeklappt: Pseudonym · Typ · maskiertes Original
   └─ EditorActionBar                       [Editierte Version senden] (Primär, Stift-Icon) · [Verwerfen] · offene Findings als Zähler
```

Decorations: Über `re_editor`s Span-Builder-Hook (Parameter `spanBuilder` von `CodeEditor`; Signatur gegen die gepinnte Version prüfen) werden pro sichtbarer Zeile `TextSpan`s erzeugt. `HEditorDecorations` (in `packages/ui`) liefert dafür drei Stile: `finding.secret` (Unterstrich `#F0784F`, 1 px, wellig), `finding.pii` (Unterstrich `#E0B24A`), `replaced` (Diff-Glow: Hintergrund Akzent 10 % Alpha, Unterstrich Akzent 1 px). Hover auf `replaced` zeigt ein Popover `Original ↔ Pseudonym` (Original maskiert wie in HUM-048 definiert, hier vorläufig: erste 2 und letzte 2 Zeichen, Rest `*`). Hover auf einem Finding zeigt Popover mit Typ, Tier, Buttons „Ersetzen", „Alle mit diesem Wert ersetzen", „Ignorieren", „Immer ignorieren" (letzterer erst mit HUM-049 aktiv, hier deaktiviert mit Tooltip).

Keyboard im Editor: `Ctrl+R` bei aktiver Auswahl öffnet ein kleines Popover „Pseudonymisieren als …" mit Typ-Auswahl (Default `CUSTOM`, Freitext für Label), Enter bestätigt. `Ctrl+Enter` sendet. `Esc` zurück zur Karte (Entwurf bleibt erhalten). `F3` nächstes Finding. Alle als `Intent`-Klassen: `PseudonymizeSelectionIntent`, `SendEditedIntent`, `CloseEditorIntent`, `NextFindingIntent`.

**Provider** (`draft_provider.dart`):

```dart
@riverpod
class DraftNotifier extends _$DraftNotifier {
  @override Draft build(FlowId id);            // initial aus flowsProvider[id] + flowBodyProvider(bodyRef); bodyKind bestimmen
  void replace(int index, String pseudonym);
  void replaceAllOfValue(String hash, String pseudonym);
  void replaceAllOpen();
  void ignore(int index);
  void replaceSelection(FindingLocation loc, int start, int end, String kindLabel);
  void setBody(String body);                    // freie Eingabe, setzt dirty, triggert Rescan mit Debounce
  void setHeader(int i, String name, String value); void addHeader(); void removeHeader(int i);
  void setPathAndQuery(String s); void setMethod(String m);
}
final editorOpenProvider = StateProvider<bool>((_) => false);
```

Entwürfe sind pro `FlowId` (Riverpod-Family, `keepAlive` bis der Flow den Zustand `Recorded` erreicht oder `TimedOut` ist; bei `TimedOut` bleibt der Entwurf lesbar mit Banner „Angehalten, Zeit abgelaufen, blockiert", siehe HUM-058).

**Senden**: `EditorActionBar` ruft `daemonClient.decide(DecideRequest(flowId, decision: ALLOW_EDITED, edited: draft.toProto()))`. `Draft.toProto()`:
- `method`: Uppercase, muss `^[A-Z]{1,16}$` matchen.
- `authority`: unverändert aus dem Original.
- `path_and_query`: muss mit `/` beginnen, keine Leerzeichen (URL-encodiert).
- `headers`: alle nicht-gesperrten Einträge, plus die gesperrten Werte aus dem Original **außer** `content-length`, `transfer-encoding`, `content-encoding`, die der Daemon setzt.
- `body`: UTF-8-Bytes von `renderBody().body`. Bei `bodyKind == json` und `jsonError != null` zeigt die Leiste einen amber Hinweis „Body ist kein gültiges JSON", Senden bleibt möglich.
- `replacements`: Liste `{location, start, end, value_hash, pseudonym}` (ohne Original), damit der Daemon `findings.resolved` setzen und das Audit-Log füllen kann.

**Proto-Änderung**:

```proto
message HttpRequestProto {
  string method = 1;
  string scheme = 2;            // "http" | "https"
  string host = 3;              // normalisiert
  uint32 port = 4;
  string path_and_query = 5;
  repeated Header headers = 6;  // message Header { string name = 1; string value = 2; }
  bytes body = 7;               // vollständiger Body, nie BodyRef (Edits sind < preview.cap_bytes)
}
message ReplacementProto { FindingLocationProto location = 1; uint32 start = 2; uint32 end = 3; bytes value_hash = 4; string pseudonym = 5; }
message DecideRequest {
  string flow_id = 1;
  DecisionKind decision = 2;    // ALLOW, ALLOW_EDITED, BLOCK
  HttpRequestProto edited = 3;  // nur bei ALLOW_EDITED
  repeated ReplacementProto replacements = 4;
  RuleProto remember = 5;       // bestehend aus HUM-028
  repeated uint32 acknowledged_findings = 6;   // HUM-049
  repeated uint32 ignore_always = 7;           // HUM-049
}
```

**Daemon-Seite** (`proxy/src/edit.rs`):

```rust
pub fn apply_edit(original: &HttpRequest, edited: EditedRequest) -> Result<HttpRequest, Diagnostic>
```

Prüfungen in dieser Reihenfolge, jede liefert ein `Diagnostic` mit Severity `Error`:
1. `EDIT_001` Authority verändert (`host` oder `port` weichen ab, Vergleich nach Normalisierung). `why`: „Die Zieladresse einer gehaltenen Anfrage darf nicht geändert werden, sonst wäre die Regelprüfung wertlos." Kein `fix`.
2. `EDIT_002` Methode ungültig (Regex oben).
3. `EDIT_003` Pfad ungültig (kein führendes `/`, Steuerzeichen, Leerzeichen).
4. `EDIT_004` Gesperrter Header im Edit enthalten (`host`, `content-length`, `transfer-encoding`, `content-encoding`, `expect`): wird nicht als Fehler behandelt, sondern still verworfen und im Tracing geloggt; Daemon setzt die Werte selbst.
5. `EDIT_005` Body größer als `preview.cap_bytes`.

Danach: `content-length` = Bytelänge des neuen Bodys (auch `0` bei leerem Body, außer Methode ist `GET`/`HEAD` und Body leer, dann kein `content-length`); `transfer-encoding` entfernt; `content-encoding` entfernt (der Editor arbeitet immer auf dem dekodierten Body, HUM-030 dekodiert gzip/br/deflate für die Anzeige, und der Daemon sendet den editierten Body unkomprimiert). `expect` entfernt (der Daemon hat den Body bereits vollständig). Danach läuft `humanitl-findings` noch einmal über den editierten Request; das Ergebnis ersetzt die Findings des Flows (Zustand bleibt `Decided(AllowEdited)`, Event `Analyzed` wird nicht erneut emittiert, stattdessen trägt `Decided` das Feld `remaining_findings: u32`). Der Flow geht nach `Forwarded`.

Audit (Vorgriff auf HUM-050, hier nur das `FlowEvent`): `Decided { decision: AllowEdited, edited: true, replacements: n, remaining_findings: m }`. Originalwerte stehen nie im Event.

### Schritte
1. Proto erweitern (`HttpRequestProto`, `ReplacementProto`, `DecideRequest`), `buf lint`, Codegen Rust und Dart läuft. Kompiliert.
2. `draft.dart`, `draft_ops.dart`, `pseudonym_naming.dart` mit freezed anlegen. Unit-Tests aus Abschnitt Tests grün, ohne Widget.
3. `draft_provider.dart` mit Family und Rescan-Debounce. Provider-Tests grün.
4. `HEditorDecorations` in `packages/ui`; Galerie-Seite (HUM-008) zeigt drei Beispielzeilen.
5. `EditorScreen` mit `OriginalView` und `DraftEditor` (nur Body-Tab). Gegen Fake-Daemon: Editor öffnet, Findings unterstrichen, „Alle ersetzen" glüht.
6. `HeaderTable`, Query-Tab, `MappingStrip`, `FindingsRail` mit Springen.
7. `Ctrl+R`-Popover und die vier Intents.
8. Daemon `edit.rs` mit `apply_edit`, Tests grün. `ipc/decide.rs` mappt Proto und ruft `hold.decide(id, Decision::AllowEdited{request})`.
9. `hold.rs`: `AllowEdited` weiterleiten, Findings neu scannen, `Decided`-Event mit `edited: true`.
10. Aktionsleiste: nach Senden wechselt die Karte in den Zustand `allowedEdited`, Queue- und History-Zeile zeigen Chip „Edited".
11. Widget-Test „edit and send" gegen Fake-Daemon grün, e2e-Fall in HUM-055 vorbereitet.

### Tests
Unit (`app/test/features/editor/draft_ops_test.dart`):
- `replaceFinding_shiftsLaterSpans`: Body `"a@x.de und b@y.de"`, zwei EMAIL-Findings, ersetze erstes durch `<EMAIL_1>`; zweites Finding hat neuen Start = alter Start + (`<EMAIL_1>`.length − `a@x.de`.length).
- `replaceFinding_overlapIgnored`: zwei überlappende Spans, Ersetzen des ersten setzt das zweite auf `ignored`.
- `replaceAllOfValue_sameHashSamePseudonym`: derselbe Wert in Header und Body, beide werden `<EMAIL_1>`.
- `replaceAllOpen_countersPerType`: 2 E-Mails, 1 IBAN ⇒ `<EMAIL_1>`, `<EMAIL_2>`, `<IBAN_1>`.
- `replaceAllOpen_userTermAlias`: Term „Müller GmbH" mit Alias „Client-A" ⇒ Body enthält `Client-A`.
- `replaceSelection_createsCustomFinding`: Auswahl `[10, 16)` mit Label `PROJECT` ⇒ `<PROJECT_1>`, Finding mit `kind: Custom("PROJECT")`, Status `replaced`.
- `renderBody_jsonInvalidAfterEdit`: Body `{"a":"x@y.de"}`, ersetze so, dass Anführungszeichen kaputtgehen ⇒ `jsonError != null`.
- `openFindings_excludesIgnoredAndReplaced`.
- `toProto_dropsLockedHeaders`: Draft mit `content-length` im Header-Set ⇒ Proto enthält es nicht.

Unit (`daemon/crates/proxy/src/edit.rs`):
- `authority_change_rejected`: Original `api.github.com:443`, Edit `evil.io:443` ⇒ `EDIT_001`.
- `authority_case_insensitive_ok`: `API.GITHUB.COM.` ⇒ ok.
- `content_length_recomputed`: Body 17 Bytes UTF-8 (mit Umlaut) ⇒ `content-length: 17`, nicht Zeichenanzahl.
- `transfer_encoding_removed`, `content_encoding_removed`, `expect_removed`.
- `get_with_empty_body_no_content_length`.
- `body_over_cap_rejected` ⇒ `EDIT_005`.
- `findings_rescanned_after_edit`: Edit entfernt eine E-Mail, lässt IBAN ⇒ `remaining_findings == 1`.

Widget (`app/test/features/editor/editor_screen_test.dart`, Fake-Daemon):
- `open_with_E_shows_editor`, `replace_all_glows` (findet 3 Widgets mit `replaced`-Decoration), `send_edited_calls_decide_with_ALLOW_EDITED` (Fake-Daemon zeichnet Aufruf auf), `esc_keeps_draft` (Editor schließen, wieder öffnen, Entwurf unverändert), `binary_body_disables_editor`.

### Akzeptanzkriterien
- [ ] `E` auf einer gehaltenen Anfrage öffnet den Editor im mittleren Pane, `Esc` kehrt zur Karte zurück, Entwurf bleibt.
- [ ] „Alle ersetzen" ersetzt jeden offenen Fund, gleiche Werte bekommen dasselbe Pseudonym, Format `<TYPE_n>`.
- [ ] Ersetzte Stellen sind mit Diff-Glow markiert, Hover zeigt maskiertes Original und Pseudonym.
- [ ] `Ctrl+R` auf einer Auswahl erzeugt ein `<CUSTOM_n>` oder gelabeltes Pseudonym.
- [ ] `Host` und `Content-Length` sind im Header-Tab sichtbar gesperrt.
- [ ] Senden mit geändertem Host ist im UI unmöglich und wird vom Daemon mit `EDIT_001` abgelehnt (Test).
- [ ] Nach dem Senden trägt die Anfrage in Queue-Abgang, History und Detail den Chip „Edited", Zustand `allowedEdited`.
- [ ] Der Upstream erhält `content-length` passend zur Bytelänge, kein `transfer-encoding`, kein `content-encoding` (Integrationstest mit axum-Fake-Upstream aus HUM-017).
- [ ] Alle Tests aus dem Abschnitt Tests grün, `flutter analyze` und `cargo clippy -D warnings` sauber.
- [ ] Neue ARB-Schlüssel `editor*` in `en` und `de`.

### Fallstricke
- `Content-Length` ist die **Byte**-Länge des UTF-8-Bodys, nicht die Zeichenanzahl. Dart `String.length` zählt UTF-16-Einheiten. Immer `utf8.encode(body).length`.
- Bodies, die als `chunked` ankamen, haben keinen `content-length`; nach dem Edit muss einer gesetzt und `transfer-encoding` entfernt werden. Beides zusammen ist ein HTTP-Fehler (RFC 9112 6.1).
- Komprimierte Request-Bodies (`content-encoding: gzip`) sind selten, aber real (z. B. Sentry-Clients). Der Editor arbeitet auf dem dekodierten Text; der Daemon sendet unkomprimiert. Manche Server erwarten dann trotzdem Erfolg, andere nicht. Tracing-Warnung `edit.content_encoding_dropped`.
- Findings-Spans beziehen sich auf den dekodierten Body und auf Byte-Offsets im Daemon (`Range<usize>` über Bytes), im Dart-Editor aber auf UTF-16-Code-Unit-Offsets. Beim Laden des Drafts einmal konvertieren (Byte-Offset → Code-Unit-Offset über einen Präfix-Scan), beim Senden zurück. Test mit Umlaut vor dem Finding.
- `re_editor` rendert nur sichtbare Zeilen; Decorations dürfen nicht auf globalen Offsets über alle Zeilen rechnen, sondern müssen pro Zeile aus dem Zeilen-Offset ableiten. Zeilenstart-Offsets einmal berechnen und cachen, bei jeder Änderung invalidieren.
- Header-Namen sind case-insensitiv. `locked`-Prüfung immer auf Lowercase.
- Query-Tab und Pfad-Feld sind zwei Sichten auf `pathAndQuery`; niemals beide getrennt speichern. Query-Werte beim Zurückschreiben URL-encoden.
- Die Authority-Prüfung im Daemon ist die Sicherheitsgrenze, nicht das gesperrte UI-Feld. Ein manipulierter Client darf nicht umleiten können.
- Bei `TimedOut` während des Editierens keine Exception, Entwurf bleibt, Senden ist deaktiviert mit Banner (HUM-058 liefert das Banner, hier nur der deaktivierte Zustand).
- `keepAlive` auf Family-Providern sammelt Speicher; beim Übergang nach `Recorded` explizit `ref.invalidate` auslösen.

### Referenzen
- BACKLOG.md Abschnitt 5 (Editor-UX, Signature-Element Diff-Glow), ADR-005, ADR-008
- `backlog/CONVENTIONS.md` 3.2 (`Finding`, `FindingKind`), 3.9 (Provider, Intents)
- re_editor: https://pub.dev/packages/re_editor
- RFC 9112 Abschnitt 6.1 (Transfer-Encoding und Content-Length): https://www.rfc-editor.org/rfc/rfc9112#section-6.1
- Caido „Edited"-Zustand: https://docs.caido.io/app/guides/intercept_traffic

---

## HUM-048 · Pseudonym-Mapping und Schlüsselverwaltung
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-026, HUM-047 · Blockiert: HUM-049, HUM-050, HUM-055

### Kontext
Setzt ADR-008 (Mapping getrennt, verschlüsselt, nur host-seitig) und BACKLOG.md 4.2 sowie den DSGVO-Hinweis um: Was der Editor tut, ist Pseudonymisierung (Art. 4 Nr. 5 DSGVO), keine Anonymisierung. Die Zuordnung Pseudonym ↔ Original ist selbst personenbezogen und muss geschützt sein. Gleichzeitig muss sie stabil und vollständig sein, damit der Nutzer die Antworten des Agenten lesen kann und damit M8 (De-Pseudonymisierung von Responses) später ohne Datenmodell-Änderung möglich ist. Dieses Issue führt außerdem den `KeyStore` ein, den HUM-050 für den Audit-HMAC mitbenutzt.

### Ziel
Jedes Pseudonym, das der Editor vergibt, wird im Daemon in der Tabelle `pseudonyms` gespeichert: Session, Pseudonym, Typ, `value_hash`, verschlüsseltes Original (bei Secrets nur Präfix), erstes Auftreten, Zähler. Derselbe Wert erhält innerhalb einer Session immer dasselbe Pseudonym, auch über mehrere Anfragen hinweg. Der Editor bekommt die Zuordnung vom Daemon, nicht umgekehrt: Bevor der Editor ersetzt, fragt er `Pseudonyms(Resolve)` an und der Daemon vergibt den Namen. Ein Mapping-Panel im Editor und im History-Detail zeigt die Tabelle mit maskiertem Original. Ein Export liefert die Tabelle als verschlüsselte Datei. Alle Schlüssel kommen aus einem Master-Key im System-Keyring mit dokumentiertem Fallback.

### Nicht-Ziel
- Rückübersetzung von Responses (M8).
- Team-weite oder projektübergreifende Pseudonyme; MVP ist pro Session.
- Löschen einzelner Mapping-Einträge im UI; Löschen erfolgt über Retention (HUM-051).

### Betroffene Pfade
- `daemon/crates/recorder/src/keys.rs` (neu): `KeyStore`
- `daemon/crates/recorder/src/pseudonyms.rs` (neu)
- `daemon/crates/recorder/migrations/V4__pseudonyms_and_allowlist.sql` (neu; Nummer an bestehende Migrationen anpassen)
- `daemon/crates/recorder/Cargo.toml` (ändern: `keyring`, `aes-gcm`, `hkdf`, `hmac`, `sha2`, `zeroize`, `rand`)
- `daemon/crates/ipc/src/pseudonyms.rs` (neu)
- `daemon/bin/humanitld/src/main.rs` (ändern: `KeyStore::open()` beim Start, Diagnostic bei Fallback)
- `proto/humanitl/v1/humanitl.proto` (ändern: `rpc Pseudonyms`)
- `app/lib/features/editor/widgets/mapping_panel.dart` (neu, ersetzt `mapping_strip.dart` aus HUM-047)
- `app/lib/features/editor/providers/pseudonym_map_provider.dart` (neu, `pseudonymMapProvider`)
- `app/lib/features/history/widgets/flow_detail.dart` (ändern: Tab „Mapping" wenn Flow `edited`)
- `app/lib/core/ipc/daemon_client.dart` (ändern: `resolvePseudonyms`, `listPseudonyms`, `exportPseudonyms`)

### Spezifikation

**KeyStore** (`recorder/src/keys.rs`):

```rust
pub struct KeyStore { master: Zeroizing<[u8; 32]>, origin: KeyOrigin }
pub enum KeyOrigin { Keyring, File(PathBuf) }
pub enum Purpose { AuditHmac, ValueHash, PseudonymAead }

impl KeyStore {
    /// Öffnet oder erzeugt den Master-Key. Reihenfolge: Keyring (service "humanitl", user "master-key", Base64 von 32 Bytes)
    /// -> Datei $XDG_DATA_HOME/humanitl/keys/master.key (0600, Verzeichnis 0700) -> neu erzeugen (rand::rngs::OsRng) und
    /// im Keyring speichern; schlägt der Keyring fehl, in Datei speichern und Diagnostic KEYS_001 zurückgeben.
    pub fn open(data_dir: &Path) -> Result<(Self, Option<Diagnostic>), Diagnostic>;
    /// HKDF-SHA256(master, salt = b"humanitl-v1", info = purpose.as_bytes()) -> 32 Bytes.
    pub fn derive(&self, purpose: Purpose) -> Zeroizing<[u8; 32]>;
    pub fn origin(&self) -> &KeyOrigin;
}
```

`Purpose::as_bytes()`: `b"audit-hmac"`, `b"value-hash"`, `b"pseudonym-aead"`. Diagnostics: `KEYS_001` (Warning) „Kein System-Keyring erreichbar, Schlüssel liegt als Datei", `why`: „Der Secret Service (D-Bus) antwortet nicht. Ohne Keyring schützt nur die Dateiberechtigung 0600 den Schlüssel.", `fix: Some(FixAction::CopyCommand("sudo apt install gnome-keyring"))` (bzw. `OpenUrl` auf die Doku). `KEYS_002` (Blocking) „Schlüsseldatei hat falsche Berechtigungen", wenn Mode ≠ 0600. `KEYS_003` (Blocking) „Keyring und Datei liefern verschiedene Schlüssel" (beide vorhanden, ungleich; niemals still einen wählen).

**value_hash** (ersetzt die bisherige Definition in HUM-025, falls dort SHA-256 ohne Schlüssel verwendet wurde, siehe Fallstricke): `HMAC-SHA256(derive(ValueHash), value_bytes)`. Stabil pro Installation, damit „Immer ignorieren" (HUM-049) über Sessions hinweg funktioniert, aber ohne Schlüssel nicht per Wörterbuch angreifbar. `humanitl-findings` erhält den Schlüssel als `&[u8; 32]` beim Bau des `Scanner`.

**Migration**:

```sql
CREATE TABLE pseudonyms (
  session_id      TEXT NOT NULL,
  pseudonym       TEXT NOT NULL,
  kind            TEXT NOT NULL,                -- FindingKind als snake_case, z. B. email, api_key, user_term:client
  value_hash      BLOB NOT NULL,                -- 32 Bytes HMAC
  value_encrypted BLOB,                         -- NULL bei Secrets; sonst nonce(12) || ciphertext || tag(16)
  display_prefix  TEXT NOT NULL,                -- maskierte Anzeige, siehe unten
  first_seen      TEXT NOT NULL,                -- RFC 3339 UTC
  count           INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (session_id, value_hash),
  UNIQUE (session_id, pseudonym)
);
CREATE TABLE finding_allowlist (
  value_hash  BLOB NOT NULL,
  kind        TEXT NOT NULL,
  scope       TEXT NOT NULL,                    -- 'global' oder Projektpfad-Hash
  created     TEXT NOT NULL,
  PRIMARY KEY (value_hash, scope)
);
```

**Verschlüsselung**: AES-256-GCM (`aes-gcm`-Crate), Schlüssel `derive(PseudonymAead)`, Nonce 12 Bytes zufällig pro Eintrag, AAD = `session_id || pseudonym` (UTF-8, mit `\x00` getrennt), damit ein Ciphertext nicht in eine andere Zeile verschoben werden kann. Secrets (`FindingKind::ApiKey`, `Jwt`) werden **nie** verschlüsselt gespeichert, `value_encrypted` bleibt NULL.

**display_prefix / Maskierung** (`mask(value, kind) -> String`, einheitlich für UI, Export und Audit):
- Email: erster Buchstabe des Local-Parts, `***`, `@`, erster Buchstabe der Domain, `***`, TLD. `niko@burkert.de` → `n***@b***.de`.
- Iban: Ländercode + 2 Prüfziffern + `****` + letzte 4. `DE89370400440532013000` → `DE89****3000`.
- CreditCard: `****` + letzte 4.
- Phone: `+49***` + letzte 3.
- Ipv4: erste zwei Oktette + `.*.*`.
- ApiKey/Jwt: erste 4 Zeichen + `…` (Länge nicht verraten). `ghp_abcd…`.
- UserTerm/Custom: erstes Zeichen + `***` + letztes Zeichen, bei Länge ≤ 3 nur `***`.

**Recorder-API** (`pseudonyms.rs`):

```rust
pub struct PseudonymStore<'a> { conn: &'a Connection, aead: Zeroizing<[u8; 32]> }
impl PseudonymStore<'_> {
    /// Liefert bestehendes Pseudonym oder vergibt <TYPE_n> (n = COUNT(kind_label in session)+1), speichert, count++ bei Wiederverwendung.
    pub fn resolve(&self, session: SessionId, kind: &FindingKind, value: &[u8], value_hash: &[u8; 32], alias: Option<&str>) -> Result<String, RecorderError>;
    pub fn list(&self, session: SessionId) -> Result<Vec<PseudonymRow>, RecorderError>;   // ohne Klartext
    pub fn reveal(&self, session: SessionId, pseudonym: &str) -> Result<Option<Zeroizing<Vec<u8>>>, RecorderError>; // nur für Export und M8
    pub fn export(&self, session: SessionId, out: &mut dyn Write) -> Result<(), RecorderError>;
}
pub struct PseudonymRow { pub pseudonym: String, pub kind: String, pub display_prefix: String, pub first_seen: DateTime<Utc>, pub count: u32, pub has_original: bool }
```

Der Editor (HUM-047) ändert sich so: `replaceAllOpen` und `replaceFinding` holen die Pseudonyme über `daemonClient.resolvePseudonyms(session, [ {kind, valueHash, value} ])` in einem Batch, statt lokal zu zählen. `PseudonymNaming` bleibt als Fallback nur für den Fake-Daemon.

**Export-Format**: Datei `pseudonyms-<session>.hpm` = JSON `{ "version": 1, "session": "...", "exported": "...", "kdf": "hkdf-sha256", "entries": [ { "pseudonym", "kind", "display_prefix", "first_seen", "count", "ciphertext_b64" | null } ] }`, das Ganze nochmals als Ganzes mit AES-256-GCM und einem vom Nutzer eingegebenen Passwort verschlüsselt (Argon2id, `argon2`-Crate, Parameter m=64 MiB, t=3, p=1, Salt 16 Bytes im Header). Dateiaufbau: `HPM1` (4 Bytes Magic) || salt(16) || nonce(12) || ciphertext. Der Export ist nur über das UI (Passwort-Dialog, dies ist ein erlaubter Modal-Fall, weil destruktiv-sensibel) und über die CLI in HUM-070 verfügbar.

**Proto**:

```proto
rpc Pseudonyms(PseudonymsRequest) returns (PseudonymsResponse);
message PseudonymsRequest {
  string session_id = 1;
  oneof op {
    ResolveOp resolve = 2;   // repeated ResolveItem { string kind; bytes value_hash; bytes value; string alias; }
    ListOp list = 3;
    ExportOp export = 4;     // string out_path; string password
  }
}
message PseudonymsResponse { repeated ResolvedItem resolved = 1; repeated PseudonymRowProto rows = 2; DiagnosticProto diagnostic = 3; }
```

`value` in `ResolveItem` verlässt den Host nie; die Verbindung ist der lokale UDS. Der Daemon loggt den Wert nirgends (kein `tracing` mit dem Feld).

**Mapping-Panel** (`mapping_panel.dart`): Unterer, einklappbarer Pane im Editor (Default eingeklappt, Titel „Mapping (n)"), Tabelle mit `TableView`: Pseudonym (Mono), Typ (Chip), Original (maskiert, Mono, `fg-1`), Zuerst gesehen, Anzahl. Kein „Anzeigen"-Button für den Klartext im MVP. Button „Exportieren…" öffnet Passwort-Dialog (zweimal eingeben, min 12 Zeichen), danach Datei-Speichern-Dialog (`file_picker.saveFile`). Im History-Detail (HUM-032) derselbe Pane als Tab „Mapping", nur bei `edited == true`.

**Audit-Ereignis** (für HUM-050): `pseudonym.created { session, pseudonym, kind }`. Kein Wert, kein Hash.

### Schritte
1. Cargo-Abhängigkeiten, `KeyStore` mit Tests gegen einen Fake-Keyring (`keyring`-Crate `mock`-Feature) und gegen ein Temp-Verzeichnis.
2. `humanitl-findings`: `Scanner::new(value_hash_key)` und HMAC statt SHA-256; bestehende Tests anpassen.
3. Migration + `PseudonymStore` mit Tests (resolve stabil, Zähler pro Typ, Secrets ohne Ciphertext, AAD-Bindung).
4. `mask()` mit Tabellen-Test.
5. Proto `Pseudonyms`, ipc-Handler, Fake-Daemon in Dart erweitern.
6. Editor auf `resolvePseudonyms` umstellen, `pseudonymMapProvider` (Family per Session, lädt `list`).
7. `MappingPanel` im Editor und im History-Detail.
8. Export mit Passwort-Dialog, Round-Trip-Test (Export → CLI-Import-Prüfung in HUM-070 `audit`? nein: eigener Test im Recorder, der die Datei wieder entschlüsselt).
9. `humanitld` öffnet den `KeyStore` beim Start, `KEYS_001` erscheint als Diagnostic im `diagnosticsProvider` und im Setup-Screen.

### Tests
Unit (`recorder/src/keys.rs`):
- `open_creates_and_persists_in_mock_keyring`, `open_falls_back_to_file_with_KEYS_001`, `file_wrong_mode_KEYS_002` (chmod 0644), `keyring_and_file_disagree_KEYS_003`, `derive_is_deterministic_and_purpose_separated` (drei Purposes ⇒ drei verschiedene Schlüssel, zweimal `derive` gleich).

Unit (`recorder/src/pseudonyms.rs`):
- `resolve_same_value_same_pseudonym_across_flows`, `resolve_counter_per_kind` (`<EMAIL_1>`, `<EMAIL_2>`, `<IBAN_1>`), `resolve_alias_used_for_user_term` (`Client-A`), `secret_stored_without_ciphertext` (ApiKey ⇒ `value_encrypted IS NULL`, `display_prefix == "ghp_…"`… genau: erste 4 + `…`), `ciphertext_bound_to_row` (Ciphertext in andere Zeile kopiert ⇒ Entschlüsselung schlägt fehl), `count_increments_on_reuse`, `list_never_returns_plaintext`.
- `mask_table`: die sieben Beispiele aus der Spezifikation.
- `export_roundtrip_with_password`, `export_wrong_password_fails`.

Unit (`findings`): `value_hash_is_hmac_not_sha256` (gleicher Wert, zwei Schlüssel ⇒ zwei Hashes).

Widget: `mapping_panel_shows_masked_only` (kein Klartext im Widget-Baum, Suche nach dem Originalstring schlägt fehl), `editor_uses_daemon_resolve` (Fake-Daemon zählt `resolve`-Aufrufe = 1 Batch pro „Alle ersetzen").

### Akzeptanzkriterien
- [ ] `humanitld` startet mit Keyring; ohne Secret Service startet er trotzdem, `KEYS_001` ist im Setup-Screen sichtbar mit Fix.
- [ ] `sqlite3 humanitl.db "select pseudonym, hex(value_hash), value_encrypted is null from pseudonyms"` zeigt für einen API-Key `NULL`-Ciphertext, für eine E-Mail einen Ciphertext.
- [ ] Zwei Anfragen derselben Session mit derselben E-Mail ⇒ beide `<EMAIL_1>`, `count == 2`.
- [ ] Mapping-Panel zeigt nur maskierte Originale; ein `grep` über den Widget-Baum im Test findet den Klartext nicht.
- [ ] Export erzeugt eine `.hpm`-Datei, die mit falschem Passwort nicht entschlüsselbar ist (Test).
- [ ] `tracing`-Ausgabe enthält an keiner Stelle Klartext-Werte (Test: `tracing-test`, Assertion auf Abwesenheit).
- [ ] Audit-Event `pseudonym.created` enthält nur Session, Pseudonym, Typ.

### Fallstricke
- Der `keyring`-Crate braucht auf Linux den Secret Service über D-Bus. Unter `systemctl --user` ist `DBUS_SESSION_BUS_ADDRESS` gesetzt, in einer SSH-Sitzung oder in CI nicht. Immer den Datei-Fallback testen, nie den Keyring als gegeben annehmen. Auf GNOME kann der Keyring gesperrt sein; dann liefert der Crate einen Fehler, nicht einen leeren Wert.
- HUM-025 hat möglicherweise `value_hash` als reines SHA-256 definiert. Das muss hier auf HMAC umgestellt werden, sonst ist die Allowlist per Wörterbuch angreifbar. Migration bestehender Findings ist im MVP nicht nötig (Test-Daten), aber der Test `value_hash_is_hmac_not_sha256` erzwingt die Umstellung.
- AES-GCM-Nonces dürfen pro Schlüssel nie wiederverwendet werden. 12 Bytes aus `OsRng` pro Eintrag; niemals Zähler, niemals aus `value_hash` ableiten.
- Zeroize: `Zeroizing<>` auf Master-Key, abgeleiteten Schlüsseln und entschlüsselten Werten. Keine `format!("{:?}", key)`.
- SQLite `PRIMARY KEY (session_id, value_hash)` mit BLOB funktioniert, aber der Vergleich ist bytegenau; `value_hash` immer als 32-Byte-BLOB, nie als Hex-String speichern (sonst Mischformen).
- `count` erhöhen nur, wenn `resolve` für einen weiteren Flow aufgerufen wird, nicht bei erneutem Rendern des Editors. Der Editor cached das Ergebnis pro Draft.
- Der Export-Dialog ist der einzige erlaubte Modal-Dialog im Editor (Passwort). Kein zweiter Modal für den Datei-Dialog: `file_picker.saveFile` ist der System-Dialog.
- Argon2 mit 64 MiB blockiert den tokio-Worker; im Daemon in `spawn_blocking` ausführen.

### Referenzen
- BACKLOG.md ADR-008, Abschnitt 4.2 (Seitenkanäle), Abschnitt 3.4 (Tabelle `pseudonyms`)
- DSGVO Art. 4 Nr. 5 (Pseudonymisierung), Leitfaden: https://www.ing-ism.de/magazin/dsgvo-pseudonymisierung-praxisleitfaden/
- `keyring`-Crate: https://docs.rs/keyring · `aes-gcm`: https://docs.rs/aes-gcm · `hkdf`: https://docs.rs/hkdf · `argon2`: https://docs.rs/argon2

---

## HUM-049 · Senden mit offenen Findings
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-047, HUM-048, HUM-062 · Blockiert: HUM-055

### Kontext
Usability-Review, Abschnitt 3 und 5 in BACKLOG.md 5: Der Nutzer darf nicht genervt, aber auch nicht überrascht werden. Bei offenen Funden muss „Allow" sichtbar anders aussehen, und es muss eine Pause geben, keine Modal-Dialoge. Das Security-Review verlangt, dass die Durchsetzung im Daemon liegt, nicht im UI. Dieses Issue liefert den Flow und das Setting `hold.hard_block_checksum_secrets`.

### Ziel
Hat eine gehaltene Anfrage offene Findings, heißt der Allow-Button „Senden mit 2 Findings" und ist amber. Ein Klick (oder Enter) öffnet statt des Sendens eine Inline-Pause innerhalb der Karte: Liste der offenen Funde mit Typ und maskiertem Wert, drei Buttons „Trotzdem senden", „Pseudonymisieren" (öffnet Editor mit „Alle ersetzen" bereits ausgeführt), „Blockieren". Jeder Fund hat „Ignorieren" (für diese Anfrage) und „Immer ignorieren" (Allowlist). Ist `hold.hard_block_checksum_secrets = true`, verweigert der Daemon `Allow` für Anfragen mit ungelösten Findings der Tier `Checksum` in den Kinds `ApiKey`, `Jwt`, `Iban`, `CreditCard` mit Diagnostic `HOLD_004`; die UI zeigt in diesem Fall „Trotzdem senden" nicht an.

### Nicht-Ziel
- Findings in Responses.
- Automatisches Pseudonymisieren ohne Klick (Regel-Aktion `redact` kommt in einem eigenen Issue nach dem MVP).

### Betroffene Pfade
- `app/lib/features/intercept/widgets/action_bar.dart` (ändern)
- `app/lib/features/intercept/widgets/findings_pause.dart` (neu)
- `app/lib/features/intercept/providers/decision_provider.dart` (ändern: `acknowledgedFindings`, `ignoreAlways`)
- `daemon/crates/proxy/src/hold.rs` (ändern: Prüfung vor `Allow`)
- `daemon/crates/recorder/src/findings.rs` (ändern: `resolved`, Allowlist-Abfrage)
- `daemon/crates/findings/src/scanner.rs` (ändern: Allowlist beim Scan anwenden)
- `daemon/crates/config/src/hold.rs` (ändern: neues Feld)
- ARB-Dateien

### Spezifikation

**Config**: `hold.hard_block_checksum_secrets: bool`, Default `false`, Tier `advanced`, Beschreibung: „Anfragen mit prüfsummen-verifizierten Secrets (API-Keys, JWT, IBAN, Kreditkarte) können nicht ungeändert gesendet werden." Sicherheitsrelevant, daher im Settings-Screen mit Hinweis.

**Daemon** (`hold.rs`, vor der Weitergabe von `Decision::Allow`):

```rust
fn check_allow(flow: &Flow, req: &DecideRequest, cfg: &HoldConfig) -> Result<(), Diagnostic> {
    let unresolved = flow.findings.iter().enumerate()
        .filter(|(i, f)| !req.acknowledged_findings.contains(&(*i as u32)) && !f.allowlisted);
    if cfg.hard_block_checksum_secrets {
        if let Some((_, f)) = unresolved.clone().find(|(_, f)| f.tier == Tier::Checksum && matches!(f.kind, ApiKey(_) | Jwt | Iban | CreditCard)) {
            return Err(Diagnostic::hold_004(f));  // Severity::Blocking, why: "…", fix: Some(FixAction::ChangeSetting{key:"hold.hard_block_checksum_secrets", value:"false"})
        }
    }
    Ok(())
}
```

`acknowledged_findings` markiert die Funde in der Tabelle `findings` als `resolved = 'acknowledged'` (Spalte `resolved` wird von BOOLEAN auf TEXT `NULL | replaced | acknowledged | allowlisted` erweitert, Migration). `ignore_always` schreibt `finding_allowlist(value_hash, kind, scope)`; `scope` ist `global`, wenn kein Projekt-Profil aktiv ist, sonst SHA-256-Hex des kanonischen Projektpfads. Beim nächsten Scan (HUM-025 `Scanner`) werden Findings mit Allowlist-Treffer mit `allowlisted = true` markiert, erscheinen im UI ausgegraut in einer eingeklappten Zeile „3 ignoriert in diesem Projekt" und zählen nicht als offen.

`Decided`-Event bekommt `unresolved_findings: u32` (nach Abzug von acknowledged und allowlisted). Audit (HUM-050): `flow.decided` trägt `unresolved_findings`, `acknowledged: n`, `allowlisted_added: n`.

**UI-Zustände des Allow-Buttons** (`action_bar.dart`):

| Bedingung | Label | Farbe | Enter |
|---|---|---|---|
| keine offenen Findings | „Senden" | Primär (Akzent) | sendet sofort |
| offene Findings, nicht hart geblockt | „Senden mit n Findings" | amber `#E0B24A`, Icon `triangle-alert` | öffnet `FindingsPause` |
| offene Checksum-Secrets und `hard_block` aktiv | „Senden nicht möglich" | deaktiviert, Tooltip mit `HOLD_004.why` | nichts |
| Editor-Entwurf vorhanden | „Editierte Version senden" | Sekundär mit Stift (aus HUM-047) | sendet Entwurf, gleiche Findings-Logik auf `remaining_findings` |

**FindingsPause** (`findings_pause.dart`): Ersetzt den unteren Teil der Karte (nicht die ganze Karte, kein Overlay), Höhe animiert 200 ms. Inhalt: Überschrift „n Funde in dieser Anfrage", Liste (Typ-Chip, maskierter Wert aus `mask()`, Ort `Header: Authorization` / `Body Zeile 12`), pro Zeile Buttons „Ignorieren" und „Immer ignorieren" (letzterer mit Tooltip „Dieser Wert wird in diesem Projekt nicht mehr gemeldet"). Fuß: `[Trotzdem senden] [Pseudonymisieren] [Blockieren]`, Tastatur: `S` senden, `P` pseudonymisieren, `B` blockieren, `Esc` zurück. „Trotzdem senden" sendet `Decide(Allow, acknowledged_findings = alle offenen)`. „Pseudonymisieren" setzt `editorOpenProvider = true` und ruft `draftProvider(id).replaceAllOpen()`.

### Schritte
1. Config-Feld, Migration `findings.resolved` TEXT, Allowlist-Abfrage im Recorder.
2. `Scanner` wendet Allowlist an; Test.
3. `check_allow` in `hold.rs` mit `HOLD_004`; Tests.
4. `Decide`-Handler verarbeitet `acknowledged_findings` und `ignore_always`.
5. `action_bar.dart` Zustände, `findings_pause.dart`, Provider.
6. Fake-Daemon: liefert `HOLD_004`, wenn ein Flag gesetzt ist.
7. Widget-Tests.

### Tests
Unit (`hold.rs`): `allow_with_open_regex_findings_ok`, `allow_with_checksum_secret_blocked_when_setting_on` (`HOLD_004`), `allow_with_checksum_secret_ok_when_acknowledged`, `allow_ok_when_setting_off`, `allowlisted_not_counted`.
Unit (`findings`): `allowlist_marks_finding` (Wert in Allowlist mit `scope = global` ⇒ `allowlisted == true`), `allowlist_project_scope_does_not_leak_to_other_project`.
Widget: `button_label_with_findings`, `enter_opens_pause_not_send` (Fake-Daemon zählt `decide` = 0), `send_anyway_acknowledges_all`, `hard_block_hides_send_anyway`, `ignore_always_calls_decide_with_ignore_always`.

### Akzeptanzkriterien
- [ ] Anfrage mit einer E-Mail im Body: Button heißt „Senden mit 1 Finding", Enter öffnet die Pause, kein Request geht raus (Fake-Daemon-Zähler).
- [ ] „Trotzdem senden" leitet weiter; History zeigt `unresolved_findings = 1`.
- [ ] Mit `hold.hard_block_checksum_secrets = true` und einer gültigen IBAN: „Senden nicht möglich", Daemon lehnt `Decide(Allow)` über die CLI (`humanitl flows decide` existiert nicht; Test über gRPC-Client) mit `HOLD_004` ab.
- [ ] „Immer ignorieren" auf einen Wert ⇒ nächste Anfrage mit demselben Wert zeigt ihn in der eingeklappten „ignoriert"-Zeile, Button ist „Senden".
- [ ] Alle Tests grün, ARB-Schlüssel `interceptFindingsPause*` in `en` und `de`.

### Fallstricke
- Die Prüfung muss im Daemon laufen. Ein UI, das `acknowledged_findings` einfach immer füllt, darf `HOLD_004` nicht umgehen können: Daemon prüft Tier und Kind, nicht nur „acknowledged". Deshalb gilt bei `hard_block` `acknowledged` nur für Nicht-Checksum-Findings? Nein: Der Nutzer darf bewusst „Trotzdem senden", wenn das Setting aus ist. Ist es an, gibt es keinen Weg außer Pseudonymisieren oder Setting ändern. Test `allow_with_checksum_secret_ok_when_acknowledged` gilt nur bei Setting aus; bei Setting an muss derselbe Aufruf `HOLD_004` liefern. Beide Fälle testen.
- Regex-Tier-Findings haben Fehlalarme (Telefonnummern, IPs). Sie dürfen nie hart blocken.
- „Immer ignorieren" auf einen API-Key wäre gefährlich; für `ApiKey`/`Jwt` ist der Button deaktiviert mit Tooltip „Secrets können nicht dauerhaft ignoriert werden".
- Findings-Indizes beziehen sich auf die Reihenfolge im `Analyzed`-Event. Nach einem Edit (HUM-047) werden sie neu vergeben; die Pause muss dann die `remaining_findings` des Entwurfs zeigen, nicht die alten.

### Referenzen
- BACKLOG.md Abschnitt 5 (Decision ergonomics, Anonymization editor), ADR-012
- Claude Code Permission-Prompts (Vorbild „was genau passiert"): https://code.claude.com/docs/en/permissions

---

## HUM-050 · Audit-Hash-Kette
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-004, HUM-026, HUM-048 · Blockiert: HUM-051, HUM-070, HUM-055

### Kontext
ADR-008 und BACKLOG.md 4.5 Test 5 sowie die Compliance-Notiz: Der Nutzer muss später belegen können, welche Anfragen der Agent gestellt hat und was entschieden wurde. Das Security-Review hat präzisiert, was eine Hash-Kette beweist (keine nachträgliche Änderung oder Löschung in der Mitte durch jemanden ohne Schreibrecht auf die ganze Kette) und was nicht (ehrlicher Schreiber, Tail-Kürzung ohne Anker, Neuaufbau ohne Schlüssel). Dieses Issue implementiert die Kette mit HMAC und Anchoring und dokumentiert die Grenzen ehrlich.

### Ziel
Jeder relevante Vorgang im Daemon erzeugt einen Audit-Record in `$XDG_DATA_HOME/humanitl/audit/audit.jsonl`. Jeder Record enthält den Hash des Vorgängers, seinen eigenen Hash über eine kanonische Serialisierung und einen HMAC mit einem Schlüssel aus dem `KeyStore`. Alle N Records und beim Beenden schreibt der Daemon einen Anker (Sequenznummer + Hash) in die SQLite-Tabelle `audit_anchors`. `humanitl audit verify` (HUM-070) und der Audit-Screen (HUM-051) prüfen Kette, HMACs und Anker und melden die erste fehlerhafte Position. Bodies, Klartext-Werte und Originale von Pseudonymen stehen nie im Log.

### Nicht-Ziel
- Externes Anchoring (Zeitstempeldienst, Blockchain, signierter Export): nach dem MVP.
- Verschlüsselung des Logs: nicht nötig, es enthält keine Payloads.
- Retention-Löschung der Kette: Löschen bricht die Kette absichtlich; im MVP `audit.retention_days = 0` (nie löschen), Rotation nach dem MVP.

### Betroffene Pfade
- `daemon/crates/audit/src/lib.rs` (neu): `AuditRecord`, `AuditWriter`, `AuditVerifier`, `canonical_json`
- `daemon/crates/audit/src/canonical.rs` (neu)
- `daemon/crates/audit/src/kinds.rs` (neu): alle Record-Kinds als Enum mit Datenstrukturen
- `daemon/crates/recorder/migrations/V5__audit_anchors.sql` (neu)
- `daemon/crates/recorder/src/anchors.rs` (neu)
- `daemon/bin/humanitld/src/audit_sink.rs` (neu): abonniert `FlowEvent`, Regel-, Config-, Sandbox-Ereignisse und schreibt Records
- `daemon/crates/ipc/src/audit.rs` (neu): RPC `Audit` mit `Verify`, `HeadHash`, `Export`, `Query`
- `daemon/crates/config/src/audit.rs` (neu): `audit.anchor_every` (u32, Default 100, `advanced`), `audit.retention_days` (u32, Default 0, `expert`), `audit.dir` (`expert`)
- `proto/humanitl/v1/humanitl.proto` (ändern: `Audit`-Messages ausformulieren)
- `docs/SECURITY.md` (ändern: Abschnitt „Was die Audit-Kette beweist")

### Spezifikation

**Record-Format** (eine Zeile pro Record, `\n`-terminiert, UTF-8, keine Leerzeile am Ende):

```json
{"data":{...},"kind":"flow.decided","prev":"<64 hex>","seq":42,"session":"<uuid>","ts":"2026-09-02T10:00:00.123456Z","hash":"<64 hex>","mac":"<64 hex>"}
```

Felder: `seq` (u64, beginnt bei 1, lückenlos), `ts` (RFC 3339 UTC mit Mikrosekunden, immer `Z`), `session` (UUID oder `"-"` für sessionlose Records), `kind` (String aus der Tabelle unten), `data` (Objekt, kind-spezifisch), `prev` (Hash des Vorgängers, für `seq == 1` 64 × `0`), `hash`, `mac`.

**Kanonische Serialisierung** (`canonical.rs`), Funktion `canonical_json(value: &serde_json::Value) -> Vec<u8>`:
- Objekte: Schlüssel bytewise aufsteigend sortiert (nicht locale-abhängig), keine Duplikate.
- Keine Whitespace-Zeichen außerhalb von Strings.
- Zahlen: nur Integer (i64/u64) erlaubt. Ein Float in `data` ist ein Programmierfehler; `canonical_json` gibt `Err(CanonicalError::Float)` zurück, und der Writer panicked im Debug-Build, im Release wird der Record mit `data: {"error":"non_canonical"}` geschrieben und `AUDIT_003` als Diagnostic gemeldet. Dauer immer als Integer Millisekunden, Größen als Bytes.
- Strings: JSON-Escaping nur für `"`, `\`, Steuerzeichen < 0x20 als `\u00XX`; alle anderen Zeichen unescaped als UTF-8 (kein `\u`-Escaping von Nicht-ASCII, kein `/`-Escaping).
- Booleans und `null` wie JSON.
- Implementierung: eigene rekursive Funktion über `serde_json::Value` mit `BTreeMap`-Sortierung; **nicht** auf das Feature-Flag `preserve_order` von `serde_json` verlassen (ein anderer Crate im Workspace kann es aktivieren).

**Hash und MAC**:
- `hash = SHA-256( canonical_json({ data, kind, prev, seq, session, ts }) )` als Hex-String (lowercase).
- `mac = HMAC-SHA256( key = KeyStore.derive(AuditHmac), msg = hash_bytes(32) )` als Hex.
- Die Zeile wird aus dem Record mit `hash` und `mac` erneut kanonisch serialisiert, damit die Datei selbst kanonisch ist (`verify` kann Zeilen bytegenau reproduzieren).

**Record-Kinds** (`kinds.rs`), `data` je Kind:

| kind | data |
|---|---|
| `daemon.started` | `version`, `proto_version`, `key_origin` (`keyring`/`file`) |
| `daemon.stopped` | `reason` |
| `session.started` | `profile`, `agent`, `work_dir_hash` (SHA-256-Hex des kanonischen Pfads), `work_mode`, `llm_endpoint_host`, `sandbox_backend`, `argv_hash` |
| `session.ended` | `flows_total`, `held`, `allowed`, `allowed_edited`, `blocked`, `timed_out`, `auto_rule`, `passthrough` |
| `isolation.check` | `results: [{check, passed}]` |
| `flow.received` | `flow`, `method`, `scheme`, `host`, `port`, `path_hash` (SHA-256-Hex des Pfads; der Pfad selbst kann Secrets in Query-Parametern enthalten), `size`, `findings` (Anzahl), `findings_kinds: [..]` |
| `flow.decided` | `flow`, `decision` (`allow`/`allow_edited`/`block`/`timed_out`/`auto_allow`/`auto_block`/`passthrough`), `rule` (RuleId oder null), `edited` (bool), `replacements` (Anzahl), `unresolved_findings`, `acknowledged`, `allowlisted_added`, `decided_by` (`user`/`rule`/`timeout`/`cli`) |
| `flow.forwarded` | `flow`, `upstream_ip` (Pinned IP, dokumentiert als bewusst geloggt) |
| `flow.responded` | `flow`, `status`, `size`, `duration_ms`, `streamed` |
| `flow.blocked_reason` | `flow`, `reason` (BlockReason snake_case) — nur bei Block ohne Nutzerentscheidung (AuthorityMismatch, BodyCap, NoRoute) |
| `rule.added` / `rule.updated` / `rule.removed` | `rule`, `action`, `match_host`, `match_method`, `expires`, `created_from`, `origin` (`ui`/`cli`/`bundled`/`remember`) |
| `config.changed` | `key`, `origin`, `secret` (bool; wenn true, kein `value`), `value` (nur wenn nicht secret) |
| `pseudonym.created` | `pseudonym`, `kind` |
| `finding.allowlisted` | `kind`, `scope` |
| `audit.anchor` | `anchored_seq`, `anchored_hash` |
| `audit.verified` | `result`, `first_bad_seq` (oder null), `records` |

**Writer** (`AuditWriter`): Öffnet die Datei `O_APPEND`, hält `last_seq` und `last_hash` im Speicher (beim Start durch Lesen der letzten Zeile ermittelt; ist die Datei leer, `seq = 0`, `prev = 0…0`). Schreibt jede Zeile mit `write_all` + `fsync` alle 50 Records oder 1 s (konfigurierbar `audit.fsync_every`, `expert`). Ein einzelner `tokio::sync::mpsc`-Consumer serialisiert alle Schreibvorgänge; kein paralleler Zugriff. Bei jedem `anchor_every`-ten Record und bei `daemon.stopped` wird ein `audit.anchor`-Record geschrieben **und** derselbe Anker in die SQLite-Tabelle:

```sql
CREATE TABLE audit_anchors (seq INTEGER PRIMARY KEY, hash TEXT NOT NULL, ts TEXT NOT NULL);
```

**Verifier** (`AuditVerifier::verify(path, hmac_key: Option<&[u8;32]>, anchors: &[Anchor]) -> VerifyReport`):

```rust
pub struct VerifyReport { pub records: u64, pub status: VerifyStatus, pub warnings: Vec<VerifyWarning> }
pub enum VerifyStatus { Ok, Broken { first_bad_seq: u64, reason: BreakReason } }
pub enum BreakReason { SeqGap, PrevMismatch, HashMismatch, MacMismatch, NonCanonicalLine, AnchorMismatch { anchor_seq: u64 }, TruncatedBelowAnchor { anchor_seq: u64 } }
pub enum VerifyWarning { NoHmacKey, UnanchoredTail { records: u64 } }
```

Algorithmus: Zeilenweise lesen. Für jede Zeile: (1) `serde_json::from_slice`; (2) die Zeile muss bytegenau gleich `canonical_json(record)` sein, sonst `NonCanonicalLine`; (3) `seq == last_seq + 1`, sonst `SeqGap`; (4) `prev == last_hash`, sonst `PrevMismatch`; (5) Hash neu berechnen, vergleichen, sonst `HashMismatch`; (6) wenn Schlüssel vorhanden, MAC prüfen, sonst `MacMismatch`; ohne Schlüssel Warnung `NoHmacKey`; (7) für jeden Anker mit `anchor.seq == seq`: `anchor.hash == hash`, sonst `AnchorMismatch`. Nach der Datei: gibt es einen Anker mit `seq > last_seq`, dann `TruncatedBelowAnchor`. Sind nach dem letzten Anker Records ohne Anker, `UnanchoredTail { records }`. Der erste Fehler beendet die Prüfung (Report enthält `first_bad_seq`).

**Was die Kette beweist** (Text für `docs/SECURITY.md`, verbindlich): Sie beweist, dass seit dem letzten Anker kein Record geändert, entfernt oder umsortiert wurde, ohne dass `verify` es meldet, sofern der Angreifer den HMAC-Schlüssel nicht hat. Sie beweist nicht, dass der Daemon ehrlich geschrieben hat, dass nie geschriebene Ereignisse fehlen, oder dass die letzten bis zu `anchor_every` Records nach dem letzten Anker nicht gekürzt wurden. Wer den Keyring des Nutzers hat, kann die Kette neu bauen. Für stärkere Garantien braucht es externes Anchoring (nach dem MVP).

**Proto** (`Audit`):

```proto
rpc Audit(AuditRequest) returns (AuditResponse);
message AuditRequest { oneof op { VerifyOp verify = 1; HeadOp head = 2; ExportOp export = 3; QueryOp query = 4; } }
message VerifyOp {}                             // Daemon prüft mit eigenem Schlüssel und SQLite-Ankern
message HeadOp {}                               // liefert seq, hash, ts, letzter Anker
message ExportOp { string format = 1; string out_path = 2; string since = 3; string until = 4; }   // jsonl | csv
message QueryOp { string kind_prefix = 1; string session_id = 2; string since = 3; uint32 limit = 4; string cursor = 5; }
message AuditResponse { VerifyReportProto verify = 1; HeadProto head = 2; repeated AuditRecordProto records = 3; string next_cursor = 4; DiagnosticProto diagnostic = 5; }
```

CSV-Export: Spalten `seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash`; `data`-Felder, die es für den Kind nicht gibt, bleiben leer; `data` als Ganzes wird nicht exportiert (dafür JSONL).

### Schritte
1. `canonical.rs` mit Tabellen-Tests (Sortierung, Escaping, Float-Fehler, Nicht-ASCII).
2. `AuditRecord`, `kinds.rs`, Hash und MAC; Test mit festem Schlüssel und festem Erwartungs-Hash (Golden-Vektor, im Test hart codiert).
3. `AuditWriter` mit mpsc, fsync-Policy, Wiederaufnahme aus bestehender Datei.
4. Migration und `anchors.rs`; Writer schreibt Anker doppelt.
5. `AuditVerifier`, alle `BreakReason`s per Test erzeugt.
6. `audit_sink.rs` in `humanitld`: Mapping `FlowEvent` → Kinds, Regel-/Config-/Sandbox-Hooks, `daemon.started/stopped`.
7. RPC `Audit`, Export JSONL/CSV.
8. `docs/SECURITY.md`-Abschnitt.

### Tests
Unit (`canonical`): `sorts_keys_bytewise` (`{"b":1,"a":2,"B":3}` ⇒ `{"B":3,"a":2,"b":1}`), `no_whitespace`, `utf8_unescaped` (`"ü"` bleibt `ü`), `control_chars_escaped` (``), `float_rejected`, `nested_objects_sorted`.
Unit (`audit`): `genesis_prev_is_zeros`, `hash_golden_vector` (fester Record, fester Schlüssel, erwarteter Hash und MAC als Konstante), `writer_resumes_from_existing_file` (3 Records schreiben, Writer neu öffnen, 4. Record hat `prev` = Hash von 3), `anchor_written_every_n` (`anchor_every = 3`, 7 Records ⇒ Anker bei 3 und 6 in Datei und SQLite), `anchor_on_stop`.
Tamper-Tests (`audit/tests/tamper.rs`): Datei mit 10 Records und Ankern bei 5 und 10 erzeugen, dann:
- `modify_field_detected`: in Record 4 `decision` ändern ⇒ `Broken{4, HashMismatch}`.
- `delete_middle_detected`: Zeile 4 löschen ⇒ `Broken{5, SeqGap}` (seq 5 folgt auf 3).
- `reorder_detected`: Zeilen 6 und 7 tauschen ⇒ `Broken{6, SeqGap}`.
- `recompute_without_key_detected`: Record 4 ändern und Hash sowie alle folgenden `prev`/`hash` korrekt neu berechnen, MAC aber mit anderem Schlüssel ⇒ `Broken{4, MacMismatch}`.
- `truncate_below_anchor_detected`: Zeilen 8–10 löschen ⇒ `Broken{7, TruncatedBelowAnchor{10}}`.
- `truncate_above_last_anchor_not_detected_documented`: 12 Records (Anker bei 5, 10), Zeilen 11–12 löschen ⇒ `Ok` mit `warnings == []`; der Test heißt so und kommentiert die Grenze. (Mit 13 Records und Löschen von 12–13 ⇒ `Ok` mit `UnanchoredTail{1}`.)
- `non_canonical_line_detected`: Whitespace in Zeile 2 einfügen ⇒ `Broken{2, NonCanonicalLine}`.
- `anchor_tampered_detected`: Anker in SQLite ändern ⇒ `Broken{5, AnchorMismatch{5}}`.
Integration (`humanitld`): `decided_event_produces_record_without_payload` (Flow mit E-Mail im Body ⇒ `flow.received` enthält keinen Body und keine E-Mail; Assertion per Substring-Suche über die Zeile), `path_is_hashed` (Query mit `token=abc` ⇒ `abc` nicht in der Datei).

### Akzeptanzkriterien
- [ ] Nach einer Session enthält `audit.jsonl` `session.started`, mindestens ein `flow.received`, `flow.decided`, `session.ended`, und `audit.anchor` bei jedem `anchor_every`-ten Record.
- [ ] `grep -c` nach einem im Test verwendeten Klartext-Wert über `audit.jsonl` liefert 0.
- [ ] Alle acht Tamper-Tests grün, inklusive des dokumentierten Nicht-Erkennungsfalls.
- [ ] `verify` über 100 000 Records dauert unter 5 s (Bench-Test, `#[ignore]` in normalem Lauf).
- [ ] `docs/SECURITY.md` enthält den Abschnitt „Was die Audit-Kette beweist" mit den vier Grenzen.
- [ ] Der Golden-Vektor-Test verhindert unbemerkte Änderungen an der Kanonisierung.

### Fallstricke
- Zeitstempel: `chrono` serialisiert je nach Feature mal mit, mal ohne Nanosekunden. Immer explizit `ts.format("%Y-%m-%dT%H:%M:%S%.6fZ")` und in `data` nie `DateTime` direkt serialisieren, sondern über denselben Formatter.
- `serde_json::Value::Number` kann Floats sein, auch wenn der Wert ganzzahlig ist (`1.0`). `canonical_json` prüft `is_i64() || is_u64()`, nicht `as_f64().fract() == 0`.
- HashMap-Iteration in Rust ist zufällig; `data` niemals aus einer `HashMap` bauen, ohne sie durch `canonical_json` zu schicken. Die Kanonisierung ist die einzige Serialisierung, die je in die Datei geschrieben wird.
- `O_APPEND` + ein Writer-Task reicht; zwei Daemon-Instanzen dürfen nie dieselbe Datei öffnen. Flock auf die Datei beim Start (`fs2`-Crate), `AUDIT_001` (Blocking) bei Konflikt.
- Wird der Daemon hart beendet, kann die letzte Zeile unvollständig sein. Beim Öffnen: letzte Zeile ohne `\n` als korrupt behandeln, in `audit.jsonl.corrupt-<ts>` verschieben, `AUDIT_002` (Warning) melden, Kette ab dem letzten vollständigen Record fortsetzen. `verify` meldet dann `SeqGap`? Nein: der Writer setzt `seq` auf `last_complete + 1`, es entsteht keine Lücke; der verlorene Record fehlt und ist genau die dokumentierte Grenze „nie geschriebene Ereignisse".
- `path_hash` statt Pfad ist bewusst: Query-Strings enthalten Tokens. Der Host ist im Klartext, das ist der dokumentierte Seitenkanal aus BACKLOG.md 4.2.
- `upstream_ip` zu loggen ist eine bewusste Entscheidung für Nachvollziehbarkeit (DNS-Rebinding-Nachweis). Im Export mit Host-Redaktion (nach MVP) wird auch die IP redigiert.
- Die Anker doppelt zu schreiben (Datei + SQLite) ist der Punkt der Übung: Ein Angreifer, der nur die JSONL-Datei editiert, scheitert an SQLite; einer, der beides editiert, braucht zusätzlich den HMAC-Schlüssel.

### Referenzen
- BACKLOG.md ADR-008, 4.5 Test 5, Compliance-Notiz in Abschnitt 4
- Tamper-evident logs mit HMAC-Kette: https://tracehold.ai/blog/immutable-audit-log-hmac-hash-chain/
- EU AI Act Logging-Pflichten (Kontext): https://ki-spezial.systems/cluster/eu-ai-act-audit-logs.html
- RFC 8785 JSON Canonicalization Scheme (Orientierung; wir nutzen eine strengere Teilmenge ohne Floats): https://www.rfc-editor.org/rfc/rfc8785

---

## HUM-051 · Audit-Screen
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-050 · Blockiert: HUM-055

### Kontext
Fünfter Eintrag der Icon-Rail (BACKLOG.md Abschnitt 5, IA). Compliance-Nutzer müssen ohne CLI prüfen und exportieren können. Retention gehört hierher, weil sie das ist, was Compliance als „dokumentierte Löschung" sehen will.

### Ziel
Der Audit-Screen zeigt oben den Zustand der Kette (verifiziert oder gebrochen ab Sequenz X), den Head-Hash zum Kopieren, Anzahl Records und Anker, Zeitpunkt des letzten Ankers, und einen Button „Jetzt prüfen". Darunter eine virtualisierte Tabelle aller Records mit Filter nach Kind, Session und Zeitraum. Export als JSONL oder CSV mit Zeitraum. Ein Abschnitt „Aufbewahrung" verlinkt auf die Settings `recorder.retention_days` und `audit.retention_days` und erklärt in zwei Sätzen, was gelöscht wird und was nie.

### Nicht-Ziel
- Bearbeiten oder Löschen einzelner Records (unmöglich per Design).
- Signierter Export (nach MVP).

### Betroffene Pfade
- `app/lib/features/audit/audit_screen.dart` (neu)
- `app/lib/features/audit/widgets/chain_status_card.dart` (neu)
- `app/lib/features/audit/widgets/audit_table.dart` (neu)
- `app/lib/features/audit/widgets/retention_section.dart` (neu)
- `app/lib/features/audit/providers/audit_provider.dart` (neu): `auditHeadProvider`, `auditVerifyProvider`, `auditRecordsProvider(filter)`
- `app/lib/core/ipc/daemon_client.dart` (ändern: `auditVerify`, `auditHead`, `auditQuery`, `auditExport`)
- `app/lib/app.dart` (ändern: Rail-Eintrag 5, `Ctrl+5`)
- `daemon/crates/recorder/src/retention.rs` (neu): täglicher Job, löscht `flows`, `messages`, `findings`, Blobs älter als `recorder.retention_days` (0 = nie); Audit-Kette wird nicht angefasst, wenn `audit.retention_days == 0`
- ARB

### Spezifikation

**Layout** (Screen unter der Rail, volle Breite):

```
┌ ChainStatusCard ─────────────────────────────────────────────────────────────┐
│ ● Verifiziert · 4 213 Records · 42 Anker · letzter Anker vor 3 min           │
│ Head  a3f9…c2e1  [Kopieren]                       [Jetzt prüfen] [Export ▾]  │
└──────────────────────────────────────────────────────────────────────────────┘
Filter: [Kind ▾] [Session ▾] [von] [bis]                                  4 213
┌ AuditTable (TableView) ──────────────────────────────────────────────────────┐
│ seq │ Zeit          │ Kind           │ Session │ Zusammenfassung              │
│ 4213│ 10:42:01.123  │ flow.decided   │ 7f3a…   │ allow · GET · api.github.com │
└──────────────────────────────────────────────────────────────────────────────┘
┌ RetentionSection ────────────────────────────────────────────────────────────┐
│ Aufzeichnungen (Anfragen, Antworten, Bodies) werden nach 180 Tagen gelöscht. │
│ Die Audit-Kette wird nie gelöscht.                          [Einstellungen]  │
└──────────────────────────────────────────────────────────────────────────────┘
```

Zustände der Status-Karte: `Ok` grün mit Check-Icon; `Broken` rot mit `shield-x`, Text „Kette gebrochen ab Sequenz 4 012 (HashMismatch)", darunter Diagnostic `AUDIT_010` (Error) mit `why`: „Ein Record wurde nach dem Schreiben verändert oder entfernt." und `fix: OpenUrl(docs/SECURITY.md#audit)`; Warnungen (`NoHmacKey`, `UnanchoredTail`) amber als Zeile. Die Prüfung läuft beim Öffnen des Screens einmal und auf Klick; Ergebnis bleibt bis zum nächsten Lauf.

Zusammenfassungs-Spalte pro Kind: `flow.decided` ⇒ `<decision> · <method> · <host>`; `rule.added` ⇒ `<action> · <match_host>`; `config.changed` ⇒ `<key> = <value|•••>`; `session.started` ⇒ `<agent> · <profile>`; sonst leer. Klick auf eine Zeile öffnet ein Sheet mit dem vollständigen Record als JSON (Mono, read-only, Kopieren).

Export: Menü mit „JSONL (vollständig)" und „CSV (Übersicht)", danach `file_picker.saveFile`; der Daemon schreibt die Datei (`ExportOp.out_path`), das UI zeigt Inline-Bestätigung „Exportiert · 4 213 Records · Pfad". Zeitraum aus dem Filter wird übernommen.

Retention-Job (`retention.rs`): Beim Daemon-Start und danach alle 24 h; löscht in einer Transaktion `flows` älter als Grenze inklusive abhängiger Zeilen, danach verwaiste Blobs (Referenzzählung über `messages.blob_ref`). Schreibt Audit-Record `recorder.retention_applied { deleted_flows, deleted_blobs, cutoff }` (Kind in HUM-050-Tabelle nachtragen). `recorder.retention_days` Default 180, Tier `advanced`, Beschreibung erwähnt DSGVO Art. 5 Abs. 1 lit. e.

### Schritte
1. `daemon_client` um vier Aufrufe erweitern, Fake-Daemon liefert 200 synthetische Records und einen `Ok`-Report (Flag für `Broken`).
2. Provider, `ChainStatusCard`, Rail-Eintrag.
3. `AuditTable` mit serverseitigem Cursor (`QueryOp.cursor`), Filterleiste.
4. Sheet für Record-Detail.
5. Export-Menü.
6. `retention.rs` mit Test, `RetentionSection`.

### Tests
Widget: `status_ok_green`, `status_broken_shows_seq_and_reason`, `filter_by_kind_calls_query_with_prefix`, `row_tap_opens_sheet_with_json`, `export_csv_calls_export_with_range`.
Unit (`retention.rs`): `deletes_older_than_cutoff_only`, `orphan_blobs_removed`, `zero_means_never`, `audit_untouched`.

### Akzeptanzkriterien
- [ ] `Ctrl+5` öffnet den Screen; Status-Karte zeigt nach ≤ 2 s ein Ergebnis.
- [ ] Nach Manipulation der JSONL-Datei (Test aus HUM-050) zeigt der Screen „gebrochen ab Sequenz n" mit Grund.
- [ ] Head-Hash im UI == `humanitl audit verify --json | jq .head` (HUM-070).
- [ ] CSV-Export enthält die zwölf Spalten aus HUM-050, JSONL ist bytegleich mit der Quelldatei für den Zeitraum.
- [ ] Retention: Flow mit `ts` vor 200 Tagen ist nach Job weg, Audit-Records bleiben.

### Fallstricke
- `verify` über die gesamte Datei kann bei sehr großen Logs Sekunden dauern; UI zeigt Fortschritt aus dem Daemon? Im MVP: Spinner und Ergebnis, `verify` läuft in `spawn_blocking`.
- Tabelle nie clientseitig komplett laden; Cursor-Paging, Seiten von 200.
- Die Zusammenfassungs-Spalte darf nur Felder anzeigen, die im Record stehen. Kein Nachladen des Flows (wäre Payload).

### Referenzen
- HUM-050 Spezifikation, BACKLOG.md Abschnitt 5 (IA), DSGVO Art. 5 Abs. 1 lit. e

---

## HUM-052 · i18n Deutsch und Englisch
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-019 · Blockiert: HUM-069, HUM-054, HUM-055

### Kontext
BACKLOG.md Abschnitt 5 „Sprache" und Usability-Review Punkt 8: Englisch als Quellsprache, Deutsch erstklassig, bestimmte Begriffe bewusst gewählt (angehalten, Senden vs Erlauben, Pseudonymisieren). Bis hierher sind Strings in den Features vermutlich teilweise hart codiert; dieses Issue zieht alles in ARB und legt das Glossar fest, das alle folgenden Issues verwenden.

### Ziel
Alle UI-Strings liegen in `app/l10n/app_en.arb` (Quelle) und `app/l10n/app_de.arb`. `flutter gen-l10n` erzeugt `AppLocalizations`. Ein Lint-Skript in CI schlägt fehl, wenn `de` Schlüssel fehlen oder ein Dart-String-Literal in `features/` in einem `Text(...)`-Widget steht. Das Setting `ui.language` (`en|de`, Default aus System-Locale, Fallback `en`) schaltet zur Laufzeit um. Das Glossar unten ist verbindlich für alle Übersetzungen.

### Nicht-Ziel
- Weitere Sprachen.
- Übersetzung der CLI-Ausgaben (bleibt Englisch im MVP).
- Übersetzung von Diagnostics-Texten aus dem Daemon: Der Daemon liefert `code`; die UI übersetzt `title`/`why`/`fix`-Label anhand des Codes aus ARB (`diag_<CODE>_title` usw.). Daemon-Text ist der Fallback, wenn kein Schlüssel existiert. Das ist Teil dieses Issues für alle bis hierher existierenden Codes.

### Betroffene Pfade
- `app/l10n.yaml` (neu)
- `app/l10n/app_en.arb`, `app/l10n/app_de.arb` (ändern/neu)
- `app/lib/core/l10n/l10n.dart` (neu): Extension `context.l10n`, `DiagnosticL10n.resolve(code)`
- `app/lib/app.dart` (ändern: `localizationsDelegates`, `supportedLocales`, `locale` aus `configProvider`)
- `app/lib/features/**` (ändern: alle Literale ersetzen)
- `tool/l10n_lint.dart` (neu)
- `.github/workflows/ci.yml` (ändern: Lint-Job)
- `docs/GLOSSARY.md` (neu, das Glossar unten)

### Spezifikation

**`l10n.yaml`**:

```yaml
arb-dir: l10n
template-arb-file: app_en.arb
output-localization-file: app_localizations.dart
output-class: AppLocalizations
nullable-getter: false
use-escaping: true
```

**Schlüssel-Konvention**: `<feature><Element><Variante>` in camelCase, Feature-Präfixe `common`, `setup`, `intercept`, `editor`, `history`, `rules`, `sandbox`, `audit`, `settings`, `diag`. Beispiele: `interceptAllowButton`, `interceptAllowWithFindingsButton`, `editorSendEditedButton`, `rulesRememberSentence`, `diagTLS003Title`. Jeder Schlüssel hat in `app_en.arb` einen `@`-Eintrag mit `description` und, bei Platzhaltern, typisierten `placeholders`.

**Platzhalter**: immer typisiert (`int`, `String`, `DateTime`), Zahlen mit `type: int, format: decimalPattern`, Bytes über eigene Helferfunktion `formatBytes` (nicht ICU), Zeiten über `DateFormat.Hms()`.

**Plural**: ICU-Syntax mit `=0`, `=1`, `other` für Englisch und Deutsch, z. B.

```json
"interceptAllowWithFindingsButton": "{count, plural, =1{Send with 1 finding} other{Send with {count} findings}}",
```
```json
"interceptAllowWithFindingsButton": "{count, plural, =1{Senden mit 1 Fund} other{Senden mit {count} Funden}}",
```

**Regel-Satz** (`rulesRememberSentence`): Platzhalter `action`, `method`, `host`, `duration`; die Einzelteile kommen aus eigenen Schlüsseln (`rulesActionAllow` ⇒ „erlauben", `rulesDurationSession` ⇒ „diese Session"), der Satz selbst ist `{action} · {method} · {host} · {duration}` in beiden Sprachen; `method` und `host` bleiben unübersetzt.

**Glossar** (`docs/GLOSSARY.md`, verbindlich, mindestens diese 44 Einträge):

| Schlüssel-Begriff | en | de | Anmerkung |
|---|---|---|---|
| held (state) | Held | Angehalten | nie „abgefangen" |
| hold (verb) | Hold | Anhalten | |
| allow (button on request) | Send | Senden | Button beschreibt, was passiert |
| allow (rule action) | Allow | Erlauben | nur in Regeln |
| allow edited | Send edited version | Editierte Version senden | |
| block (button) | Block | Blockieren | nicht „Ablehnen" |
| block (rule action) | Block | Blockieren | |
| ask (rule action) | Ask | Nachfragen | |
| redact (rule action) | Redact | Pseudonymisieren | Aktion `redact` bleibt im YAML englisch |
| timed out | Timed out | Zeit abgelaufen | |
| auto-allowed by rule | Allowed by rule | Durch Regel erlaubt | |
| LLM passthrough | LLM passthrough | LLM-Durchleitung | |
| edited | Edited | Editiert | Chip |
| finding | Finding | Fund | nicht „Treffer" |
| secret | Secret | Secret | bleibt |
| PII | Personal data | Personenbezogene Daten | |
| pseudonymize | Pseudonymize | Pseudonymisieren | nie „Anonymisieren" |
| pseudonym | Pseudonym | Pseudonym | |
| mapping | Mapping | Zuordnung | Panel-Titel „Zuordnung (3)" |
| replace all | Replace all with pseudonyms | Alle durch Pseudonyme ersetzen | |
| ignore (once) | Ignore | Ignorieren | |
| ignore always | Always ignore | Immer ignorieren | |
| send anyway | Send anyway | Trotzdem senden | |
| rule | Rule | Regel | |
| remember | Remember | Merken | |
| scope (target) | Target | Ziel | |
| scope (duration) | Duration | Gültigkeit | |
| once | Once | Einmal | |
| this session | This session | Diese Session | „Session" bleibt |
| forever | Always | Immer | |
| exact URL | Exact URL | Genaue URL | |
| host | Host | Host | |
| domain (apex + subs) | Domain and subdomains | Domain und Subdomains | |
| host + method | Host and method | Host und Methode | |
| queue | Queue | Warteschlange | |
| history | History | Verlauf | |
| intercept (screen name) | Intercept | Anhalten | Rail-Label |
| sandbox | Sandbox | Sandbox | |
| isolation check | Isolation check | Isolationsprüfung | |
| no network interface | No network interface | Kein Netzwerk-Interface | |
| one socket | Exactly one socket, to Humanitl | Genau ein Socket, zu Humanitl | |
| seccomp active | New sockets forbidden (seccomp) | Neue Sockets verboten (seccomp) | |
| project folder | Project folder | Projektordner | |
| work dir | Work directory | Arbeitsverzeichnis | `/work` bleibt |
| agent is waiting | The agent is waiting for you | Der Agent wartet auf dich | Du-Form |
| daemon | Daemon | Daemon | |
| audit chain | Audit chain | Audit-Kette | |
| verified | Verified | Verifiziert | |
| broken at | Broken at sequence {seq} | Gebrochen ab Sequenz {seq} | |
| retention | Retention | Aufbewahrung | |
| settings tier basic/advanced/expert | Basic / Advanced / Expert | Grundlegend / Erweitert / Experte | |
| origin (of a setting) | Source | Herkunft | |
| reset | Reset to default | Auf Standard zurücksetzen | |

Deutsch verwendet die Du-Form. Protokollbegriffe (GET, POST, Header, Body, Query, Status, Content-Type) bleiben Englisch. Tastenkürzel-Hinweise sind sprachunabhängig.

**Diagnostics-Übersetzung**: `DiagnosticL10n.resolve(Diagnostic d, AppLocalizations l) -> (title, why, fixLabel)`. Sucht `diag<CODE>Title`, `diag<CODE>Why`, `diag<CODE>Fix` (CODE ohne Unterstrich, z. B. `diagSANDBOX001Title`); fehlt ein Schlüssel, Daemon-Text. Alle Codes, die bis Sprint 3 existieren, werden in beiden Sprachen angelegt (Liste aus `grep -r 'DiagnosticCode("' daemon/` generieren, im Lint-Skript prüfen).

**Lint** (`tool/l10n_lint.dart`): (1) jeder Schlüssel in `en` existiert in `de`, gleiche Platzhalter; (2) kein `Text('...')`, `Text("...")`, `label: '...'`, `tooltip: '...'` mit Literal in `lib/features/**` und `lib/packages/ui/**` außer in Dateien mit `// l10n-exempt` in Zeile 1 (nur für Galerie/Storybook); (3) jeder `DiagnosticCode` hat `Title` und `Why` in `en`. Exit-Code ≠ 0 bei Verstoß.

**Umschalten**: `configProvider.select((c) => c.ui.language)` steuert `MaterialApp.locale` (bzw. `ShadcnApp.locale`); Wechsel ohne Neustart. `system` ist kein Wert im MVP; beim ersten Start wird `ui.language` aus `Platform.localeName` (`de*` ⇒ `de`, sonst `en`) gesetzt und in die globale Config geschrieben.

### Schritte
1. `l10n.yaml`, Delegates in `app.dart`, `context.l10n`-Extension; Build läuft.
2. Alle bestehenden Literale in `features/` und `packages/ui` in ARB überführen (Feature für Feature, jeweils `flutter analyze` grün).
3. `docs/GLOSSARY.md` anlegen, `de`-ARB vollständig füllen.
4. Diagnostics-Schlüssel generieren und übersetzen.
5. `tool/l10n_lint.dart` und CI-Job.
6. Laufzeit-Umschaltung, Setting `ui.language`.
7. Goldens `intercept_card_de`, `action_bar_de` (Definition in HUM-054, hier nur sicherstellen, dass die deutschen Texte in 28 px Zeilenhöhe nicht umbrechen; ggf. kürzere Formulierung wählen).

### Tests
- `l10n_lint` in CI grün; Negativtest: absichtliches Literal in einer Testdatei ⇒ Exit 1 (im Lint-Test selbst).
- Widget `language_switch_updates_texts`: Config auf `de` setzen ⇒ Allow-Button zeigt „Senden".
- Unit `remember_sentence_de`: `allow · GET · *.npmjs.org · diese Session` ⇒ `erlauben · GET · *.npmjs.org · diese Session`.
- Unit `plural_de_one_vs_other`.
- Unit `diagnostic_fallback_to_daemon_text` (unbekannter Code).

### Akzeptanzkriterien
- [ ] `grep -rn "Text('" app/lib/features | grep -v l10n-exempt` liefert nichts.
- [ ] `app_de.arb` hat dieselbe Schlüsselmenge wie `app_en.arb` (Lint).
- [ ] Alle 44 Glossar-Einträge sind in beiden ARB-Dateien als Schlüssel vorhanden (Lint prüft eine Liste aus `docs/GLOSSARY.md`).
- [ ] Sprachwechsel im laufenden Programm ohne Neustart.
- [ ] Jeder bis Sprint 3 existierende Diagnostic-Code hat `Title` und `Why` in beiden Sprachen.

### Fallstricke
- shadcn_flutter bringt eigene Lokalisierung für seine Komponenten (`ShadcnLocalizations`); ihre Delegates müssen zusätzlich registriert werden, sonst stürzen Komponenten wie DatePicker in `de` ab.
- Deutsche Strings sind ~30 % länger; die Aktionsleiste (HUM-028) hat feste Breiten. Vor dem Übersetzen die Buttons auf `IntrinsicWidth` mit Maximalbreite umstellen, sonst Overflow. Golden `action_bar_de` fängt das.
- ICU-Plural in Deutsch: `=1{… 1 Fund}` und `other{… {count} Funden}`; `one` funktioniert in Dart-intl für `de` auch, aber `=1` ist explizit und sicher.
- `use-escaping: true` bedeutet, dass einfache Anführungszeichen in ARB als `''` geschrieben werden müssen.
- Keine Übersetzung von Schlüsseln, die in Audit-Records oder YAML landen (`allow`, `block`, `ask`, `redact`, Kinds). Nur Anzeige.
- `Platform.localeName` kann `C` oder `POSIX` sein (CI, Container) ⇒ Fallback `en`.

### Referenzen
- BACKLOG.md Abschnitt 5 „Sprache", Usability-Review Punkt 8
- Flutter i18n: https://docs.flutter.dev/ui/accessibility-and-internationalization/internationalization
- ICU MessageFormat Plural: https://unicode-org.github.io/icu/userguide/format_parse/messages/

---

## HUM-069 · Settings-Screen mit Progressive Disclosure
Sprint: 4 · Größe: L · Abhängigkeiten: HUM-062, HUM-052 · Blockiert: HUM-054, HUM-055

### Kontext
ADR-011 und Prinzip 8: Eine Konfigurationsquelle mit Schema; das UI wird aus dem Schema generiert, damit kein Setting nur im UI oder nur in der CLI existiert. Drei Stufen `basic`, `advanced`, `expert`. Herkunft jedes Wertes ist sichtbar. Das ist der Ort, an dem der Nutzer „viel tun kann", ohne dass der Standardweg davon belastet wird.

### Ziel
Ein Settings-Screen (über Command Palette „Settings", Zahnrad in der Statusleiste, `Ctrl+,`) rendert alle Config-Felder aus dem JSON-Schema des Daemons: gruppiert nach Top-Level-Objekt, sortiert nach Tier, mit Titel, Beschreibung, aktuellem Wert, Herkunfts-Badge, Reset-Button. `expert`-Felder sind eingeklappt und tragen bei Sicherheitsrelevanz einen Warnhinweis. Eine Suche findet Felder über Schlüssel, Titel und Beschreibung in allen Stufen. Änderungen werden über den Daemon geschrieben (`Config(Set)`), der Daemon lädt live neu und sendet `ConfigChanged`; das UI aktualisiert sich ohne Neustart. Felder, deren Wert aus Env oder CLI kommt, sind deaktiviert mit Erklärung. Ein Link „Config-Datei öffnen" öffnet `config.toml` im Systemeditor.

### Nicht-Ziel
- Profil-Editor (Profile bleiben Dateien; Auswahl im Setup und in HUM-066). Ein Profil-Feld wird als `enum` aus den vorhandenen Profilnamen gerendert.
- Regeln (eigener Screen HUM-033).
- Import/Export der Config (CLI in HUM-070: `config get`/`set`; Datei ist das Format).

### Betroffene Pfade
- `app/lib/features/settings/settings_screen.dart` (neu)
- `app/lib/features/settings/model/settings_schema.dart` (neu): Parser für das JSON-Schema mit `x-tier`, `x-security`, `x-origin`
- `app/lib/features/settings/widgets/setting_field.dart` (neu): Schema → Widget
- `app/lib/features/settings/widgets/setting_group.dart`, `settings_search.dart`, `origin_badge.dart`, `tier_section.dart` (neu)
- `app/lib/features/settings/providers/settings_provider.dart` (neu): `settingsSchemaProvider`, `settingsValuesProvider`, `settingsSearchProvider`
- `app/lib/core/ipc/daemon_client.dart` (ändern: `configSchema`, `configGet`, `configSet`, `configReset`, `subscribeConfig`)
- `app/lib/app.dart` (ändern: Route, `Ctrl+,`, Zahnrad)
- `app/lib/features/tray/providers/attention.dart` (ändern: `notificationsEnabled` liest `ui.notifications`, statt fest `true` zu antworten)
- `app/lib/features/shell/providers/theme.dart` (ändern: `themeModeProvider` startet aus `ui.theme` und folgt `ConfigChanged`)
- `daemon/crates/config/src/model.rs` und `daemon/crates/config/tests/config_readers.rs` (ändern: der Vermerk `x-pending-issue` an `ui.notifications` und `ui.theme` entfällt, ihre Registerzeilen stehen danach auf `effective`)
- `daemon/bin/humanitld/src/main.rs` (ändern: `load_config` behält `Resolved::diagnostics` und speist sie über die Warteschlange in den Ereignisstrom, statt sie nur zu protokollieren)
- `daemon/crates/config/src/schema.rs` (ändern, falls HUM-062 nicht bereits liefert: `x-tier`, `x-security`, `title`, `description`, `default`, `format`, `minimum`, `maximum`)
- `daemon/crates/config/src/origin.rs` (ändern: `Origin` pro Feld in `Config(Get)`-Antwort)
- `daemon/crates/ipc/src/config.rs` (neu oder ändern: RPC `Config`, Stream `SubscribeConfig`)
- `daemon/bin/humanitld/src/reload.rs` (neu): Datei-Watcher (`notify`-Crate) auf `config.toml`, Profile; Reload mit Validierung; `ConfigChanged`-Event
- ARB

### Spezifikation

**Die Befunde des Ladens dürfen nicht im Journal enden.** `load_config` (`daemon/bin/humanitld/src/main.rs:483-489`) schreibt heute jeden Befund aus `Resolved::diagnostics` mit `tracing::warn!` und verwirft ihn danach; unter systemd sieht ihn niemand. Das trifft genau die Fälle, in denen ein Wert stillschweigend übergangen wird: ein entfallener Schlüssel (`CONFIG_005`, `alias::RETIRED`, HUM-101), ein alter Name neben dem heutigen (`CONFIG_006`), ein gesperrter Schlüssel aus dem Projekt-Profil. Der Weg ist gebaut: `report_recorder_diagnostics` (`main.rs:302-330`) veröffentlicht Befunde ohne Flow als `FlowEvent::Diagnostic { flow_id: None }`, und `diagnosticsProvider` (HUM-106) sammelt sie. Zu tun ist, dass `load_config` seine Befunde behält, bis die Warteschlange steht, und sie dort einspeist; dieser Bildschirm zeigt sie neben den Feldern, die sie betreffen.

**Zwei Schlüssel bekommen mit diesem Issue ihren ersten Leser.** `ui.notifications` und `ui.theme` stehen seit HUM-062 im Schema und werden von der Oberfläche nicht gelesen: `notificationsEnabled` (`app/lib/features/tray/providers/attention.dart`) antwortet fest `true`, und `themeModeProvider` (`app/lib/features/shell/providers/theme.dart`) startet fest auf dunkel. Beiden fehlt nicht der Schlüssel, sondern der Weg, ihn zu erfragen — `configGet` und `SubscribeConfig`, die dieses Issue liefert. Das Leser-Register aus HUM-101 (`daemon/crates/config/tests/config_readers.rs`) führt sie deshalb als `pending(HUM-069)`; mit diesem Bildschirm werden sie wirksam, und ihre Registerzeilen wechseln im selben Commit auf `effective`. Ohne diesen Wechsel zeigt der Zeiger auf ein Issue, das den Schlüssel nicht abdeckt, und das Register sähe nach Nachverfolgung aus, ohne eine zu sein.

**Schema-Erweiterungen** (vom Daemon geliefert, JSON Schema Draft 2020-12 via `schemars`): Jedes Property hat `title` (kurz), `description`, `default`, optional `format` (`uri`, `path`, `duration-secs`, `bytes`, `host-port`), `enum`, `minimum`, `maximum`, `x-tier` (`basic|advanced|expert`), `x-security` (bool: Änderung beeinflusst Sicherheitsgarantien), `x-restart` (bool: wirkt erst nach Session-Neustart). Beispielausschnitt:

```json
"hold": { "type": "object", "title": "Holding", "properties": {
  "timeout_secs": { "type": "integer", "minimum": 10, "maximum": 86400, "default": 300, "format": "duration-secs",
    "title": "Hold timeout", "description": "How long a held request waits for a decision before it is blocked.", "x-tier": "advanced" },
  "body_cap_bytes": { "type": "integer", "default": 33554432, "format": "bytes", "x-tier": "expert", "x-security": true, ... },
  "ask_mode": { "type": "string", "enum": ["ui", "terminal", "none"], "default": "ui", "x-tier": "advanced" } } }
```

**Proto** (falls HUM-062 die RPCs nicht schon definiert hat, exakt so; sonst dessen Definition verwenden und dieses Issue anpassen):

```proto
rpc Config(ConfigRequest) returns (ConfigResponse);
rpc SubscribeConfig(Empty) returns (stream ConfigChanged);
message ConfigRequest { oneof op { SchemaOp schema = 1; GetOp get = 2; SetOp set = 3; ResetOp reset = 4; } }
message GetOp { repeated string keys = 1; }                      // leer = alle
message SetOp { string key = 1; string value_json = 2; SetTarget target = 3; }   // target: GLOBAL | PROJECT
message ResetOp { string key = 1; SetTarget target = 2; }
message ConfigResponse { string schema_json = 1; repeated ConfigValue values = 2; DiagnosticProto diagnostic = 3; }
message ConfigValue { string key = 1; string value_json = 2; Origin origin = 3; bool overridden_by_higher = 4; string higher_origin = 5; }
enum Origin { ORIGIN_UNSPECIFIED = 0; DEFAULT = 1; GLOBAL = 2; PROFILE_GLOBAL = 3; PROFILE_PROJECT = 4; ENV = 5; CLI = 6; }
message ConfigChanged { repeated string keys = 1; Origin origin = 2; }
```

`SetOp` schreibt in die Zieldatei (Global `config.toml` oder Projektprofil `.humanitl/profile.toml`), validiert vorher gegen das Schema (`CONFIG_001` Error bei Typ/Range/Enum-Verstoß, mit `why` = Schema-Fehlermeldung), erhält Kommentare und Reihenfolge in der TOML-Datei (`toml_edit`-Crate, nicht `toml::to_string`). Ist der Schlüssel durch `ENV` oder `CLI` überschrieben, schreibt `Set` trotzdem in die Datei, antwortet aber mit `overridden_by_higher = true`; das UI zeigt das vorher an und deaktiviert das Feld.

**Schema → Widget** (`setting_field.dart`):

| Schema | Widget (aus `packages/ui`) | Verhalten |
|---|---|---|
| `string` ohne format/enum | `HTextField` | Commit bei Blur oder Enter |
| `string`, `enum` | `HSelect` | Commit sofort |
| `string`, `format: uri` | `HUrlField` + „Testen" (nur für `llm.endpoint`: ruft `Sandbox(LlmProbe)` aus HUM-039 und zeigt Modelle oder Diagnostic) | Validierung `Uri.parse`, Schema `http|https` |
| `string`, `format: path` | `HPathField` + Ordner-Button (`file_picker.getDirectoryPath`) | zeigt Existenz-Status |
| `string`, `format: host-port` | `HTextField` mit Mono | Regex `^[^:]+:\d+$` |
| `integer` mit `minimum`/`maximum` | `HNumberField` (Stepper) | Clamp, Fehler unter Feld |
| `integer`, `format: duration-secs` | `HDurationField` | Eingabe `5m`, `300`, `1h30m`; Anzeige humanisiert |
| `integer`, `format: bytes` | `HBytesField` | Eingabe `32MiB`, `256k`, Anzeige humanisiert |
| `boolean` | `HSwitch` | Commit sofort |
| `array` of `string` | `HListEditor` (Chips mit Hinzufügen/Entfernen) | Commit bei jeder Änderung |
| `array` of `object {term, alias}` (`findings.user_terms`) | `HKeyValueEditor` | zwei Spalten |
| `object` (verschachtelt) | `SettingGroup` rekursiv | Untergruppe eingerückt |

Jedes Feld: Titel (13/500), Beschreibung (12, `fg-1`), Widget rechts (Zweispalten-Layout ab 900 px Breite, sonst gestapelt), `OriginBadge` (Mono 11: `default`, `global`, `profile`, `project`, `env`, `cli`), Reset-Icon (nur sichtbar, wenn Origin ≠ default), `x-security` ⇒ kleines `shield-alert`-Icon mit Tooltip „Beeinflusst Sicherheitsgarantien", `x-restart` ⇒ Hinweis „wirkt nach Neustart der Session". Fehlerzustand: rote Unterkante, Diagnostic-`why` unter dem Feld.

**Layout**:

```
┌ Settings ────────────────────────────────────────────────────────────────────┐
│ [🔍 Suche …]                       Ziel: (•) Global ( ) Dieses Projekt        │
│ ┌ Gruppen (links, 220 px) ──┐ ┌ Inhalt ───────────────────────────────────┐ │
│ │ LLM                       │ │ ## Holding                                │ │
│ │ Holding              ●    │ │ Hold timeout        [ 5m      ] global ↺  │ │
│ │ Sandbox                   │ │ Ask mode            [ ui   ▾  ] default   │ │
│ │ Agent                     │ │ ▸ Expert (2)  ⚠ security-relevant         │ │
│ │ Recorder                  │ │                                            │ │
│ │ Preview                   │ │ [Config-Datei öffnen]  ~/.config/…/config │ │
│ │ IPC · UI · Experimental   │ │                                            │ │
│ └───────────────────────────┘ └────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

Tier-Darstellung: `basic`- und `advanced`-Felder direkt; `expert`-Felder je Gruppe in einer `Collapsible` „Expert (n)", Zustand pro Gruppe in `localStorage`-Äquivalent (`shared_preferences`) gemerkt; enthält die Gruppe ein `x-security`-Feld, trägt der Collapsible-Header ein Warn-Icon und beim Aufklappen erscheint einmalig eine Zeile „Diese Einstellungen können die Isolationsgarantien schwächen. Änderungen werden im Audit-Log festgehalten." (`config.changed`-Record aus HUM-050). Punkt neben der Gruppe = enthält Nicht-Default-Werte.

Suche: `settingsSearchProvider` filtert über `key`, `title`, `description` (case-insensitiv, Substring, später fuzzy), zeigt Treffer flach als Liste mit Gruppen-Breadcrumb und Tier-Badge; `expert`-Treffer sind sichtbar, Collapsible spielt keine Rolle. `Esc` leert die Suche.

Ziel-Umschalter (Global / Dieses Projekt) erscheint nur, wenn eine Session mit Projektprofil läuft oder `.humanitl/profile.toml` im gewählten Projekt existiert. `Set` mit `PROJECT` schreibt ins Projektprofil.

„Config-Datei öffnen": `Process.run('xdg-open', [path])`; daneben der Pfad als Mono zum Kopieren.

**Live-Reload** (`reload.rs`): `notify` beobachtet `config.toml`, `rules.yaml` (bereits HUM-027?) und aktive Profildateien; Debounce 300 ms; Neuladen mit Validierung; bei Fehler `CONFIG_002` (Error, `why` mit Zeile/Spalte aus `toml` Fehler, `fix: CopyCommand("humanitl config edit")`) und **die alte Config bleibt aktiv**; bei Erfolg `ConfigChanged { keys }` an alle Subscriber und Audit `config.changed` pro Key mit `origin = file`. Werte mit `x-restart` werden erst beim nächsten `Sandbox(Start)` übernommen.

`secret`-Felder: Es gibt im MVP keine (Tokens werden nicht in der Config gespeichert). Das Schema-Attribut `x-secret` wird trotzdem unterstützt (Darstellung als Passwortfeld, Audit ohne Wert), damit M10 nichts nachrüsten muss.

### Schritte
1. Schema-Attribute im `config`-Crate prüfen/ergänzen, `humanitl config schema` (HUM-070, hier vorziehen als Test) gibt vollständiges Schema mit `x-tier` für jedes Feld aus; Test: kein Feld ohne Tier.
2. Proto `Config`/`SubscribeConfig`, ipc-Handler, `toml_edit`-Schreiben mit Kommentar-Erhalt; Tests.
3. `reload.rs` mit `notify`; Test: Datei ändern ⇒ Event; Datei kaputt ⇒ `CONFIG_002`, alter Wert bleibt.
4. Dart `settings_schema.dart` Parser mit Tests gegen ein eingefrorenes Schema-Fixture.
5. `SettingField` für alle Zeilen der Mapping-Tabelle, Galerie-Seite zeigt jeden Typ.
6. Screen mit Gruppen, Tier-Collapsibles, Origin-Badges, Reset.
7. Suche, Ziel-Umschalter, „Config-Datei öffnen".
8. Live-Update über `SubscribeConfig`.
9. Widget-Tests, Golden `settings_group_with_expert` (HUM-054).

### Tests
Unit (`config`): `every_field_has_tier`, `set_preserves_comments_and_order` (TOML mit Kommentar, `set hold.timeout_secs 600`, Kommentar bleibt, Reihenfolge bleibt), `set_invalid_enum_CONFIG_001`, `set_out_of_range_CONFIG_001`, `origin_env_overrides_file` (`HUMANITL_HOLD__TIMEOUT_SECS=42` ⇒ Origin `Env`, `overridden_by_higher` bei Set), `reload_invalid_keeps_old_CONFIG_002`.
Unit (Dart): `schema_parser_reads_tier_and_security`, `duration_field_parses_1h30m`, `bytes_field_parses_32MiB`.
Widget: `expert_collapsed_by_default`, `search_finds_expert_field`, `env_overridden_field_disabled_with_badge`, `set_calls_daemon_with_json_value`, `reset_visible_only_when_non_default`, `config_changed_event_updates_field`.

### Akzeptanzkriterien
- [ ] `humanitl config schema | jq '[.. | objects | select(has("type") and (has("x-tier") | not))] | length'` ergibt 0 für alle Blatt-Properties.
- [ ] Jedes Feld aus CONVENTIONS.md 3.7 ist im Screen auffindbar (Suche nach dem Schlüssel).
- [ ] Änderung von `hold.timeout_secs` im UI ⇒ `config.toml` enthält den neuen Wert, Kommentare erhalten, Daemon nutzt ihn für die nächste gehaltene Anfrage (Integrationstest mit Fake-Agent: Timeout 15 s ⇒ Block nach 15 s).
- [ ] Änderung von `config.toml` im Editor ⇒ UI aktualisiert innerhalb 1 s.
- [ ] Kaputte `config.toml` ⇒ Diagnostic `CONFIG_002` im Setup-Banner, alte Werte bleiben aktiv.
- [ ] `HUMANITL_HOLD__TIMEOUT_SECS=42 humanitld` ⇒ Feld im UI deaktiviert mit Badge `env`.
- [ ] `expert`-Felder mit `x-security` zeigen das Warn-Icon; Änderung erzeugt `config.changed` im Audit-Log.
- [ ] `ui.notifications = false` unterdrückt die Meldung, und `ui.theme` bestimmt das Erscheinungsbild beim Start; beide Zeilen im Leser-Register stehen auf `effective`, und `docs/CONFIG.md` zeigt für sie in der Spalte „Wirkung" `ja` (HUM-101).
- [ ] Die Befunde des Ladens erreichen die Oberfläche, nicht nur das Journal: Eine `config.toml` mit einem entfallenen Schlüssel (`alias::RETIRED`, heute `limits.idle_timeout_secs`) zeigt ihre `CONFIG_005`-Warnung im Bildschirm, und ein Test belegt es.

### Fallstricke
- `toml::to_string` verwirft Kommentare und ordnet um. Nur `toml_edit::DocumentMut` verwenden.
- Der Datei-Watcher feuert bei Editoren wie vim mehrfach (rename + write). Debounce und beim Reload die Datei vollständig lesen, nicht auf das Event vertrauen.
- Ein Setting mit `x-restart` darf im laufenden Proxy nicht halb wirken (z. B. `hold.body_cap_bytes` mitten in einem Hold). Werte, die der Proxy beim Start einer Session snapshot, im Schema markieren und im UI erklären.
- Generierte UIs neigen zu Wortwüsten. `title` ist maximal 3 Wörter, `description` ein Satz; das Schema ist die Copy-Quelle, also dort kurz halten (Lint auf Länge in `config`-Tests: `title.len() <= 32`, `description.len() <= 160`).
- Tastaturnavigation: alle Felder in einer `FocusTraversalGroup`, Tab-Reihenfolge = visuelle Reihenfolge, Collapsible-Header fokussierbar.
- `SetTarget::PROJECT` in ein Projekt zu schreiben, das der Agent gerade mit `rw` gemountet hat: `.humanitl/` muss in der Sandbox maskiert sein (tmpfs), sonst kann der Agent sein eigenes Profil ändern. In HUM-043 nachprüfen und den Pfad `/work/.humanitl` in die `tmpfs`-Liste des Standardprofils aufnehmen; hier als Test `sandbox_masks_dot_humanitl`.

### Referenzen
- BACKLOG.md ADR-011, Prinzip 8, CONVENTIONS.md 3.7
- JSON Schema 2020-12: https://json-schema.org/draft/2020-12
- `schemars`: https://docs.rs/schemars · `toml_edit`: https://docs.rs/toml_edit · `notify`: https://docs.rs/notify

---

## HUM-070 · CLI config, audit, daemon
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-064, HUM-050, HUM-062 · Blockiert: HUM-053, HUM-055

### Kontext
ADR-013 und Prinzip 9: Alles, was das UI kann, kann die CLI. Dieses Issue ergänzt die Subkommandos `config`, `audit`, `daemon` aus CONVENTIONS.md 3.8. `daemon install` ist außerdem die Grundlage des Erststart-Flows in HUM-053.

### Ziel
`humanitl config get|set|schema|edit`, `humanitl audit verify|export`, `humanitl daemon install|status|logs` funktionieren mit Tabellen- und JSON-Ausgabe, liefern bei Fehlern `Diagnostic`s im CLI-Format und die dokumentierten Exit-Codes.

### Nicht-Ziel
- `humanitl pseudonyms export` (kommt mit M8, Export im MVP nur über das UI).
- Shell-Completions (nach MVP; `clap_complete` vorbereiten).

### Betroffene Pfade
- `daemon/bin/humanitl/src/cmd/config.rs` (neu)
- `daemon/bin/humanitl/src/cmd/audit.rs` (neu)
- `daemon/bin/humanitl/src/cmd/daemon.rs` (neu)
- `daemon/bin/humanitl/src/output.rs` (ändern: `Table`, `Json`, `Diagnostic`-Renderer, falls in HUM-064 nicht vorhanden)
- `packaging/systemd/humanitld.service`, `packaging/systemd/humanitld.socket` (neu, Inhalt in HUM-053; `daemon install` bettet sie via `include_str!` ein)
- `docs/CLI.md` (neu)

### Spezifikation

**Ausgabeformate**: Standard menschenlesbar (Tabellen mit `comfy-table` oder eigener Minimal-Renderer, keine Farben wenn `!isatty` oder `NO_COLOR`), `--json` liefert ein JSON-Objekt pro Aufruf (nicht JSONL), Diagnostics auf stderr als

```
error[CONFIG_001]: Invalid value for hold.timeout_secs
  why: 5 is below the minimum of 10
  fix: humanitl config set hold.timeout_secs 10
```

und mit `--json` als `{"diagnostic": {code, severity, title, why, fix: {kind, ...}}}` auf stdout, Exit-Code nach CONVENTIONS.md 3.8.

**`config`**:
- `config get [KEY] [--json] [--origin]`: ohne KEY alle Blattwerte als `key = value` Tabelle; `--origin` fügt Spalte hinzu. Liest über RPC `Config(Get)`, wenn Daemon erreichbar, sonst lokal über `humanitl-config` (dann Origin ohne `CLI`/`ENV`-Auflösung der Daemon-Instanz, Hinweis auf stderr).
- `config set KEY VALUE [--project]`: VALUE wird nach Schema-Typ geparst (`5m` für duration, `32MiB` für bytes, `true/false`, JSON für Arrays: `'["a","b"]'`); schreibt über RPC oder lokal (`toml_edit`); Ausgabe `hold.timeout_secs = 300 (global)`; Exit 1 mit `CONFIG_001` bei Verstoß.
- `config schema [--json]`: gibt das JSON-Schema aus (immer JSON; ohne `--json` pretty).
- `config edit`: öffnet `$VISUAL`/`$EDITOR` (Fallback `nano`, dann `vi`) auf `config.toml`; nach dem Schließen validieren; bei Fehler Diagnostic `CONFIG_002` mit Zeile und Angebot „erneut öffnen? [y/N]" (einzige interaktive Abfrage in der CLI, nur bei TTY).

**`audit`**:
- `audit verify [--json] [--file PATH]`: ohne `--file` über RPC (Daemon prüft mit Schlüssel und Ankern); mit `--file` lokal ohne Schlüssel und Anker (Kette und Kanonik), Warnung `NoHmacKey` und `NoAnchors`. Ausgabe:

```
audit chain: OK
records:     4213
head:        a3f9…c2e1 (seq 4213, 2026-09-02T10:42:01Z)
anchors:     42 (last at seq 4200)
warnings:    unanchored tail: 13 records
```
Exit 0 bei `Ok`, 4 bei `Broken` (Sicherheitsverletzung), Ausgabe dann `audit chain: BROKEN at seq 4012 (HashMismatch)`.
- `audit export --format jsonl|csv --out FILE [--since TS] [--until TS]`: über RPC; Ausgabe `exported 4213 records to FILE`.

**`daemon`**:
- `daemon install [--bin-dir DIR]`: (1) ermittelt Pfade der Binaries `humanitld` und `humanitl-shim` (neben dem eigenen Binary, oder `--bin-dir`); läuft die CLI aus einem AppImage (`$APPIMAGE` gesetzt), kopiert sie beide nach `~/.local/lib/humanitl/<version>/` und verlinkt `~/.local/lib/humanitl/current`; (2) schreibt `~/.config/systemd/user/humanitld.service` und `humanitld.socket` aus den eingebetteten Templates mit `ExecStart` auf den ermittelten Pfad; (3) `systemctl --user daemon-reload`, `systemctl --user enable --now humanitld.socket`; (4) wartet bis `GetInfo` antwortet (max 5 s); Ausgabe der drei Schritte mit Häkchen. Fehler: `DAEMON_002` „systemd user session nicht verfügbar" (`why`: `XDG_RUNTIME_DIR` fehlt oder `systemctl --user` schlägt fehl, `fix: CopyCommand("loginctl enable-linger $USER")`), `DAEMON_003` „Binary nicht gefunden".
- `daemon status [--json]`: Socket-Pfad, Unit-Status (`systemctl --user is-active`), `GetInfo` (Version, Proto, Uptime, aktive Session, Key-Origin), Exit 2 wenn nicht erreichbar.
- `daemon logs [-f] [-n N]`: `journalctl --user -u humanitld -n N [-f]` als Kindprozess mit durchgereichtem TTY.

### Schritte
1. `output.rs` Renderer (Tabelle, JSON, Diagnostic) mit Tests (Snapshot über `insta`).
2. `config`-Subkommandos, lokaler und RPC-Pfad.
3. `audit`-Subkommandos.
4. `daemon`-Subkommandos; Test von `install` in einem Temp-`HOME` mit gemocktem `systemctl` (PATH-Shim-Skript, das Aufrufe protokolliert).
5. `docs/CLI.md` aus `clap`-Hilfe generieren (`clap_mangen` oder eigenes Skript) plus Beispiele.

### Tests
- `config_get_table_and_json` (Snapshot), `config_set_duration_parsing` (`5m` ⇒ 300), `config_set_invalid_exit_1_with_CONFIG_001`, `config_schema_is_valid_json_schema` (Parsen mit `jsonschema`-Crate).
- `audit_verify_ok_exit_0`, `audit_verify_broken_exit_4` (manipulierte Datei aus HUM-050-Fixture, `--file`), `audit_export_csv_columns`.
- `daemon_install_writes_units_and_calls_systemctl` (Mock protokolliert `daemon-reload`, `enable --now humanitld.socket`), `daemon_install_appimage_copies_binaries` (`APPIMAGE` gesetzt), `daemon_status_exit_2_when_down`.

### Akzeptanzkriterien
- [ ] `humanitl config set hold.timeout_secs 5m && humanitl config get hold.timeout_secs` ⇒ `300`.
- [ ] `humanitl config set hold.ask_mode banana` ⇒ Exit 1, stderr enthält `CONFIG_001`.
- [ ] `humanitl audit verify` nach Manipulation ⇒ Exit 4 und `BROKEN at seq`.
- [ ] `humanitl daemon install` auf einer frischen Debian-VM mit systemd-user-Session ⇒ Socket aktiv, `humanitl daemon status` Exit 0.
- [ ] `NO_COLOR=1` und Pipe ⇒ keine ANSI-Sequenzen (Test mit Regex).
- [ ] `docs/CLI.md` enthält jedes Subkommando mit einem Beispiel.

### Fallstricke
- `systemctl --user` braucht `XDG_RUNTIME_DIR` und einen laufenden `systemd --user`; über SSH ohne Linger fehlt beides. Diagnostic mit `loginctl enable-linger` ist der einzig sinnvolle Fix.
- `daemon install` aus einem AppImage: `ExecStart` darf nie auf den `/tmp/.mount_*`-Pfad zeigen; deshalb das Kopieren.
- `config set` ohne laufenden Daemon schreibt die Datei; der Daemon übernimmt beim nächsten Start oder per Watcher. Beides ist korrekt, die Ausgabe sagt, welcher Fall vorliegt.
- `--json` und Diagnostics: niemals JSON auf stdout und Tabellentext gemischt; bei `--json` geht alles nach stdout als ein Objekt, stderr bleibt leer.
- `audit verify --file` ohne Schlüssel ist eine schwächere Prüfung; die Ausgabe muss das sagen (`warnings: no HMAC key (file mode)`), sonst wiegt sich der Nutzer in Sicherheit.

### Referenzen
- CONVENTIONS.md 3.8, ADR-013, HUM-050 (VerifyReport), HUM-069 (Config-RPC)
- `clap`: https://docs.rs/clap · `toml_edit`: https://docs.rs/toml_edit · `insta`: https://docs.rs/insta

---

## HUM-053 · Packaging deb, AppImage, systemd
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-070 · Blockiert: HUM-055, HUM-060

### Kontext
ADR-010 und BACKLOG.md 4.4 (Härtung der Unit). Der Nutzer installiert ein Paket und hat danach UI, CLI, Daemon, Shim und einen laufenden systemd-Dienst. Flatpak ist bewusst nach dem MVP.

### Ziel
`fastforge release` erzeugt aus einem Tag ein `.deb` (amd64) und ein AppImage. Das `.deb` installiert Bundle, CLI, Daemon, Shim, Desktop-Eintrag, Icon und die systemd-User-Units unter `/usr/lib/systemd/user/`; nach der Installation aktiviert der Nutzer den Dienst mit einem Klick im Setup-Screen (ruft `humanitl daemon install`, das bei vorhandenen System-Units nur `enable --now` ausführt) oder per CLI. Das AppImage enthält dieselben Binaries; `daemon install` kopiert Daemon und Shim heraus. Die Unit ist gehärtet, so weit bwrap es zulässt, und nutzt Socket-Activation.

### Nicht-Ziel
- Flatpak, Snap, RPM, AUR, arm64 (nach MVP; RPM ist mit fastforge fast gratis, aber ungetestet).
- Signierte Pakete / Repository (Release-Job in HUM-060 liefert Checksummen).

### Betroffene Pfade
- `distribute_options.yaml` (neu, Repo-Wurzel)
- `packaging/deb/` (neu): `control`-Template, `postinst`, `prerm`, `humanitl.desktop`, Icons
- `packaging/systemd/humanitld.service`, `humanitld.socket` (neu)
- `packaging/appimage/AppRun` (neu), `packaging/appimage/humanitl.desktop`
- `app/linux/CMakeLists.txt` (ändern: `install(PROGRAMS ...)` für `humanitld`, `humanitl`, `humanitl-shim` in das Bundle)
- `Makefile` oder `justfile` (neu): `just build-daemon`, `just build-app`, `just package`
- `.github/workflows/package.yml` (neu, Platzhalter; Release-Trigger in HUM-060)
- `daemon/bin/humanitld/src/main.rs` (ändern: `LISTEN_FDS`-Übernahme)
- `app/lib/features/setup/` (ändern: Schritt „Dienst installieren" nutzt `FixAction::InstallService`)
- `docs/INSTALL.md` (neu)

### Spezifikation

**Build-Reihenfolge** (`justfile`):
1. `cargo build --release -p humanitld -p humanitl -p humanitl-shim` (Ziel `x86_64-unknown-linux-gnu`; `humanitl-shim` zusätzlich mit `-C target-feature=+crt-static` als statisches Binary, weil es in der Sandbox ohne Garantie über `/usr/lib` läuft).
2. Kopieren nach `app/linux/bundle-extra/`; `CMakeLists.txt` installiert sie nach `<bundle>/bin/`.
3. `flutter build linux --release`.
4. `fastforge package --platform linux --targets deb,appimage`.

**`distribute_options.yaml`**:

```yaml
output: dist/
releases:
  - name: linux
    jobs:
      - name: deb
        package: { platform: linux, target: deb }
      - name: appimage
        package: { platform: linux, target: appimage }
```

Plus `linux/packaging/deb/make_config.yaml` (fastforge-Format) mit den Feldern unten.

**deb-Control**:

```
Package: humanitl
Version: <aus Tag>
Architecture: amd64
Maintainer: Niko Burkert <kreativ@burkert-gestaltung.com>
Section: net
Priority: optional
Homepage: https://github.com/<org>/humanitl
Depends: bubblewrap (>= 0.8), socat, libgtk-3-0, libglib2.0-0, ca-certificates, libayatana-appindicator3-1 | libappindicator3-1
Recommends: gnome-keyring | kwalletmanager
Suggests: opencode
Description: Human-in-the-loop network moderation for AI coding agents
 Runs an AI coding agent in a sandbox without a network interface and
 holds every outbound request for human review.
```

Dateien im Paket: `/usr/lib/humanitl/` (Flutter-Bundle inklusive `bin/humanitld`, `bin/humanitl`, `bin/humanitl-shim`), `/usr/bin/humanitl` (Symlink auf `/usr/lib/humanitl/bin/humanitl`), `/usr/bin/humanitl-app` (Symlink auf das Flutter-Binary), `/usr/share/applications/humanitl.desktop`, `/usr/share/icons/hicolor/{64x64,128x128,256x256,scalable}/apps/humanitl.{png,svg}`, `/usr/lib/systemd/user/humanitld.service`, `/usr/lib/systemd/user/humanitld.socket`, `/usr/share/doc/humanitl/`.

`postinst`: nur `update-desktop-database` und `gtk-update-icon-cache`, **kein** `systemctl --user` (läuft als root, kann User-Units nicht aktivieren). `prerm`: nichts. Die Aktivierung passiert pro Nutzer über `humanitl daemon install`, das bei System-Units unter `/usr/lib/systemd/user/` nur `systemctl --user enable --now humanitld.socket` ausführt und keine Kopie in `~/.config/systemd/user/` anlegt.

**`humanitld.socket`**:

```ini
[Unit]
Description=Humanitl daemon socket

[Socket]
ListenStream=%t/humanitl/daemon.sock
SocketMode=0600
DirectoryMode=0700
RemoveOnStop=yes

[Install]
WantedBy=sockets.target
```

**`humanitld.service`**:

```ini
[Unit]
Description=Humanitl daemon (moderating proxy and sandbox manager)
Documentation=https://github.com/<org>/humanitl/blob/main/docs/SECURITY.md
Requires=humanitld.socket
After=humanitld.socket

[Service]
Type=notify
ExecStart=/usr/lib/humanitl/bin/humanitld
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=info
# --- Härtung (siehe BACKLOG.md 4.4). Jede Zeile hat einen Grund, siehe docs/INSTALL.md#hardening ---
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=no
ReadWritePaths=%h/.local/share/humanitl %h/.config/humanitl %t/humanitl
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
SystemCallArchitectures=native
SystemCallFilter=@system-service @mount
SystemCallErrorNumber=EPERM
# RestrictNamespaces darf NICHT gesetzt werden: bwrap braucht user, mnt, pid, net, ipc, uts Namespaces.
# CapabilityBoundingSet bleibt Standard: bwrap non-setuid nutzt userns, keine Caps nötig.

[Install]
WantedBy=default.target
```

Begründungen (in `docs/INSTALL.md#hardening`): `ProtectHome=no` statt `read-only`, weil der Daemon Projektordner unter `$HOME` mit `rw` in die Sandbox mountet und `ReadWritePaths` keine zur Laufzeit gewählten Pfade kennt; `ProtectSystem=strict` macht den Rest des Systems read-only; `@mount` ist nötig für bwrap; `MemoryDenyWriteExecute` ist mit Rust ohne JIT kompatibel; `RestrictAddressFamilies` erlaubt IPv4/IPv6 nur für den Upstream-Verkehr des Proxys.

**Socket-Activation im Daemon** (`main.rs`): Beim Start `LISTEN_FDS`/`LISTEN_PID` prüfen (`sd-notify`- oder `listenfd`-Crate); ist FD 3 vorhanden, `UnixListener::from_raw_fd(3)` für tonic verwenden, sonst selbst binden (Entwicklungsmodus). `Type=notify`: nach erfolgreichem Start `READY=1` senden (`sd_notify`), sonst startet systemd nicht sauber. Der Proxy-Socket wird immer vom Daemon selbst angelegt (nicht aktiviert), Verzeichnis `%t/humanitl/proxy/` 0700.

**AppImage**: `AppRun` startet das Flutter-Binary; setzt **kein** `LD_LIBRARY_PATH` auf gebündelte GTK-Libs (GTK, GLib, Wayland-Libs werden vom System genommen; nur Flutter-eigene `.so` aus dem Bundle). Enthält `bin/humanitld`, `bin/humanitl`, `bin/humanitl-shim`. `humanitl daemon install` erkennt `$APPIMAGE` und kopiert (HUM-070). Die AppImage-Variante der CLI ist über `./Humanitl.AppImage --cli daemon install` erreichbar (AppRun leitet `--cli …` an `bin/humanitl` weiter).

**Desktop-Datei**: `Exec=humanitl-app %U`, `Icon=humanitl`, `Categories=Development;Network;Security;`, `Keywords=agent;proxy;sandbox;llm;`, `StartupWMClass=humanitl`.

**Erststart** (Setup-Screen, HUM-044): Schritt „Daemon" prüft (1) Socket vorhanden und `GetInfo` antwortet ⇒ grün; (2) sonst Unit-Dateien vorhanden (System oder User) ⇒ Button „Dienst aktivieren" (`FixAction::InstallService`) ⇒ ruft `humanitl daemon install` als Kindprozess und zeigt dessen Ausgabe; (3) sonst Diagnostic `DAEMON_001` „Humanitl-Dienst nicht installiert" mit `fix: CopyCommand("humanitl daemon install")` und Link auf `docs/INSTALL.md`.

**Test-Matrix (manuell, dokumentiert in `docs/INSTALL.md`)**: Debian 13 GNOME Wayland (Intel), Debian 13 KDE Wayland (NVIDIA proprietär), Ubuntu 24.04 GNOME X11. Prüfen: Start, Tray-Icon (GNOME braucht AppIndicator-Extension, Hinweis im Setup), Fenster-Scaling 200 %, Notification, `daemon install`, Sandbox-Start, Impeller-Rendering (Fallback `--no-enable-impeller` dokumentieren).

### Schritte
1. `justfile`, statisches Shim-Binary, `CMakeLists.txt`-Install.
2. Socket-Activation und `sd_notify` im Daemon; lokaler Test mit `systemd-socket-activate`.
3. Unit-Dateien, `docs/INSTALL.md#hardening`; Test: Daemon startet unter der Unit und kann eine Sandbox mit `rw`-Projekt unter `$HOME` starten (sonst ist die Härtung zu streng).
4. deb-Konfiguration, `postinst`, Desktop, Icons; `dpkg -i` auf einer frischen VM (CI: Docker-Container mit systemd ist unzuverlässig; deshalb ein `lintian`-Lauf in CI und der VM-Test manuell, Ergebnis in `docs/INSTALL.md` protokolliert).
5. AppImage mit `AppRun`, `--cli`-Weiterleitung.
6. Setup-Screen-Schritt „Dienst aktivieren".
7. `package.yml` (Build ohne Upload).

### Tests
- `lintian` auf dem `.deb`: keine Errors (Warnings dokumentiert).
- `systemd-analyze --user security humanitld.service` ⇒ Exposure ≤ 4.0 (Wert in `docs/INSTALL.md` festhalten).
- Integration: Daemon unter `systemd-socket-activate -l $XDG_RUNTIME_DIR/humanitl/daemon.sock` starten, `humanitl daemon status` Exit 0.
- Integration: Unter der echten Unit `humanitl sandbox run --work ~/tmp/proj -- touch /work/x` ⇒ Datei existiert (beweist `ProtectHome=no` + bwrap unter der Härtung).
- `humanitl-shim` ist statisch: `ldd bin/humanitl-shim` ⇒ „not a dynamic executable".
- AppImage: `./Humanitl.AppImage --cli daemon status` läuft; `APPIMAGE`-Erkennung im Install-Test aus HUM-070.

### Akzeptanzkriterien
- [ ] `just package` erzeugt `dist/humanitl_<ver>_amd64.deb` und `dist/Humanitl-<ver>-x86_64.AppImage`.
- [ ] Frische Debian-13-VM: `sudo dpkg -i …deb && humanitl daemon install && humanitl daemon status` ⇒ Exit 0; Setup-Screen zeigt Daemon grün.
- [ ] Unter der Unit läuft eine Session mit `rw`-Projekt unter `$HOME` und Escape-Tests 1–3 sind grün (Härtung bricht bwrap nicht).
- [ ] `systemd-analyze security` Wert dokumentiert, `RestrictNamespaces` nicht gesetzt.
- [ ] `lintian` ohne Errors.
- [ ] AppImage startet auf Wayland (GNOME) ohne gebündeltes GTK.

### Fallstricke
- `ProtectHome=read-only` klingt richtig, bricht aber jeden `rw`-Mount eines Projekts unter `$HOME` in bwrap (Bind-Mounts erben die Read-only-Eigenschaft des Daemon-Mount-Namespaces). Deshalb `no` plus `ProtectSystem=strict`. Das ist eine bewusste Abwägung und steht so in der Doku.
- `SystemCallFilter=@system-service` allein blockiert `mount`, `pivot_root`, `unshare` ⇒ bwrap scheitert mit `EPERM`. `@mount` ergänzen; `unshare` ist in `@system-service` enthalten, `setns` ebenfalls, prüfen mit `systemd-analyze syscall-filter`.
- `Type=notify` ohne `sd_notify`-Aufruf lässt den Start nach 90 s fehlschlagen. Der Daemon muss `READY=1` senden, nachdem gRPC lauscht.
- `postinst` läuft als root; `systemctl --user` funktioniert dort nicht. Nie versuchen.
- AppImage mit gebündeltem GTK/GLib bricht auf Wayland und bei abweichenden Mesa-Versionen; nur Flutter-Bibliotheken bündeln. Auf Systemen ohne `libayatana-appindicator3` läuft die App, Tray-Icon fehlt (Diagnostic `UI_001` Info, kein Fehler).
- `libfuse2` wird für AppImage-Mount gebraucht und fehlt auf neuen Distributionen; alternativ `--appimage-extract-and-run` dokumentieren.
- Der Shim muss statisch sein: In der Sandbox liegt `/usr` zwar read-only gebunden, aber ein Profil könnte es weglassen; ein dynamischer Shim würde dann nicht starten und das Fehlerbild wäre irreführend.
- Versionsnummer an genau einer Stelle (`Cargo.toml` Workspace + `pubspec.yaml` synchron durch `just bump`), sonst weichen `GetInfo` und Paket ab.

### Referenzen
- BACKLOG.md ADR-010, 4.4
- fastforge: https://github.com/fastforgedev/fastforge · Flutter Linux build: https://docs.flutter.dev/platform-integration/linux/building
- systemd.exec Härtung: https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html · systemd.socket: https://www.freedesktop.org/software/systemd/man/latest/systemd.socket.html
- AppImage-Portabilität: https://www.industrialflutter.com/blogs/portability-a-case-study-in-flutter-appimage-distribution/

---

## HUM-054 · Golden- und Widget-Tests
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-047, HUM-052, HUM-069 · Blockiert: HUM-055

### Kontext
BACKLOG.md 3.2 Testpyramide und Flutter-Recherche: `alchemist` im CI-Modus (Ahem-Font), damit Linux-Runner und lokale Renders übereinstimmen. Goldens sichern das Design aus Abschnitt 5 gegen Regressionen durch shadcn_flutter-Upgrades.

### Ziel
Ein Golden-Test-Set für die zentralen Widgets in beiden Themes und beiden Sprachen, Widget-Tests für die Interaktionen der Sprints 2 bis 4, alles in CI grün. Ein `just goldens-update` regeneriert kontrolliert.

### Nicht-Ziel
- Integration-Tests unter xvfb (HUM-055 und HUM-036 haben ihre eigenen).
- Pixelgenaue Goldens mit echten Fonts (nur CI-Modus mit Ahem; „platform goldens" bleiben lokal und sind gitignored).

### Betroffene Pfade
- `app/test/goldens/` (neu): eine Datei pro Widget-Familie
- `app/test/flutter_test_config.dart` (neu): `AlchemistConfig` mit `ciGoldensConfig`, `platformGoldensConfig(enabled: false)` in CI, Theme-Wrapper
- `app/test/support/` (neu): `pumpApp(widget, {locale, theme, size})`, `FakeDaemonClient`-Builder mit Szenarien, Fixtures (`fixtures/flows.json`, `fixtures/schema.json`)
- `justfile` (ändern: `goldens`, `goldens-update`)
- `.github/workflows/ci.yml` (ändern: Job `goldens` mit `flutter test --tags golden`)

### Spezifikation

**Konfiguration**: `flutter_test_config.dart` setzt `AlchemistConfig(ciGoldensConfig: CiGoldensConfig(enabled: true), platformGoldensConfig: PlatformGoldensConfig(enabled: !Platform.environment.containsKey('CI')))`. Jedes Golden wird in vier Varianten erzeugt: `dark-en`, `dark-de`, `light-en`, `light-de`, Dateiname `<name>.<variant>.png`. Fenstergröße 1280×800 für Screens, natürliche Größe für Komponenten. Tag `golden` an allen Golden-Tests.

**Golden-Liste** (Datei · Test-Name · Inhalt):

| Datei | Golden-Name | Szenario |
|---|---|---|
| `queue_row_test.dart` | `queue_row_held` | Zeile 36 px, GET, api.github.com, Countdown 0:42, kein Finding |
| | `queue_row_held_findings` | POST, 2 Findings-Chip orange |
| | `queue_row_selected` | 56 px, Zweitzeile mit Größe/Content-Type |
| | `queue_row_hover` | bg-3, Inline-Buttons sichtbar |
| | `queue_group_header` | „github.com (4)" mit Allow/Block |
| `request_card_test.dart` | `request_card_json` | Header aufgeklappt (9), Body JSON-Tree, Finding unterstrichen |
| | `request_card_form` | Form-Felder |
| | `request_card_binary` | Hex-Vorschau |
| | `request_card_large` | 50 MB, Hinweiszeile |
| | `request_card_timed_out` | Banner, deaktivierte Leiste |
| `action_bar_test.dart` | `action_bar_default` | Senden (Akzent) · Edit · Block, Merken-Popover geschlossen |
| | `action_bar_findings` | „Senden mit 2 Funden" amber |
| | `action_bar_hard_block` | deaktiviert |
| | `action_bar_edited` | „Editierte Version senden" |
| | `action_bar_remember_open` | Dauer × Ziel-Raster mit Regelsatz-Vorschau |
| `findings_pause_test.dart` | `findings_pause` | 3 Funde, drei Buttons |
| `editor_test.dart` | `editor_split_findings` | Original/Entwurf, Findings-Rail, Unterstriche |
| | `editor_replaced_glow` | nach „Alle ersetzen" |
| | `editor_mapping_open` | Mapping-Panel ausgeklappt |
| | `editor_json_invalid` | amber Hinweis |
| `domain_panel_test.dart` | `domain_known` | Katalog-Karte npm |
| | `domain_unknown` | gestrichelt, Fetch-Button |
| `history_test.dart` | `history_table` | 12 Zeilen, alle Zustandsfarben je einmal |
| | `history_detail_edited` | Detail mit Tab Mapping |
| `rules_test.dart` | `rules_list` | 6 Regeln, Bundled-Badge, Temporär-Tab |
| `sandbox_test.dart` | `isolation_panel_ok` | drei grün + amber LLM-Zeile |
| | `isolation_panel_failed` | eine rot mit Diagnostic |
| | `isolation_ring_states` | Ring 3/3, 2/3, 0/3 nebeneinander |
| `audit_test.dart` | `audit_status_ok`, `audit_status_broken` | |
| `settings_test.dart` | `settings_group_with_expert` | Holding-Gruppe, Expert eingeklappt mit Warn-Icon |
| | `settings_search_results` | Suche „timeout" |
| | `settings_env_overridden` | deaktiviertes Feld mit `env`-Badge |
| `setup_test.dart` | `setup_checklist` | 4 Punkte, zwei grün, eines mit Diagnostic |
| `ui_gallery_test.dart` | `tokens_palette` | alle Zustandsfarben mit Glyph und Label (Prüfung der Token) |

**Widget-Tests** (ohne Golden), Ergänzung zu den in HUM-047 bis HUM-069 definierten: `shortcuts_map_to_intents` (jede Tastenkombination aus CONVENTIONS.md 3.9 löst genau den Intent aus, Tabelle), `focus_not_stolen_on_new_flow` (Fokus bleibt im Editor, wenn ein Flow ankommt), `queue_not_reordered_under_pointer` (Hover auf Zeile 3, neuer Flow ⇒ Zeile 3 hat denselben FlowId), `notification_sent_on_zero_to_one` (Fake `NotificationService` zählt), `tray_badge_count` (Fake `TrayService`).

**Fixtures**: `fixtures/flows.json` enthält 20 Flows in allen Zuständen, mit Findings, einem Edited-Flow, einem Passthrough, einem Timed-out; `fixtures/schema.json` ist ein eingefrorenes Config-Schema (mit `x-tier`); `FakeDaemonClient.fromFixture()` liefert beides. Die Fixtures werden von `humanitl config schema` und einem Export aus dem Fake-Daemon generiert und im Repo eingecheckt; ein Test prüft, dass `schema.json` mit dem aktuellen Daemon-Schema übereinstimmt (sonst Fixture aktualisieren).

### Schritte
1. `flutter_test_config.dart`, `pumpApp`, Fixtures, Fake-Client-Builder.
2. Goldens Familie für Familie anlegen, jeweils `just goldens-update` und Sichtprüfung der PNGs (in Reviewer-Kommentar dokumentieren).
3. Widget-Tests.
4. CI-Job; `goldens` laufen getrennt vom normalen Test-Job, damit Fehlschläge sofort als Design-Regression erkennbar sind.

### Tests
Die Goldens und Widget-Tests sind der Inhalt. Meta-Test: `golden_variants_complete` prüft, dass zu jedem Golden-Namen vier Varianten-Dateien existieren.

### Akzeptanzkriterien
- [ ] `flutter test --tags golden` grün lokal und in CI (Ahem).
- [ ] 36 Golden-Namen × 4 Varianten = 144 PNG-Dateien unter `app/test/goldens/**`.
- [ ] Änderung eines Tokens (z. B. `held`-Farbe) lässt mindestens `tokens_palette` und `queue_row_held` fehlschlagen.
- [ ] Flutter-Anhebung in `app/.fvmrc`: Golden-Job zeigt die Abweichungen als Diff-Bilder in den CI-Artefakten.

### Fallstricke
- Ahem rendert alle Glyphen als Blöcke; deutsche Textlängen werden nur über Boxbreiten sichtbar. Für Überlauf-Prüfung zusätzlich einen Widget-Test `no_overflow_in_de` mit echtem Font lokal (`platform goldens`) oder mit `debugCheckHasOverflow`-Assertion: `tester.takeException()` muss null sein.
- Animationen: `pumpAndSettle` vor jedem Golden; Countdown-Ring braucht `FakeAsync`/fixierte `Clock`. `Clock`-Injection über `clockProvider`.
- `Image.memory` für Favicons aus dem Katalog: im Test synchron über `precacheImage`, sonst leere Boxen.
- Goldens nie mit lokalem Font committen; `.gitignore` für `**/goldens/platform/`.

### Referenzen
- alchemist: https://pub.dev/packages/alchemist · Flutter Golden-Tests: https://api.flutter.dev/flutter/flutter_test/matchesGoldenFile.html
- BACKLOG.md 3.2 (Testpyramide), Abschnitt 5 (Tokens)

---

## HUM-055 · Demo-Skript M4
Sprint: 4 · Größe: S · Abhängigkeiten: alle Sprint-4-Issues · Blockiert: HUM-060

### Kontext
Jeder Sprint endet mit einem grünen Demo-Skript in CI (BACKLOG.md Abschnitt 8). M4 beweist: Editor, Mapping, Audit, Sprache, Settings und Packaging arbeiten zusammen.

### Ziel
`tests/e2e/m4_trusted_editor.sh` (plus Flutter-`integration_test`) läuft unter xvfb mit echtem Daemon, Fake-Agent (aus HUM-036) und UI. Eine Anfrage mit E-Mail, IBAN und Kundenname wird gehalten, im Editor vollständig pseudonymisiert, gesendet; der Fake-Upstream erhält nur Pseudonyme; History zeigt „Editiert"; Mapping enthält drei Einträge; die Audit-Kette verifiziert; ein Export enthält keine Originale; die Sprache lässt sich umschalten; ein Setting wirkt live; das gebaute `.deb` besteht `lintian`.

### Nicht-Ziel
- VM-Installationstest (manuell, HUM-053).

### Betroffene Pfade
- `tests/e2e/m4_trusted_editor.sh` (neu)
- `app/integration_test/m4_trusted_editor_test.dart` (neu)
- `tests/e2e/fixtures/m4_request.json` (neu): POST an `https://api.example.test/tickets` mit Body `{"customer":"Müller GmbH","contact":"anna@mueller.de","iban":"DE89370400440532013000","note":"…"}`
- `.github/workflows/ci.yml` (ändern: Job `e2e-m4`)

### Spezifikation

Ablauf des Shell-Skripts (jeder Schritt mit `set -euo pipefail`, Ausgabe `[m4] step N ok`):

1. Build: `just build-daemon`, `flutter build linux --debug`.
2. Temp-`XDG_*`-Verzeichnisse, Fake-Keyring aus (Datei-Fallback erwartet, `KEYS_001` wird toleriert und im Log geprüft).
3. `config.toml` schreiben: `hold.timeout_secs = 120`, `findings.user_terms = [{term="Müller GmbH", alias="Client-A"}]`, `ui.language = "en"`, `audit.anchor_every = 5`.
4. Daemon starten (`humanitld --socket $XDG_RUNTIME_DIR/humanitl/daemon.sock`), auf `humanitl daemon status` warten (Exit 0, ≤ 5 s).
5. Fake-Upstream (axum, aus HUM-017 als Binary `fake-upstream`) auf `127.0.0.1:8443` mit der Daemon-CA signiertem Leaf; Regel `allow host ip:127.0.0.1 port 8443`? Nein: die Anfrage soll gehalten werden. Stattdessen Host `api.example.test` über `--resolve`-Äquivalent im Daemon: Test-Setting `experimental.static_hosts = { "api.example.test" = "127.0.0.1:8443" }` (nur in Test-Builds, `expert`, Diagnostic-Warnung beim Start).
6. Session starten: `humanitl run --work $TMP/proj --agent fake -- fake-agent --request tests/e2e/fixtures/m4_request.json` (Fake-Agent sendet über `HTTP_PROXY`).
7. Integration-Test (Dart) übernimmt: wartet auf Queue = 1; prüft Button „Send with 3 findings"; drückt `E`; klickt „Replace all with pseudonyms"; prüft, dass der Entwurf `Client-A`, `<EMAIL_1>`, `<IBAN_1>` enthält und keinen Originalwert; öffnet Mapping-Panel, zählt 3 Zeilen mit maskierten Werten `a***@m***.de`, `DE89****3000`, `M***H`; sendet mit `Ctrl+Enter`; wartet auf History-Zeile mit Chip „Edited".
8. Shell: Fake-Upstream-Log prüfen: Body enthält `Client-A`, `<EMAIL_1>`, `<IBAN_1>`; enthält **nicht** `anna@`, `DE89`, `Müller`; `content-length` == Bytelänge; kein `transfer-encoding`.
9. `humanitl audit verify` ⇒ Exit 0; `humanitl audit export --format jsonl --out $TMP/audit.jsonl`; `grep -c 'anna@\|DE89\|Müller' $TMP/audit.jsonl` ⇒ 0; `grep -c '"kind":"pseudonym.created"'` ⇒ 3; `grep -c '"kind":"audit.anchor"'` ≥ 1.
10. Tamper: Zeile 3 der Audit-Datei ändern, `humanitl audit verify` ⇒ Exit 4. Datei zurücksetzen.
11. Sprache: `humanitl config set ui.language de`; Integration-Test prüft, dass der Allow-Button „Senden" heißt (Live-Reload über `ConfigChanged`).
12. Setting live: `humanitl config set hold.timeout_secs 10`; Fake-Agent sendet zweite Anfrage; nach ≤ 12 s ist sie `timed_out` in `humanitl flows list --json`.
13. Packaging: `just package` (nur deb in CI, AppImage optional), `lintian dist/*.deb` ohne `E:`.
14. Aufräumen, Exit 0.

Der Dart-Integration-Test kommuniziert mit dem Shell-Skript über Dateien in `$TMP/steps/` (Schritt-Marker), damit die Reihenfolge deterministisch ist.

### Schritte
1. `experimental.static_hosts` (Test-only, hinter Cargo-Feature `test-hooks`, in Release-Builds nicht vorhanden).
2. Fixture, Fake-Agent-Option `--request`.
3. Shell-Skript Schritte 1–6, 8–14.
4. Dart-Integration-Test Schritte 7 und 11.
5. CI-Job `e2e-m4` mit `xvfb-run -a`, Artefakte: Screenshots bei Fehlschlag, Daemon-Log, Audit-Datei.

### Tests
Das Skript ist der Test. Zusätzlich `e2e_m4_fails_when_original_leaks`: ein absichtlich falsch konfigurierter Lauf (Ersetzung übersprungen) muss in Schritt 8 rot werden (Negativprobe des Skripts, einmal manuell dokumentiert, nicht in CI).

### Akzeptanzkriterien
- [ ] Job `e2e-m4` grün auf `ubuntu-latest` in unter 10 Minuten.
- [ ] Bei Fehlschlag liegen Screenshot, `humanitld.log`, `audit.jsonl` und `fake-upstream.log` als Artefakte vor.
- [ ] Schritte 8, 9 und 10 sind die Sicherheitsbeweise des Sprints und dürfen nicht mit `|| true` abgeschwächt werden (Review-Checkliste).

### Fallstricke
- Der Fake-Upstream muss ein Zertifikat der Daemon-CA für `api.example.test` haben, sonst scheitert die MITM-Upstream-Verbindung; im Test die CA aus `$XDG_DATA_HOME/humanitl/ca/` verwenden und dem Fake-Upstream als Server-Zert ein damit signiertes Leaf geben. Alternativ akzeptiert der Daemon im `test-hooks`-Feature ein zusätzliches Root-Zertifikat für Upstream-Prüfung (`experimental.extra_upstream_ca`). Beides nur im Test-Build.
- `static_hosts` darf niemals in Release-Builds kompiliert sein; ein Test in CI prüft `strings target/release/humanitld | grep -c static_hosts` == 0.
- xvfb und Notifications: `flutter_local_notifications` braucht D-Bus; im CI `dbus-run-session` um das Skript wickeln, sonst hängt Schritt 7.
- Timeouts in Schritt 12 großzügig prüfen (`≤ 12 s`), CI-Runner sind langsam.

### Referenzen
- HUM-036 (Fake-Agent, e2e-Aufbau), HUM-017 (Fake-Upstream), HUM-050, HUM-053
- Flutter integration_test auf Linux: https://docs.flutter.dev/testing/integration-tests


## HUM-077 · Ein-Klick-Installation
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-053, HUM-075, HUM-063 · Blockiert: HUM-060

### Kontext
Prinzip 9. Die Zweiteilung UI/Daemon ist eine Architekturentscheidung (ADR-003, ADR-010), darf aber für den Nutzer nicht sichtbar sein. Ein Paket, ein Klick, fertig.

### Ziel
Das `.deb` installiert UI, `humanitld`, `humanitl`, `humanitl-shim`, Profile und die user unit nach `/usr/lib/systemd/user/`. Beim ersten Start prüft die App den Dienst; fehlt er oder läuft er nicht, zeigt der Setup-Screen genau eine Karte „Hintergrunddienst aktivieren" mit einem Button, der `FixAction::InstallService` ausführt (`systemctl --user enable --now humanitld.socket`), ohne Terminal. Das AppImage legt Binaries nach `~/.local/lib/humanitl/<version>/`, die Unit nach `~/.config/systemd/user/`, aktualisiert beides bei Versionswechsel und räumt alte Versionen auf. `humanitl doctor` bestätigt den Zustand.

### Nicht-Ziel
Kein Flatpak (Post-MVP). Keine systemweite Unit (nur user). Kein Autostart des UI.

### Betroffene Pfade
- `packaging/deb/` (Control, postinst ohne Root-Aktionen außer Dateien), `packaging/systemd/humanitld.socket`, `humanitld.service`
- `packaging/appimage/AppRun` (Self-Install-Logik)
- `daemon/bin/humanitl/src/cmd/daemon.rs` (`install`, `uninstall`, `status`)
- `app/lib/features/setup/widgets/service_card.dart` (neu)

### Spezifikation
- `InstallService`-Ablauf im UI: `humanitl daemon install` als Kindprozess (kein Root), Ausgabe als Fortschritt; Erfolg ⇒ Karte wird grün und verschwindet nach 2 s; Fehler ⇒ Diagnostic `DAEMON_005` mit dem exakten `systemctl`-Befehl zum Kopieren.
- Socket-Activation: `humanitld.socket` lauscht auf `%t/humanitl/daemon.sock`; erster Client startet den Dienst. Damit ist „Dienst läuft nicht" ein Zustand, den der Nutzer nie sieht, solange die Unit aktiviert ist.
- AppImage: `AppRun` vergleicht `~/.local/lib/humanitl/current` mit der eigenen Version; bei Abweichung kopiert es Binaries, schreibt die Unit mit absoluten Pfaden, `systemctl --user daemon-reload`, `restart humanitld.socket`. Deinstallation `humanitl daemon uninstall --purge-binaries`.
- Versionscheck: UI vergleicht `GetInfo.daemon_version` mit der eigenen; bei Abweichung Karte „Dienst neu starten" (ein Klick, `systemctl --user restart`).

### Schritte
1. Unit-Dateien mit Härtung aus HUM-053 und Socket-Activation.
2. `daemon install|uninstall|status` mit Diagnostics.
3. `service_card.dart` und Setup-Verdrahtung; Widget-Test mit Fake-Prozess.
4. AppImage-`AppRun`; Test in Docker-Container mit systemd-user (`--privileged` Job, nur nightly).
5. Doctor-Zeile 6 nutzt `daemon status --json`.

### Tests
- `daemon_cmd::tests::install_writes_unit_and_enables` (Fake-`systemctl` im PATH, Aufrufe geloggt).
- Widget-Test: Karte erscheint bei `DAEMON_001`, Klick ruft Install, Karte verschwindet bei Erfolg.
- Nightly: frisches Debian-Image, `.deb` installieren, UI-Start unter xvfb, Setup ohne Fehlerkarte nach dem Klick.

### Akzeptanzkriterien
- [ ] Frisches System: `.deb` installieren, App starten, ein Klick, Sandbox-Start möglich; kein Terminal nötig.
- [ ] AppImage: erster Start richtet Unit ein, zweiter Start mit neuer Version aktualisiert sie.
- [ ] `humanitl doctor` zeigt Dienst ok.
- [ ] Deinstallation entfernt Unit, Socket und Binaries; `doctor` zeigt danach `DOCTOR_006` mit Fix.

### Fallstricke
- `systemctl --user` braucht `DBUS_SESSION_BUS_ADDRESS`/`XDG_RUNTIME_DIR` im Kindprozess; aus einem AppImage heraus können sie fehlen. Aus der Umgebung des UI durchreichen, sonst Diagnostic.
- `ProtectHome=read-only` in der Unit bricht bwrap-Bind-Mounts von Projekten unter `$HOME`. `ReadWritePaths=%h` ist zu breit; Lösung aus HUM-053: `ProtectHome=tmpfs` plus `BindPaths=` pro Session ist nicht dynamisch möglich, deshalb `ProtectHome=no` und stattdessen `ProtectSystem=strict`, `PrivateTmp`, `NoNewPrivileges`; im SECURITY.md begründen.
- Alte AppImage-Versionen unter `~/.local/lib/humanitl/` nie löschen, während der Dienst läuft; erst nach `restart`.

### Referenzen
BACKLOG.md Prinzip 9, ADR-010, 4.4; HUM-053, HUM-075; systemd socket activation (https://www.freedesktop.org/software/systemd/man/systemd.socket.html).


## HUM-078 · Paritäts-Tabelle und CI-Check
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-070, HUM-064, HUM-003 · Blockiert: HUM-059

### Kontext
ADR-018: UI und CLI sollen dieselben Fähigkeiten haben; der Kern hat eine Schnittstelle. Ohne mechanische Prüfung driftet das. Die Tabelle wird generiert, nicht gepflegt.

### Ziel
`cargo xtask docs` erzeugt `docs/reference/parity.md` mit einer Zeile pro RPC: RPC-Name, CLI-Subkommando, UI-Ort. Quellen: Proto-Descriptor (Service-Methoden), clap-Struktur (jedes Subkommando trägt `#[command(long_about)]` plus ein Marker-Attribut `rpc = "Humanitl.Decide"` über eine kleine `humanitl_cli::Parity`-Tabelle), UI-Registry `app/lib/core/parity.dart` (`const parity = { 'Humanitl.Decide': 'intercept/action_bar', … }`). CI-Job `parity-check` schlägt fehl, wenn ein RPC keine CLI-Zeile hat; fehlende UI-Zeilen werden als `warn` ausgegeben.

### Nicht-Ziel
Keine automatische Generierung von CLI-Subkommandos aus der Proto (bewusst: die CLI soll ergonomisch sein, nicht generisch).

### Betroffene Pfade
- `daemon/xtask/src/parity.rs` (neu)
- `daemon/bin/humanitl/src/parity.rs` (neu): `pub static PARITY: &[(&str, &str)]` (RPC, Subkommando)
- `app/lib/core/parity.dart` (neu)
- `docs/reference/parity.md` (generiert)
- `.github/workflows/ci.yml` (Job `parity-check`)

### Spezifikation
Tabellenformat:
| RPC | CLI | UI |
|---|---|---|
| `Humanitl.Decide` | `humanitl flows decide <id> allow|block [--note]` | `intercept/action_bar` |
Ausnahmen (RPCs ohne CLI-Sinn, z. B. `Terminal`-Stream) stehen in `xtask/parity_exempt.toml` mit Begründung; die Liste wird in der Tabelle als Abschnitt „Ausnahmen" ausgegeben.

### Schritte
1. Proto-Descriptor per `prost-reflect` laden, Methoden listen.
2. `PARITY`-Tabelle in der CLI, Dart-Registry.
3. Generator, Ausgabe deterministisch sortiert.
4. CI-Job: generieren, `git diff --exit-code`, dann Prüfung fehlender CLI-Einträge.

### Tests
- `xtask::parity::tests::missing_cli_fails` (Fixture-Descriptor mit einem RPC ohne Eintrag).
- `xtask::parity::tests::exempt_listed`.

### Akzeptanzkriterien
- [ ] `docs/reference/parity.md` existiert, ist eingecheckt, deckt alle RPCs ab.
- [ ] Neuer RPC ohne CLI-Zeile bricht `parity-check`.
- [ ] Ausnahmen haben Begründung.

### Fallstricke
- Die Dart-Registry kann nicht aus Rust gelesen werden; der Generator parst die `.dart`-Datei mit einer Regex auf `'Humanitl.X': '…'`; Format deshalb strikt halten.
- Streams (`Subscribe`, `Terminal`, `Browser`) haben oft kein sinnvolles CLI; `flows watch` als CLI-Entsprechung für `Subscribe` trotzdem anbieten.

### Referenzen
BACKLOG.md ADR-018; `docs/ARCHITECTURE.md` 3b; HUM-059.


## HUM-079 · Rücktausch von Pseudonymen in Text-Antworten
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-048, HUM-026 · Blockiert: HUM-055

### Kontext
Review-Befund: Wer `<EMAIL_1>` in die Anfrage schreibt, bekommt Antworten, die `<EMAIL_1>` enthalten. Ohne Rücktausch sind Pseudonyme für den Agenten unbrauchbar und landen in `/work`. Das Session-Mapping (HUM-048) existiert bereits, der einfache Fall ist billig.

### Ziel
Nicht-gestreamte Antworten mit Content-Type `text/*`, `application/json`, `application/xml`, `application/x-www-form-urlencoded` werden nach vollständigem Empfang host-seitig durchsucht; jedes Pseudonym der Session wird durch das Original ersetzt, `Content-Length` neu berechnet, dann an den Agenten geliefert. Gestreamte Antworten (SSE, `stream=true`) und Binärdaten bleiben unverändert und tragen `X-Humanitl-Pseudonyms: untranslated`. Der Recorder speichert beide Fassungen, History zeigt einen Umschalter.

### Nicht-Ziel
Kein Rücktausch in gestreamten Antworten (M9). Kein Rücktausch von Secrets (Tokens werden nie zurückgetauscht, nur PII/UserTerm/Custom). Keine Heuristik für veränderte Pseudonyme; erkannt werden die exakte und die URL-encodierte Form.

### Betroffene Pfade
- `daemon/crates/proxy/src/pseudonym_reverse.rs` (neu)
- `daemon/crates/proxy/src/handler.rs` (Response-Pfad, nach Puffern, vor Senden)
- `daemon/crates/recorder/` (`messages.body_translated`, Migration V3)
- `daemon/crates/config/src/schema.rs`: `pseudonyms.translate_responses: bool` (Default true, Tier `advanced`), `pseudonyms.max_response_bytes` (Default 8 MiB)
- `daemon/crates/config/src/model.rs` (der Vermerk `x-pending-issue = "HUM-079"` an `pseudonyms.translate_responses` und `pseudonyms.max_response_bytes` entfällt) und `daemon/crates/config/tests/config_readers.rs` (beide Registerzeilen wechseln auf `effective`). Das Leser-Register aus HUM-101 führt die zwei Schlüssel heute als `pending(HUM-079)`, weil dieses Issue ihnen den ersten Leser gibt; sein Test vergleicht Register und Schema und wird rot, solange nur eine Seite nachgezogen ist
- `app/lib/features/history/widgets/response_view.dart` (Umschalter „Original / Übersetzt")

### Spezifikation
- Nur wenn der Flow `AllowEdited` war und mindestens ein Pseudonym erzeugt hat; sonst kein Scan.
- Ersetzung über `aho_corasick` mit allen Pseudonymen der Session (exakt und `percent_encoding`-Form), längste zuerst, keine Überlappung.
- Antwort > `pseudonyms.max_response_bytes` ⇒ unübersetzt mit Header und Diagnostic `PROXY_006` (Warnung) im Flow.
- `Content-Encoding: gzip|br` wird vor dem Scan dekomprimiert (Ratio-Limit aus `limits`), danach ohne Kompression gesendet (`Content-Encoding` entfernt, `Content-Length` gesetzt).
- Secrets (`FindingKind::ApiKey`, `Jwt`) sind ausgeschlossen; ihr Original ist nicht im Klartext gespeichert (HUM-048).

### Schritte
1. `pseudonym_reverse.rs`: reine Funktion `translate(body: &[u8], map: &PseudonymMap) -> Cow<[u8]>` mit Tests.
2. Handler-Anbindung im gepufferten Response-Pfad, Content-Length, Header, Dekompression.
3. Recorder-Migration und History-Umschalter.
4. e2e-Erweiterung HUM-055: Fake-Upstream echo't die Anfrage, Agent sieht Original-E-Mail.

### Tests
- `pseudonym_reverse::tests::exact_and_url_encoded`, `secrets_never_reversed`, `no_map_no_change`, `overlapping_longest_wins`.
- Integration: JSON-Antwort mit `<EMAIL_1>` ⇒ Agent erhält Original; SSE-Antwort ⇒ unverändert plus Header; gzip-Antwort ⇒ übersetzt, unkomprimiert.

### Akzeptanzkriterien
- [ ] Echo-Upstream liefert Original-Werte an den Agenten zurück.
- [ ] Secrets bleiben pseudonymisiert.
- [ ] Gestreamte Antwort trägt `X-Humanitl-Pseudonyms: untranslated`.
- [ ] History zeigt beide Fassungen; Audit vermerkt `translated=true`.
- [ ] `pseudonyms.translate_responses` und `pseudonyms.max_response_bytes` haben einen Leser: `translate_responses = false` schaltet den Rücktausch ab, eine Antwort über `max_response_bytes` bleibt unübersetzt; beide Zeilen im Leser-Register stehen auf `effective`, und `docs/CONFIG.md` zeigt für sie in der Spalte „Wirkung" `ja` (HUM-101).

### Fallstricke
- Nach dem Rücktausch enthält die Antwort wieder PII; der Recorder speichert sie, das ist gewollt und im Audit vermerkt.
- Kein Rücktausch in Headern (`Set-Cookie`, `Location`); nur Body.
- `aho_corasick` mit leerer Musterliste panict in manchen Versionen; Leerfall vorher abfangen.

### Referenzen
BACKLOG.md ADR-008, Abschnitt 9 M9; HUM-048; aho-corasick (https://docs.rs/aho-corasick).

---

## HUM-090 · Paritaetsluecke zwischen CLI, RPC und UI
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-027, HUM-064, HUM-036 · Blockiert: HUM-078

### Kontext
Prinzip 10 (BACKLOG.md 52) und ADR-018 sagen zu, dass jede Fähigkeit genau einmal als RPC existiert, dass UI und CLI austauschbare dünne Clients derselben Proto sind und dass jedes Issue, das einen RPC einführt, das CLI-Subkommando im selben Issue mitliefert. Für `Humanitl.Decide` gilt das heute nicht: `DecideRequest` trägt `repeated string flow_ids = 1` (`proto/humanitl/v1/humanitl.proto:574`) und `Rule remember = 5` (`:592`), der Dienst setzt beides um (`daemon/crates/ipc/src/server.rs:876-905`: erst die Regel, dann jede Id, Rücknahme der Regel, wenn nichts wirkte), der Fake identisch (`daemon/crates/ipc/src/fake/mod.rs:246-293`). Die Kommandozeile kennt nur eine Id und keine Regel: `FlowsCmd::Decide { id, verdict, note }` (`daemon/bin/humanitl/src/cli.rs:235-245`) und `flow_ids: vec![id.to_owned()], remember: None` (`daemon/bin/humanitl/src/cmd/flows.rs:336-339`). Die Zusage ist also nicht bloß unvollständig, sie ist unwahr: Der Server kann es, ein Client bekommt es nur mit der Maus.

Messbar ist die Folge an einer Stelle, an der Belege verloren gehen. Die Oberfläche hängt beim Merken `createdFrom: flow.id` an die Regel (`app/lib/features/intercept/rule_sentence.dart:117`); der Regel-Bildschirm zeigt daraus das Herkunfts-Abzeichen, das zurück auf die Anfrage springt (`app/lib/features/rules/widgets/rule_row.dart:378-395`). Über die Kommandozeile entsteht dieselbe Regel nur über den Umweg `rules add`, und `rule_from_args` kennt kein Herkunftsfeld (`daemon/bin/humanitl/src/cmd/rules.rs:632`). Genau so läuft das M2-Skript: erst `rules add --expires session`, dann zwölf Einzelentscheidungen (`tests/e2e/m2_first_decision/run.sh:337-375`). Die Regel trägt kein `created_from_flow_id`, das Abzeichen hat für sie nichts anzuzeigen, und der Lauf kann nicht zeigen, was er zeigen soll.

Zwei Dinge werden dabei oft falsch erzählt, und dieses Issue behauptet sie nicht. **Erstens** nutzt `repeated flow_ids` kein einziger Client: `Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember})` (`app/lib/core/ipc/daemon_client.dart:39`) und `..flowIds.add(flowId.value)` (`app/lib/core/ipc/convert.dart:501-502`) schicken je eine Id, und die Oberfläche schleift selbst über die Flows und hängt `remember: i == 0 ? rule : null` an den ersten Aufruf (`app/lib/features/intercept/providers/decision.dart:615-620`). Der Unterschied zwischen Oberfläche und Kommandozeile ist nicht eine Stapel-Anfrage, sondern dass die Oberfläche die Schleife und das Anlegen der Regel selbst fährt und die Kommandozeile beides nicht hat. **Zweitens** meldet keine Prüfung die Lücke, auch nicht die geplante: Der CI-Job `parity-check` ruft `scripts/ci/parity-placeholder.sh` (`.github/workflows/ci.yml:572-582`), das Skript endet mit `exit 0`, solange `daemon/xtask/src/parity.rs` fehlt — und es fehlt, `daemon/xtask/src/` enthält nur `main.rs`. HUM-078 vergleicht auf RPC-Ebene („CI-Job `parity-check` schlägt fehl, wenn ein RPC keine CLI-Zeile hat", `backlog/sprint-4.md:1614`), und seine Beispielzeile `| Humanitl.Decide | humanitl flows decide <id> allow oder block [--note] | intercept/action_bar |` (`:1629`) ist der heutige Stand: grün, mit Lücke.

Zu klären ist außerdem ein Widerspruch im Repository. Der Doc-Kommentar `daemon/bin/humanitl/src/cmd/flows.rs:308-311` nennt die Ein-Id-Regel Absicht („auf der Kommandozeile wäre er die bequeme Art, versehentlich mehr freizugeben als gemeint"), `backlog/CONVENTIONS.md:1305-1318` (4.22) nennt dieselbe Sache eine Paritätslücke, die in ein eigenes Issue gehört. Beides stimmt für verschiedene Hälften: Die Sorge gilt einem Stapel, den niemand einzeln benannt hat; für `--remember` gibt es nirgends eine Begründung. Dieses Issue löst den Widerspruch auf, statt eine der beiden Stellen zu überschreiben.

### Ziel
`humanitl flows decide` entscheidet mehrere ausdrücklich genannte Flows in einer einzigen `Decide`-Anfrage und legt dabei auf Wunsch die Regel an, die die Oberfläche an derselben Stelle anlegt — mit `created_from_flow_id`, damit die Herkunft der Regel auch ohne Maus belegt ist. Der Vertrag, der Daemon und der Fake bleiben, wie sie sind; es entsteht nur der Client, der sie nutzt.

### Nicht-Ziel
Keine Massenentscheidung über einen Filter (`decide --filter state:held`, `--all`): Das ist genau die bequeme Art, mehr freizugeben als gemeint, vor der `flows.rs:308-311` warnt, und sie bleibt ausgeschlossen. Kein `allow_edited` auf der Kommandozeile (der Vertrag lässt dafür genau eine Id zu, `daemon/crates/ipc/src/validate.rs:67-79`, und einen Editor gibt es im Terminal nicht). Kein Flag für `acknowledge_findings` (`humanitl.proto:594`): Das Feld liest heute kein Dienstpfad, ein Flag würde eine Wirkung versprechen, die es nicht gibt; es kommt mit HUM-049, der ihm einen Leser gibt. Keine Paritäts-Tabelle und kein Generator — das ist HUM-078; eine mechanische Prüfung auf Feld-Ebene (jedes Proto-Feld einer Anfrage hat eine CLI-Entsprechung) baut auch dieses Issue nicht, sie wird in `CONVENTIONS.md` 4.22 ausdrücklich als offene Grenze festgehalten, damit die nächste Lücke dieser Art nicht für geprüft gehalten wird.

### Betroffene Pfade
- `daemon/bin/humanitl/src/cli.rs:229-245`: `FlowsCmd::Decide` bekommt `--also ID` (wiederholbar) und den Argumentsatz `RememberArgs` (neu, präfigierte Flags)
- `daemon/bin/humanitl/src/cmd/flows.rs:306-381`: `decide()`, Ausgabe je Flow, JSON-Form, Doc-Kommentar neu
- `daemon/bin/humanitl/src/cmd/rules.rs:632`: `rule_from_args` wird `pub(crate)`
- `daemon/bin/humanitl/tests/cli.rs`: Prozesstests gegen `FakeServer`
- `tests/e2e/lib.sh:424-431`: Helfer `flow_decide`
- `tests/e2e/m2_first_decision/run.sh:337-375`: Schritt 2
- `backlog/CONVENTIONS.md:478` (Signaturzeile) und `:1305-1318` (4.22)
- `backlog/sprint-4.md:1629`: Beispielzeile der Paritäts-Tabelle in HUM-078

Unberührt, weil dort nichts fehlt: `proto/humanitl/v1/humanitl.proto`, `daemon/crates/ipc/src/server.rs`, `daemon/crates/ipc/src/fake/mod.rs`, `daemon/crates/ipc/src/validate.rs`, `app/`.

### Spezifikation
Aufrufform, positional unverändert:

```
humanitl flows decide <ID> allow|block [--also <ID>]... [--note TEXT]
    [--remember --remember-host PATTERN [--remember-expires WHEN]
     [--remember-path P] [--remember-method M]... [--remember-note TEXT]]
```

- `--also ID` ist wiederholbar und nennt jede weitere Id einzeln. Alle Ids gehen in **eine** `DecideRequest` (`flow_ids` in der Reihenfolge der Kommandozeile, `<ID>` zuerst). Eine Id, die zweimal vorkommt, ist `CLI_004` mit Exit 1 vor dem Aufruf; sonst käme sie als zweites, nicht angewandtes Ergebnis zurück und sähe aus wie ein Fehlschlag.
- `--remember` schaltet `DecideRequest.remember` ein und verlangt `--remember-host`. Die Aktion der Regel kommt aus dem Verdikt (`allow` ⇒ `allow`, `block` ⇒ `block`); es gibt kein `--remember-action`, damit ein Block nie eine Freigabe-Regel anlegt. `--remember-expires` ohne Angabe ist `session`; eine dauerhafte Regel schreibt `rules.yaml` und muss deshalb ausgesprochen werden.
- Die Regel entsteht nicht in der CLI: `RememberArgs` wird auf `RuleArgs` abgebildet, `rules::rule_from_args(&args, None)` baut die Wire-Form, danach setzt `decide()` `created_from_flow_id` auf die erste Id. Eine zweite Regelbau-Logik im Binary wäre Fachlogik im Client (`docs/ARCHITECTURE.md` 3b).
- Reihenfolge und Rücknahme bleiben Sache des Dienstes (`server.rs:876-905`): Scheitert das Anlegen, wird nichts entschieden; wirkte keine Entscheidung, wird die Regel zurückgenommen. Die CLI wiederholt das nicht, sie meldet nur, was zurückkam.
- Textausgabe: je Ergebnis eine Zeile `<verdict> <short_id>` wie heute (`short_id` sind die ersten 8 Zeichen). Nicht angewandte Flows stehen mit ihrem `Diagnostic` des Dienstes auf stderr. Mit `--remember` folgt zuletzt `remembered <short rule id> <action> <host> <expires>`.
- `--json`: ein Objekt mit `results` (je Id `flow_id`, `decision`, `applied`, bei Ablehnung `diagnostic`), `note` und `created_rule` (die Regel des Dienstes samt `rule_id` und `created_from_flow_id`, ohne `--remember` `null`). Die alte Ein-Objekt-Form entfällt; sie wird heute von niemandem gelesen (`tests/e2e/lib.sh:427-429` verwirft stdout).
- Exit: `0` nur, wenn jeder genannte Flow entschieden wurde; `1`, sobald einer abgelehnt wurde, auch wenn andere entschieden wurden und die Regel steht — die Ausgabe nennt dann beides. `2` bleibt der nicht erreichbare Daemon.
- Der Doc-Kommentar `flows.rs:306-314` wird neu geschrieben: Die Sicherung gegen versehentliche Freigaben liegt jetzt darin, dass jede Id einzeln genannt wird, jede Entscheidung eine eigene Zeile bekommt und das Host-Muster nie erraten wird — nicht mehr darin, dass es nur eine Id gibt.

### Schritte
1. `RememberArgs` und `--also` in `cli.rs`, Hilfetexte, Abbildung auf `RuleArgs`.
2. `rule_from_args` auf `pub(crate)`, `decide()` auf Ergebnisliste umbauen, `created_from_flow_id` setzen, JSON-Form, Exit-Regel, Doc-Kommentar.
3. Prozesstests in `tests/cli.rs`.
4. M2-Schritt 2 auf einen Aufruf ziehen, Erzähltext `run.sh:340-343` und die Regel-Prüfungen `:345-360` auf die Regel aus `--remember` umhängen, Prüfung auf `created_from_flow_id` ergänzen.
5. `CONVENTIONS.md:478` und 4.22 auf den neuen Stand, `sprint-4.md:1629` auf die neue Signatur.

### Tests
- `daemon/bin/humanitl/tests/cli.rs`, gegen `FakeServer`: `decide_releases_every_named_id`, `decide_remember_creates_exactly_one_rule`, `decide_remember_carries_the_origin_flow`, `decide_rejected_rule_decides_nothing`, `decide_all_refused_leaves_no_rule`, `decide_repeated_id_is_cli_004`.
- `daemon/bin/humanitl/src/cmd/flows.rs` Testmodul (ab `:642`): JSON-Form einer Antwort mit drei Ergebnissen und `created_rule`, Abbildung Verdikt ⇒ Aktion, Default `session`.
- `tests/e2e/m2_first_decision/run.sh` als Integrationsbeleg.

### Akzeptanzkriterien
- [ ] `humanitl flows decide <id1> allow --also <id2> --also <id3>` endet mit Exit 0, druckt drei Zeilen `allow <short_id>`, und `humanitl --json flows list --filter state:held` nennt danach keine der drei Ids.
- [ ] Derselbe Aufruf mit `--remember --remember-host '**.npmjs.org'` liefert in `--json` genau ein `created_rule`; `humanitl --json rules list` zeigt danach genau eine Session-Regel mit `.action == "allow"`, `.host == "**.npmjs.org"`, `.expires.kind == "session"` und `.created_from_flow_id == <id1>` (heute für jede über die CLI angelegte Regel leer).
- [ ] `--remember --remember-host 'nicht:gueltig'` endet mit Exit 1 und dem `Diagnostic` des Dienstes; danach hat `rules list` null Session-Regeln und alle genannten Flows stehen weiter auf `state:held`.
- [ ] Ein Aufruf, dessen Ids alle nicht mehr warten, endet mit Exit 1, und `rules list` zeigt danach keine neue Regel (Rücknahme im Dienst).
- [ ] Zweimal dieselbe Id endet mit `CLI_004` und Exit 1, ohne dass ein Flow entschieden wurde.
- [ ] `humanitl flows decide ID allow` und `humanitl flows decide ID block --note TEXT` verhalten sich unverändert: `tests/e2e/lib.sh:424-431` und die Escape-Schritte aus Sprint 1 bleiben unangetastet, `make check` ist grün.
- [ ] `tests/e2e/m2_first_decision/run.sh` gibt die zwölf npm-Anfragen in genau einem `humanitl flows decide` frei (`grep -c 'flows decide' run.sh` zählt einen Aufruf in Schritt 2), endet mit Exit 0 und prüft, dass die Session-Regel `created_from_flow_id` der ersten freigegebenen Anfrage trägt.
- [ ] `grep -rn 'kennt weder mehrere Ids noch' backlog/ tests/` findet nichts mehr; `backlog/CONVENTIONS.md:478` und `backlog/sprint-4.md:1629` zeigen die neue Signatur, 4.22 hält fest, dass eine Prüfung auf Feld-Ebene weiterhin fehlt.

### Stand (2026-09-04): Überschneidung mit HUM-095

HUM-095 (`backlog/sprint-2.md`, Sprint 2) baut dieselbe Fähigkeit in unvereinbarer Form: `--remember <PATTERN>` statt `--remember` plus `--remember-host PATTERN`; ohne das Flag bleibt dort die Ein-Objekt-Form von `--json` (`{flow_id, decision, note, applied}`, `daemon/bin/humanitl/src/cmd/flows.rs:371-377`) bestehen, hier entfällt sie zugunsten von `results[]` und `created_rule`. Beide setzen `created_from_flow_id`, beide bilden `RememberArgs` auf `RuleArgs` ab, beide heben `rule_from_args` auf `pub(crate)`, beide bauen Tests in `daemon/bin/humanitl/tests/cli.rs`, beide schreiben `tests/e2e/m2_first_decision/run.sh` Schritt 2 um, beide ändern `backlog/CONVENTIONS.md` 4.22 und die Zeile `:1629` in diesem Sprint-File. Wer zuerst läuft, zwingt den anderen zum Umbau einer gerade veröffentlichten Kommandozeilen-Flagge. **Der Projekteigentümer entscheidet Flag-Form und JSON-Form, bevor eines von beiden gebaut wird.** Dieser Abschnitt entscheidet nichts; derselbe Absatz steht bei HUM-095. Die Kopfzeile `Blockiert: HUM-078` ist in beiden Issues falsch: `parity-check` vergleicht Subkommandos, nicht Flags (`docs/adr/0018-rpc-parity.md:41-43`), und `:1629` führt `Humanitl.Decide` schon als abgedeckt. Zeilenanker dieser Spezifikation sind seit dem Schreiben verschoben (`cli.rs:258-268`, `rules.rs:796`, `CONVENTIONS.md:504` und `:1368-1400`, `lib.sh:444-451`, `run.sh:418-472`); nur `flows.rs:336-339` stimmt.

### Fallstricke
- clap duldet eine variadische Position nur zuletzt: `decide <ID>... <VERDICT>` ist nicht baubar, und ein Tausch der Reihenfolge bräche jeden vorhandenen Aufrufer (`tests/e2e/lib.sh:424-431`, die Escape-Schritte, `CONVENTIONS.md:478`). Deshalb `--also`, nicht mehr Positionen.
- `RuleArgs` trägt selbst `--note` (`cli.rs:399-401`) und der Block-Zweig ebenfalls (`cli.rs:242-244`). Ein flaches `#[command(flatten)]` kollidiert; nur der präfigierte Satz `--remember-*` geht. Jedes `--remember-*` bekommt `requires = "remember"`, sonst wirkt eine Angabe ohne `--remember` stillschweigend nicht.
- Exit 1 heißt hier nicht „nichts ist passiert": Bei gemischtem Ausgang bleiben die entschiedenen Flows entschieden und die Regel steht. Die Ausgabe muss beides nennen, sonst räumt jemand hinterher eine Regel weg, die er für nicht angelegt hält.
- `created_from_flow_id` muss eine UUID sein, sonst lehnt `humanitl_ipc::convert` die Regel ab (`daemon/crates/ipc/src/convert.rs:1266-1272`). Die Id kommt unverändert von der Kommandozeile; ein Tippfehler wird zum Fehler beim Anlegen, nicht zu einer Regel ohne Herkunft.
- Der M2-Lauf hängt an `M2_RULE_ID` aus der Ausgabe von `rules add` (`run.sh:346-347`). Die Id kommt künftig aus `.created_rule.rule_id` der Entscheidung; die Prüfungen `:349-360` bleiben, sie zeigen nur auf eine andere Quelle.
- Die Haltefrist des Laufs steht auf 10 Sekunden, weil zwölf Prozesse nacheinander liefen (`CONVENTIONS.md` 4.22). Mit einem Aufruf entfällt der Grund; die Frist bleibt trotzdem, bis ein Lauf auf CI-Hardware das Gegenteil zeigt.

### Referenzen
BACKLOG.md Prinzip 10, ADR-018; `docs/ARCHITECTURE.md` 3b; `docs/adr/0018-rpc-parity.md` (Mitlieferpflicht); `backlog/CONVENTIONS.md` 4.12 (CLI), 4.22; HUM-027 (Regel vor Entscheidung), HUM-036 (M2), HUM-064, HUM-078; clap, variadische Positionen (https://docs.rs/clap/latest/clap/struct.Arg.html#method.num_args).

---

## HUM-092 · Export ist Fachlogik in der Anwendung
Sprint: 4 · Größe: L · Abhängigkeiten: HUM-026, HUM-032, HUM-065 · Blockiert: HUM-078

### Kontext
`README.md` sagt in Zeile 123 bis 124 über beide Clients: „Every capability is an RPC first; neither client contains domain logic." Dieser Satz ist heute unwahr. Die vier Export-Formate existieren ausschließlich in der Flutter-Anwendung: `encodeHar` (`app/lib/features/history/export/har.dart:49`), `encodeJsonl` (`jsonl.dart:15`), `encodeCsv` (`csv.dart:39`), `encodeCurl` (`curl.dart:20`), zusammengeschaltet in `history_export.dart:104-110`. Der Service in `proto/humanitl/v1/humanitl.proto:23-44` führt fünfzehn RPCs, keinen davon für den Export von Flows; `daemon/bin/humanitl/src/cli.rs:187-263` kennt `flows list|show|decide`, kein `export`. Wer die Historie ohne die Oberfläche exportieren will, kann es nicht — und die Aussage des README beschreibt eine Architektur, die an dieser Stelle nicht gebaut ist.

Das ist kein Versäumnis der Umsetzung, sondern ein Widerspruch zwischen zwei Dokumenten, die beide im Repository stehen. ADR-0018 zählt in Zeile 25 bis 28 die Fähigkeiten auf, die zuerst RPC sind, und nennt „Export" ausdrücklich; Zeile 110 wiederholt, dass Fachlogik in `app/` ein Architekturverstoß ist, ebenso `docs/ARCHITECTURE.md:66`. Die Sprint-Spezifikation ordnete das Gegenteil an: `backlog/sprint-2.md:583` schreibt „HUM-032 baut HAR in der UI aus `GetFlow`/`GetBody`", `sprint-2.md:1342` nennt `app/lib/features/history/export/{har,jsonl,curl}.dart` als neue Dateien. HUM-032 hat also getan, was dort stand. Solange diese beiden Zeilen stehen bleiben, kommt die Abweichung beim nächsten Export-Format zurück; dieses Issue korrigiert sie deshalb mit.

Das Muster für die richtige Seite existiert bereits: `AuditRequest.Export` (`humanitl.proto:863`) exportiert `jsonl` und `csv` daemon-seitig, und `backlog/sprint-4.md:789` hält für den Audit-Screen fest: „der Daemon schreibt die Datei (`ExportOp.out_path`), das UI zeigt Inline-Bestätigung". Der Export fehlt also nicht generell, die Historie weicht von einem vorhandenen Muster ab. CSV kam dabei ohne Spezifikation dazu (`backlog/CONVENTIONS.md:865`: „**CSV ist ein vierter Export.** Die Spezifikation nennt HAR, JSONL und curl.") — ein zusätzlicher Beleg dafür, dass Formate dort wachsen, wo niemand sie über den Vertrag sieht.

Maschinell ist die Klasse unsichtbar. Der CI-Job `parity-check` (`.github/workflows/ci.yml:572-582`) läuft auf `scripts/ci/parity-placeholder.sh` und überspringt bis HUM-078; auch danach prüft er nach ADR-0018 Zeile 39 bis 42 nur „RPC ohne CLI-Zeile", nie „Fähigkeit nur im Client". Ein Export ohne RPC fällt keiner Prüfung auf.

Schweregrad **major**, nicht blocking: Die Produktzusage aus `README.md:83` („exportable as HAR, JSONL and CSV") ist erfüllt, Nutzer bekommen ihre Dateien, und die Sicherheitsaussage bleibt unberührt. Unwahr ist allein die Architektur-Aussage — und ein dokumentierter Satz, der nicht gilt, ist in diesem Repository ein Fehler und keine Geschmacksfrage.

Umfang der Verschiebung: rund 1100 Zeilen Dart ohne Tests (`export/` 755, `providers/history_export.dart` 335, `history_export_menu.dart` 284, dazu die Formatierer aus `history_view.dart`, die alle drei Encoder importieren) plus 964 Zeilen Tests; dagegen rund 1000 Zeilen neuer Rust.

### Ziel
Der Daemon kann Flows exportieren. Ein RPC `ExportFlows` nimmt Format und Auswahl entgegen, holt Bodies aus dem Recorder, kodiert HAR 1.2, JSON Lines, CSV oder `curl` und liefert die Bytes zurück oder schreibt die Datei selbst. `humanitl flows export` und der Export-Dialog der Oberfläche sind zwei dünne Aufrufer desselben RPC und erzeugen für dieselbe Auswahl byte-identische Dateien. `app/lib/features/history/export/` existiert nicht mehr; die Anwendung wählt Format und Umfang, zeigt Fortschritt und legt ab, was sie bekommt.

### Nicht-Ziel
- Kein fünftes Format, keine Änderung an den vier bestehenden Abbildungen. Was `CONVENTIONS.md` 4.18 über HAR festhält (`timings.wait` ist 0, `content.text` fehlt ohne Bytes, `content.comment` sagt das), gilt unverändert weiter; es sind Entscheidungen über das Format, nicht über seinen Ort.
- Keine Host-Redaktion und keine andere Entschärfung des Inhalts. Sie kommt nach dem MVP (`docs/SECURITY.md`, `BACKLOG.md` Zeile 313).
- Der Audit-Export (HUM-050, HUM-051, HUM-070) wird nicht angefasst. `AuditRequest.Export` bleibt, wie er ist; die beiden Exporte werden nicht zusammengelegt.
- Der allgemeine Paritäts-Check „Fähigkeit nur im Client" bleibt HUM-078. Dieses Issue liefert nur die eine enge Sicherung gegen den Rückfall und den Satz, den HUM-078 aufgreift.
- Mehrfachauswahl bleibt draußen: Der Umfang „Auswahl" ist weiterhin eine Zeile (`CONVENTIONS.md` 4.18), bis HUM-029 im History-Screen eine Menge liefert.

### Betroffene Pfade
Proto:
- `proto/humanitl/v1/humanitl.proto` (ändern: `rpc ExportFlows` ans Ende des Service, `ExportFormat`, `ExportFlowsRequest`, `ExportFlowsChunk`)
- `proto/descriptor.binpb`, `proto/generated.sha256` (ändern, `make proto`)

Daemon:
- `daemon/crates/recorder/src/export/{mod,har,jsonl,csv,curl}.rs` (neu): die vier Encoder, portiert aus den vier Dart-Dateien
- `daemon/crates/recorder/src/export/entry.rs` (neu): der Satz Daten, den ein Encoder braucht (Gegenstück zu `export_entry.dart`), gefüllt aus `query.rs` und `blob.rs`
- `daemon/crates/recorder/tests/export.rs` (neu), `daemon/crates/recorder/tests/fixtures/export/` (neu)
- `daemon/crates/ipc/src/server.rs`, `src/server_stub.rs` (Trait-Methode neben `audit`), `src/convert.rs`, `src/validate.rs`, `src/fake/mod.rs`
- `daemon/crates/ipc/tests/proto_contract.rs` (Namenstabelle, heute Zeile 416 mit `AuditRequest.Export`)
- `daemon/bin/humanitl/src/cli.rs` (`FlowsCmd::Export`), `src/cmd/flows.rs`, `src/render.rs`
- `daemon/crates/config/src/schema.rs`: `limits.export_max_flows`

App:
- löschen: `app/lib/features/history/export/{har,jsonl,csv,curl,export_entry,history_export}.dart`
- umbauen: `app/lib/features/history/providers/history_export.dart`, `app/lib/features/history/history_export_menu.dart`, `app/lib/features/history/history_view.dart` (die Formatierer, die nur die Encoder brauchten, wandern nach Rust)
- `app/lib/core/ipc/daemon_client.dart`, `grpc_daemon_client.dart`, `fake_daemon_client.dart`, `convert.dart`
- `app/test/features/history/history_export_test.dart`, `history_export_flow_test.dart`
- `app/test/goldens/goldens/ci/history_export_light.png`, `history_export_dark.png`
- `app/l10n/app_en.arb`, `app_de.arb`

Sicherung und Dokumente:
- `scripts/ci/check-client-logic.sh` (neu), `Makefile`, `.github/workflows/ci.yml` (Job `parity-check`)
- `backlog/sprint-2.md` (Zeile 583 und 1329 bis 1345), `backlog/CONVENTIONS.md` (4.18, neuer Abschnitt 4.23), `backlog/sprint-4.md` (Notiz an HUM-078)
- `docs/PROTOCOL.md` (Zeile 11, RPC-Liste), `BACKLOG.md` (Zeile 460)

### Spezifikation

**Proto.** Additiv, neuer RPC ans Ende des Service, Kommentare ohne Umlaute wie im Rest der Datei (`docs/PROTOCOL.md` 4).

```proto
  // Exportiert aufgezeichnete Flows in ein Austauschformat (HUM-092). Die
  // Bytes entstehen im Daemon; der Client waehlt Format und Umfang.
  rpc ExportFlows(ExportFlowsRequest) returns (stream ExportFlowsChunk);
```

```proto
enum ExportFormat {
  EXPORT_FORMAT_UNSPECIFIED = 0;
  EXPORT_FORMAT_HAR = 1;
  EXPORT_FORMAT_JSONL = 2;
  EXPORT_FORMAT_CSV = 3;
  EXPORT_FORMAT_CURL = 4;
}

message ExportFlowsRequest {
  ExportFormat format = 1;

  // Genau eine Auswahl. `query` benutzt dieselbe Filtergrammatik wie
  // `ListFlows`; `limit` und `cursor` darin werden ignoriert, die Obergrenze
  // ist `max_flows`.
  oneof selection {
    FlowIds flows = 2;
    ListFlowsRequest query = 3;
  }

  // 0 bedeutet die Vorgabe des Dienstes (`limits.export_max_flows`).
  uint32 max_flows = 4;

  // Leer: der Dienst streamt die Bytes. Sonst schreibt er die Datei selbst
  // und nennt die Pfade in `Done`.
  string out_path = 5;

  // Was in den `creator`-Block der HAR kommt, zum Beispiel "humanitl-app 0.1.0".
  string creator = 6;

  message FlowIds {
    repeated string flow_ids = 1;
  }
}

message ExportFlowsChunk {
  oneof part {
    Progress progress = 1;
    FileStart file = 2;
    bytes data = 3;
    Done done = 4;
    Diagnostic diagnostic = 5;
  }

  // Wie viele Flows gesammelt sind. `total` ist die gekappte Trefferzahl.
  message Progress {
    uint32 done = 1;
    uint32 total = 2;
  }

  // Beginnt eine Datei. Der `curl`-Export sendet zwei davon.
  message FileStart {
    string name = 1;
    string mime_type = 2;
  }

  message Done {
    uint32 flow_count = 1;
    uint64 byte_count = 2;
    repeated string out_paths = 3;
    // Der Filter traf mehr als `max_flows`.
    bool capped = 4;
  }
}
```

Ablauf des Stroms: beliebig viele `progress`, dann je Datei ein `file` gefolgt von `data`-Stücken (höchstens 256 KiB je Stück), am Ende genau ein `done`. Mit gesetztem `out_path` entfallen die `data`-Stücke; `file` und `done` kommen trotzdem, damit der Client die Namen nennen kann, bevor etwas geschrieben ist. Ein Fehler beendet den Strom mit einem `diagnostic` als letztem Teil, nie mit einem halben `done`.

**Reihenfolge und Determinismus.** `query` wird serverseitig ausgewertet, in der Reihenfolge, die `ListFlows` mit demselben `filter`, `order_by` und `include_passthrough` liefert; `flows` exportiert in der Reihenfolge der Liste. Zweimal derselbe Aufruf ergibt dieselben Bytes.

**Kappe.** `limits.export_max_flows`, Default 5000, Tier `advanced` (`CONVENTIONS.md` 4.4 ist die Heimat aller Caps). Die Dart-Konstante `historyExportMaxFlows` entfällt ersatzlos; sie war nie ein Config-Schlüssel, also braucht sie keinen Alias. Trifft der Filter mehr, exportiert der Dienst die ersten `max_flows` der Sortierung und setzt `Done.capped`.

**Formate.** Byte-gleich zum heutigen Dart-Stand, sonst ist der Umbau nicht prüfbar:
- HAR 1.2 mit `_humanitl` je Eintrag, `timings.wait` = 0, `content.text` fehlt ohne aufgezeichnete Bytes, `content.comment` sagt warum, Binärinhalt base64 mit `encoding: "base64"`.
- JSON Lines, ein Objekt je Zeile, Bodies in `body_b64` mit `truncated` daneben, abschließender Zeilenumbruch. Der Round-Trip-Partner `decodeJsonl` wandert als `parse` in dieselbe Rust-Datei, damit der Round-Trip-Test bleibt.
- CSV nach RFC 4180: die 21 Spalten aus `csvColumns` in derselben Reihenfolge, CRLF, Feld mit Komma, Anführungszeichen oder Umbruch wird gequotet, inneres Anführungszeichen verdoppelt, unfertiger Wert leer statt null. Keine Bodies.
- `curl`: genau ein Flow, Kopfzeilenreihenfolge wie aufgezeichnet, `--data-binary @request.body`, zweite Datei `request.body` daneben. Eine vorhandene Datei wird nummeriert (`request.body.1`), nie überschrieben.

**Fehler.** Jeder Pfad liefert ein `Diagnostic` mit `why` und, wo möglich, `fix`; freie Codes im Bereich `recorder` (`CONVENTIONS.md` 4.6):
- `RECORDER_005` Format unbekannt oder Auswahl leer.
- `RECORDER_006` `out_path` nicht schreibbar (Verzeichnis fehlt, kein Recht, Symlink); `fix` nennt den geprüften Pfad.
- `RECORDER_007` `curl` mit einer Auswahl ungleich einem Flow; `why` nennt die Anzahl.
`--out -` zusammen mit `--format curl` ist ein `CLI_004` („curl schreibt zwei Dateien"), weil zwei Dateien nicht in einen Stdout passen.

**CLI.** Ein Subkommando von `flows`, mit dem Pflicht-Attribut `#[humanitl(rpc = "ExportFlows")]` (ADR-0018):

```
humanitl flows export [FILTER...] --format har|jsonl|csv|curl --out PATH
                      [--flow ID]... [--limit N] [--sort KEY] [--asc]
```

`--out -` schreibt nach Stdout (nicht für `curl`), sonst wird `out_path` gesetzt und der Daemon schreibt. `--flow` und `FILTER` schließen sich aus. Die Ausgabe nennt danach Anzahl, Bytes und Pfade in einer Zeile, mit `--json` als Objekt.

**Anwendung.** `history_export_menu.dart` behält Formatwahl, Umfangswahl, den Satz über den Inhalt der Datei (`historyExportContents`) und die Bestätigung. `providers/history_export.dart` schrumpft auf: Aufruf, Fortschritt aus `Progress`, Ablage. `isolateHistoryExportEncoder`, `historyExportEncoderProvider` und `dart:isolate` entfallen — es wird in der Anwendung nichts mehr kodiert. Die Naht für Tests ist ab jetzt der `DaemonClient`, nicht eine Encoder-Funktion.

**Wohin die Datei geht.** Zwei Wege, die Entscheidung fällt in Schritt 1 und steht danach in `CONVENTIONS.md` 4.23:
- **A, Strom (Vorgabe):** `out_path` bleibt leer, der Dienst streamt, die Oberfläche reicht die Bytes an `file_picker` 12 weiter, das sie selbst schreibt und die `Uri` zurückgibt (`CONVENTIONS.md` 4.18, Zeilen 857 bis 864). Bytes ablegen, die ein anderer errechnet hat, ist keine Fachlogik.
- **B, `out_path`:** nur wenn gemessen ist, dass der Speichern-Dialog unter Linux einen Pfad liefert, den der Daemon-Prozess selbst öffnen darf. Dann wie beim Audit-Screen (`sprint-4.md:789`).
Die CLI benutzt in beiden Fällen `out_path`.

**ARB.** Die Schlüssel ab `historyExport` (`app_en.arb:1090-1133`, mit den späteren ab 1174) bleiben, soweit ihr Text stimmt: `historyExportCollecting` bekommt seine beiden Zahlen ab jetzt aus `Progress`. Texte, die behaupten, die Anwendung schreibe („Writing {count} requests"), werden auf das umgestellt, was tatsächlich passiert; jede Änderung in beiden Dateien. Kein Schlüssel verschwindet, ohne dass seine Verwendung verschwindet.

### Schritte
1. Weg A oder B entscheiden: `FilePicker.platform.saveFile` auf dem Zielsystem einmal aufrufen und protokollieren, was zurückkommt (Pfad oder Portal-`Uri`) und ob ein zweiter Prozess dorthin schreiben darf. Ergebnis mit der Messung in `CONVENTIONS.md` 4.23. Zwischenstand: ein Absatz mit Zahl und Datum im Repository.
2. Proto ergänzen, `make proto`, Namenstabelle in `proto_contract.rs` erweitern. Zwischenstand: `cargo test -p humanitl-ipc` grün, `checked_in_descriptor_matches_the_proto_sources` grün.
3. Fixtures aus `app/test/features/history/history_export_test.dart` einmalig als Dateien erzeugen (Eingabe als JSON, erwartete Ausgabe als `.har`, `.jsonl`, `.csv`, `.sh`) und nach `daemon/crates/recorder/tests/fixtures/export/` legen. Zwischenstand: die Dateien liegen da und stammen nachweislich aus dem alten Code.
4. Die vier Encoder in `humanitl-recorder` bauen. Zwischenstand: `cargo test -p humanitl-recorder export::` grün, jede Fixture byte-identisch.
5. RPC bedienen: `DaemonApi`-Methode, `server.rs`, `validate.rs`, `convert.rs`, Fake mit drei synthetischen Flows. Zwischenstand: `grpcurl` gegen den Fake liefert einen HAR-Strom.
6. CLI `flows export` mit `render.rs`. Zwischenstand: `humanitl flows export --format har --out /tmp/e.har` schreibt eine Datei, die `jq` liest.
7. Anwendung umbauen, `export/` löschen, Client-Methode in allen drei Clients. Zwischenstand: `flutter test` grün, Goldens neu.
8. Sicherung `scripts/ci/check-client-logic.sh`, Einbindung in `make check` und `parity-check`. Zwischenstand: das Skript wird mit einer absichtlich wieder eingefügten Encoder-Datei rot und ohne sie grün.
9. Dokumente korrigieren: `sprint-2.md`, `CONVENTIONS.md` 4.18 und 4.23, `sprint-4.md` (Notiz an HUM-078), `docs/PROTOCOL.md`, `BACKLOG.md`. Zwischenstand: `make check` und `tools/verify-commit.sh` grün.

### Akzeptanzkriterien
- [ ] `grep -rn "encodeHar\|encodeJsonl\|encodeCsv\|encodeCurl" app/lib` findet nichts, und `app/lib/features/history/export/` existiert nicht mehr (`test ! -d`).
- [ ] `proto/humanitl/v1/humanitl.proto` enthält `rpc ExportFlows`; `cargo test -p humanitl-ipc` ist grün, `proto/descriptor.binpb` und `proto/generated.sha256` liegen im selben Commit.
- [ ] Byte-Gleichheit der Formate: `cargo test -p humanitl-recorder export::` prüft alle Fixtures aus Schritt 3; für jede ist die Ausgabe des Rust-Encoders byte-identisch mit der Datei, die die Dart-Implementierung erzeugt hat (Vergleich über `assert_eq!` auf `&[u8]`, nicht auf Text).
- [ ] `humanitl flows export host:example.com --format har --out /tmp/e.har` endet mit 0, und `jq -e '.log.version == "1.2" and (.log.entries | length) == 3' /tmp/e.har` ist wahr (gegen den Fake-Daemon).
- [ ] Oberfläche und Kommandozeile liefern dasselbe: `tests/e2e/m2_first_decision/run.sh` exportiert die gefilterte Menge einmal über die Oberfläche und einmal über `humanitl flows export`; `cmp ui.har cli.har` endet mit 0.
- [ ] `curl`-Export schreibt zwei Dateien, ein zweiter Lauf in dasselbe Verzeichnis schreibt `request.body.1`, und `request.body` ist danach byte-identisch zum ersten Lauf (`export::curl::existing_body_file_is_numbered`).
- [ ] Kappe: bei 5001 passenden Flows im Recorder liefert der Strom `Done.flow_count == 5000` und `Done.capped == true`, und die Oberfläche zeigt `historyExportCap` (Widget-Test mit gesetztem Fake-Wert).
- [ ] Fehlerpfade: `--format curl` mit zwei `--flow` endet mit Exit ungleich 0 und `RECORDER_007` in der Ausgabe; `--out /nonexistent/dir/e.har` endet mit `RECORDER_006`, dessen `why` und `fix` gefüllt sind; kein Pfad liefert einen nackten String (Register-Test aus `CONVENTIONS.md` 4.6).
- [ ] `humanitl flows export --help` existiert, und `grep -n 'humanitl(rpc = "ExportFlows")' daemon/bin/humanitl/src/cli.rs` findet das Attribut.
- [ ] `scripts/ci/check-client-logic.sh` läuft in `make check` und im Job `parity-check`; mit einer testweise unter `app/lib/features/` angelegten Datei, die `encodeHar` definiert, endet es mit 1 und nennt Datei und Zeile, ohne sie mit 0.
- [ ] `backlog/sprint-2.md` enthält weder in Zeile 583 noch im Abschnitt HUM-032 die Anweisung, den Export in der Oberfläche zu bauen; beide Stellen verweisen auf HUM-092. `grep -n "in der UI" backlog/sprint-2.md` findet keine Export-Zeile mehr.
- [ ] `backlog/CONVENTIONS.md` 4.18 sagt bei „Der Export schreibt die Datei selbst" und „CSV ist ein vierter Export", dass die Kodierung ab HUM-092 im Daemon liegt; 4.23 nennt die Messung aus Schritt 1 mit Datum.
- [ ] `docs/PROTOCOL.md` Zeile 11 führt den Export in der Liste des Service, und die HUM-032-Zeile in `BACKLOG.md` (Zeile 460) nennt `ExportFlows` als Quelle des Exports.
- [ ] `flutter gen-l10n` läuft ohne Warnung, `app_en.arb` und `app_de.arb` haben denselben Schlüsselsatz (bestehender Test), und `flutter test` ist grün, Goldens `history_export_light.png` und `history_export_dark.png` neu erzeugt.
- [ ] `make check` und `tools/verify-commit.sh` sind auf dem Commit grün, nicht nur im Arbeitsbaum.

### Fallstricke
- Die Kommentare in `humanitl.proto` schreiben Umlaute als `ae`, `oe`, `ue`. Wer das bricht, sieht es erst im Descriptor-Diff.
- Der Export trägt Bodies. `ExportFlowsChunk` darf von `FlowEvent` aus nicht erreichbar sein, sonst schlägt `edited_request_and_body_preview_stay_out_of_the_event_stream` an; das neue `bytes`-Feld gehört mit Begründung in die Erlaubnisliste von `proto_contract.rs` (`docs/PROTOCOL.md` 4.5).
- `out_path` ist ein Pfad, den der Daemon unter seinem eigenen Benutzer öffnet. Prüfen vor dem Schreiben: kein Symlink (`O_NOFOLLOW`, wie in HUM-043), kein Pfad in einen Sandbox-Mount, keine stille Auflösung von `~`. Ein Export ist kein Grund, dem Client einen Schreibzugriff zu leihen.
- Der Inhalt bleibt gefährlich: Hosts, vollständige Pfade mit Query, alle Kopfzeilen und beide Rümpfe im Klartext. Der Satz davor (`historyExportContents`, `CONVENTIONS.md` 4.18, `docs/SECURITY.md`) wandert mit und verschwindet nicht dadurch, dass jetzt der Daemon schreibt.
- `timings.wait` bleibt 0. Die `FlowSummary` trägt keine Haltezeit, und der Encoder neben dem Recorder zu haben ist kein Anlass, eine Aufteilung zu raten (`CONVENTIONS.md` 4.13, 4.18). Wer `held_ms` will, nimmt das Feld in die Proto auf, in einem eigenen Issue.
- Die 964 Zeilen Dart-Tests werden nicht gelöscht, sondern geteilt: was das Format prüft, wird zum Rust-Test, was den Dialog prüft (`history_export_flow_test.dart`), bleibt und bekommt den Fake-Client als Naht statt der Encoder-Funktion.
- Keine neue Crate ohne Not: die Encoder liegen in `humanitl-recorder`, das die Bodies ohnehin hält und von `humanitl-ipc` schon abhängt. Ein Subagent ändert `daemon/Cargo.toml` nicht.
- `history_view.dart` liefert Formatierer sowohl an die Tabelle als auch an die Encoder. Nur die zweite Gruppe wandert; wer die Datei leert, nimmt der Tabelle ihre Spaltenformate.
- Der Fake-Daemon muss den RPC mitliefern, sonst ist die Anwendung im Fake-Modus ohne Export und die Widget-Tests haben keinen Gegenstand.
- `make proto` nicht vergessen: ein fehlender Descriptor fällt sofort auf, ein fehlender Dart-Hash erst in CI („Fail on generated drift", `docs/PROTOCOL.md` 4.8).

### Quellen
`docs/ARCHITECTURE.md` 3b (Zeile 66); `docs/adr/0018-rpc-parity.md` Zeilen 25 bis 28, 39 bis 42, 110; `README.md` Zeile 83 und Zeilen 123 bis 124; `backlog/CONVENTIONS.md` 4.4, 4.6, 4.13, 4.18 (Zeilen 802 bis 880); `backlog/sprint-2.md` HUM-026 Nicht-Ziel (Zeile 583) und HUM-032 (Zeilen 1329 bis 1345); `backlog/sprint-4.md` HUM-051 (Zeile 789, Audit-Export als Vorbild) und HUM-078; `docs/PROTOCOL.md` Abschnitte 3 und 4; `proto/humanitl/v1/humanitl.proto` Zeilen 23 bis 44 und 863; `daemon/bin/humanitl/src/cli.rs` Zeilen 102 bis 142 und 187 bis 263; `.github/workflows/ci.yml` Zeilen 572 bis 582; HAR 1.2 (http://www.softwareishard.com/blog/har-12-spec/); RFC 4180 (https://www.rfc-editor.org/rfc/rfc4180).
