# Sprint 4 · Trusted Editor (M4)

Ziel des Sprints: Der Nutzer kann eine gehaltene Anfrage pseudonymisieren, bevor sie rausgeht. Alles, was passiert, landet in einer prüfbaren Audit-Kette. Die App spricht Deutsch und Englisch, ist über einen generierten Settings-Screen vollständig konfigurierbar und lässt sich als `.deb` und AppImage installieren. Demo-Skript M4 (HUM-055) ist am Sprintende in CI grün.

Voraussetzungen aus früheren Sprints: `humanitl-core` mit `Finding`, `Diagnostic`, `FlowState` (HUM-004, HUM-063), `humanitl-findings` Tier 1 (HUM-025), `humanitl-recorder` mit Schema aus BACKLOG.md 3.4 (HUM-026), `humanitl-config` mit Schema und Tiers (HUM-062), CLI-Grundgerüst (HUM-064), Intercept-Screen mit Aktionsleiste (HUM-020, HUM-028), Body-Ansichten (HUM-030).

> **Umfangsentscheidung 2026-09-18: rudimentäre Pseudonymisierung im MVP.** Der Nutzer hat entschieden, das MVP zu beschleunigen und für die Pseudonymisierung mit einer einfachen Fassung auszukommen. Der Editor aus HUM-047 ist gemergt und bleibt: Funde ersetzen, Auswahl mit `Ctrl+R` pseudonymisieren, Kopfzeilen, Methode und Pfad bearbeiten, Prüfung im Daemon. Die Zuordnung Pseudonym zu Original lebt im Speicher und gilt je Sitzung.
>
> - **Nach dem MVP verschoben** (BACKLOG.md Abschnitt 9, Punkt 15 und 4): **HUM-048** (dauerhafte, verschlüsselte Zuordnungstabelle, Schlüssel im System-Keyring, Mapping-Panel, Export), **HUM-079** (Rücktausch der Pseudonyme in Antworten) und aus HUM-047 das Popover am Diff-Glow mit den Aktionen je Fund. Ihre Spezifikationen bleiben unten stehen, damit die Arbeit später nicht neu gedacht werden muss; sie zählen nicht mehr zu Sprint 4.
> - **Verkleinert:** **HUM-049** baut nur die Inline-Pause beim Senden mit offenen Funden („Trotzdem senden", „Pseudonymisieren", „Blockieren") und die harte Sperre `HOLD_004` für bestätigte Geheimnisse. „Ignorieren" und „Immer ignorieren" je Fund samt Allowlist entfallen im MVP. Die Abhängigkeit von HUM-048 entfällt.
> - **Angepasst:** **HUM-055** prüft den verkleinerten Umfang: Pseudonymisieren, Senden, der Upstream sieht nur Pseudonyme, History zeigt „Editiert". Die Prüfungen „Mapping enthält drei Einträge" und „ein Export enthält keine Originale" entfallen mit HUM-048.
> - **Was das kostet:** Ohne HUM-079 liest der Agent in Antworten die Platzhalter (`<EMAIL_1>`) statt der Originale. Ohne HUM-048 ist die Zuordnung nach einem Neustart der App verloren; man kann später nicht mehr nachschlagen, wofür ein Platzhalter stand.
> - **Was es spart** (Schätzung, nicht gemessen): zwei M-Issues mit ihren Review-Runden und etwa die Hälfte von HUM-049, zusammen ungefähr ein bis zwei Arbeitstage bis zum MVP.
>
> **Zweite Umfangsentscheidung 2026-09-18: ein einfaches Einstellungsformular statt des generischen Settings-Screens.** HUM-069 und HUM-140 werden zusammen als ein handgebautes Formular gebaut, das die wenigen Einstellungen zeigt, die ein Mensch im MVP wirklich braucht: `hold.timeout_secs`, `hold.ask_mode`, `ui.language`, `ui.theme`, `ui.notifications`, `sandbox.work_dir`, den Modell-Endpunkt, das Modell und den Zugangsschlüssel. „Prüfen" für den Endpunkt bleibt, und der Schlüssel liegt weiter im Schlüsselbund des Systems (Secret Service) mit einer `0600`-Datei als Rückfall — das ist eine Sicherheitsaussage und wird nicht gekürzt. Geschrieben wird über einen Daemon-RPC, der den Konfig-Schreiber aus HUM-070 (`humanitl_config::edit`) benutzt; eine Änderung wirkt nach einem Neustart des Daemons, und das Formular sagt das. Alles andere bleibt in `config.toml`, erreichbar über „Datei öffnen".
>
> - **Nach dem MVP verschoben** (BACKLOG.md Abschnitt 9, Punkt 16): das Formular aus dem JSON-Schema für alle Felder, Suche, Detailstufen, Herkunftsanzeige und Zurücksetzen je Feld, Live-Neuladen mit `ConfigChanged`, die Dateiüberwachung von `config.toml`, der Audit-Eintrag je Änderung und die Anzeige der Ladebefunde im Formular.
> - **Was das kostet:** Eine Änderung wirkt erst nach einem Neustart des Daemons, und Einstellungen außerhalb der Liste ändert man in der Datei.
> - **Was es spart** (Schätzung, nicht gemessen): der Daemon-Umbau für Live-Neuladen und der Formular-Generator, ungefähr zwei bis drei Arbeitstage.


| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-047 | Pseudonymisierungs-Editor | L | HUM-025, HUM-028, HUM-030 |
| HUM-049 | Senden mit offenen Findings (verkleinert, siehe Umfangsentscheidung) | S | HUM-047, HUM-062 |
| HUM-050 | Audit-Hash-Kette | M | HUM-004, HUM-026, HUM-048 |
| HUM-051 | Audit-Screen | S | HUM-050 |
| HUM-052 | i18n Deutsch und Englisch | M | HUM-019 |
| HUM-069 | Einstellungsformular (verkleinert, siehe Umfangsentscheidung; mit HUM-140) | M | HUM-062, HUM-052 |
| HUM-070 | CLI config, audit, daemon | S | HUM-064, HUM-050, HUM-062 |
| HUM-077 | Ein-Klick-Installation | M | HUM-053, HUM-075 |
| HUM-078 | Paritäts-Tabelle und CI-Check | S | HUM-070, HUM-059 |
| HUM-053 | Packaging deb, AppImage, systemd | M | HUM-070 |
| HUM-054 | Golden- und Widget-Tests | M | HUM-047, HUM-052, HUM-069 |
| HUM-055 | Demo-Skript M4 | S | alle oben |
| HUM-134 | Zwei Tests werden unter Last rot | S | — |
| HUM-136 | Die Oberflaeche verschwindet, und niemand weiss warum | M | HUM-042 |
| HUM-137 | Ein Agent, den es nicht gibt, faellt lautlos aus | S | HUM-040, HUM-067 |
| HUM-138 | Geblockter Verkehr ausserhalb von HTTP ist unsichtbar | M | HUM-002, HUM-021 |
| HUM-139 | Die Vorpruefung sucht den Agenten im PATH des Hosts | S | HUM-037, HUM-135 |
| HUM-140 | Modell-Endpunkt und Zugangsschlüssel (im Formular von HUM-069) | M | HUM-062, HUM-069, HUM-039 |
| HUM-141 | Das Mock-Modell laesst keinen Werkzeugaufruf zu | M | HUM-046, HUM-067 |
| HUM-142 | Der Daemon wartet ohne Frist auf einen Agenten, der nicht gehen will | M | HUM-011, HUM-042 |
| HUM-143 | Zwischen zwei Segmenten verschwindet der Klick | S | HUM-028 |
| HUM-145 | Die Kopplung an einen Adapter waechst nicht weiter | S | HUM-074 |
| HUM-148 | Die Sandbox-Goldens lesen die Wanduhr | S | HUM-040 |
| HUM-150 | Ein langer Befund wird mitten im Wort abgeschnitten | S | HUM-106 |
| HUM-153 | Im History-Detail bleibt dem Body kaum Platz | S | HUM-032, HUM-116 |
| HUM-154 | „No body." steht in einer Farbe, die für Sätze zu schwach ist | S | HUM-030 |
| HUM-156 | Der Daemon beantwortet `Audit` nicht | M | HUM-050, HUM-070, HUM-051 |
| HUM-157 | `audit.retention_days` hat keinen Leser | M | HUM-050, HUM-051, HUM-156 |
| HUM-158 | Ein Export ohne freien Namen meldet `IPC_006` | S | HUM-051, HUM-070 |
| HUM-159 | Die harte Sperre blockt, bevor jemand pseudonymisieren kann | M | HUM-049, HUM-156 |
| HUM-160 | „Trotzdem senden" hinterlässt keine Spur | M | HUM-049, HUM-050 |
| HUM-161 | Der Editor sendet verbliebene Funde ohne Rückfrage | S | HUM-047, HUM-049 |
| HUM-162 | Der Kopf der Kette hat über den Daemon keinen Zeitpunkt | S | HUM-156 |
| HUM-163 | Jede Seite der Audit-Tabelle liest die ganze Kette | M | HUM-156 |
| HUM-164 | Ein Client weckt den Daemon über den Socket nicht | S | HUM-053 |
| HUM-165 | Der Daemon aus dem AppImage findet Katalog und Sandbox-Profil nicht | S | HUM-053, HUM-070 |
| HUM-166 | `humanitl_ipc::serve` und `systemd::serve` sind zwei Fassungen desselben Dienstes | S | HUM-053, HUM-156 |
| HUM-167 | Eine alte Nutzer-Unit verdeckt die Units des Pakets | S | HUM-053 |
| HUM-168 | Ein Rückfall für Impeller fehlt im Release-Bau | S | HUM-053 |
| HUM-169 | `flows watch`: der Ereignisstrom auf der Kommandozeile | S | HUM-078 |
| HUM-170 | `GetConfig` und `GetSessionSummary` haben keinen Ort in der Oberfläche | S | HUM-078 |
| HUM-171 | Tests lassen ihre Verzeichnisse in `/tmp` liegen | S | — |
| HUM-185 | Der Bildschirm-Test gegen den echten Daemon fällt in CI zufällig aus | S | HUM-144 |
| HUM-190 | `AGENT_004` zeigt einen PATH, den der Bildschirm zurückhält | S | HUM-139, HUM-137 |
| HUM-197 | Im schmalen History-Detail bleibt dem Body weiter kaum Platz | S | HUM-153 |
| HUM-198 | Die History des Fakes nennt eine Anfragegröße, die ihr Rumpf nicht hat | S | HUM-032 |
| HUM-194 | Das Audit-Log hat keine Obergrenze in Bytes | S | HUM-157 |
| HUM-195 | `AuditWarning` nennt im Vertrag nur zwei Arten | S | HUM-157, HUM-160 |

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

Nachgezogen auf den gelieferten Vertrag (HUM-089, 2026-09-11): Die bearbeitete Anfrage reist als `EditedRequest` im `oneof decision` auf Nummer 7 (bestehend, `proto/humanitl/v1/humanitl.proto`), samt Body als `bytes`; ein eigenes `HttpRequestProto` braucht es nicht mehr. Die Nummern 3 und 6 sind gesperrt, 4 ist `block`, deshalb bekommt `replacements` die freie Nummer 10. Die übrigen Nennungen von `HttpRequestProto` in diesem Issue (Betroffene Pfade, Schritt 1) sind beim Bau entsprechend zu lesen.

```proto
message ReplacementProto { FindingLocationProto location = 1; uint32 start = 2; uint32 end = 3; bytes value_hash = 4; string pseudonym = 5; }
message DecideRequest {
  repeated string flow_ids = 1;                  // bestehend
  reserved 3, 6;                                 // 3: altes `allow_edited`; 6: `acknowledge_findings` (HUM-089)
  reserved "acknowledge_findings";
  oneof decision {                               // bestehend
    google.protobuf.Empty allow = 2;
    EditedRequest allow_edited = 7;              // die bearbeitete Anfrage samt Body
    Block block = 4;
  }
  Rule remember = 5;                             // bestehend aus HUM-028
  repeated uint32 acknowledged_findings = 8;     // HUM-049
  repeated uint32 ignore_always = 9;             // HUM-049
  repeated ReplacementProto replacements = 10;   // dieses Issue; 4 ist `block`
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
- [x] `E` auf einer gehaltenen Anfrage öffnet den Editor im mittleren Pane, `Esc` kehrt zur Karte zurück, Entwurf bleibt. **Gemessen am 2026-09-13**: `app/test/features/editor/editor_screen_test.dart`, `open_with_E_shows_editor` (nach `E` steht `editor-send`, `intercept-allow` ist weg) und `esc_keeps_draft` (Rumpf von Hand auf „changed by hand" gesetzt, `Esc`, wieder `E`: derselbe Text, `dirty == true`).
- [x] „Alle ersetzen" ersetzt jeden offenen Fund, gleiche Werte bekommen dasselbe Pseudonym, Format `<TYPE_n>`. **Gemessen**: `replace_all_glows` (drei Funde ⇒ `<EMAIL_1> <EMAIL_2> <EMAIL_3>`), `replaceAllOpen_countersPerType` (2 E-Mails, 1 IBAN ⇒ `<EMAIL_1> <EMAIL_2> <IBAN_1>`), `the same value gets the same pseudonym across locations`, `replaceAllOfValue_sameHashSamePseudonym` (Kopfzeile und Rumpf ⇒ beide `<EMAIL_1>`).
- [ ] Ersetzte Stellen sind mit Diff-Glow markiert, Hover zeigt maskiertes Original und Pseudonym. **Hover-Teil nach dem MVP verschoben** (Umfangsentscheidung 2026-09-18, BACKLOG.md 9 Punkt 15); der Diff-Glow selbst ist gebaut. **Halb.** Der Diff-Glow steht und ist gemessen: `replace_all_glows` zählt drei Markierungen `HEditorDecorationKind.replaced`, und `HEditorDecorations` malt sie mit Akzent-Unterstrich (1 px) plus Fläche in 10 % Alpha. **Was fehlt, ist das Hover-Popover.** Statt seiner zeigt die `MappingStrip` unter dem Editor Pseudonym, Art und maskiertes Original (`the mapping shows the masked original, never the value`: `an************om` steht da, `anna@example.com` nirgends). Das Popover braucht eine Zeiger-Trefferfläche über einem `TextSpan` — `HPopover` nimmt heute nur zwei Zeichenketten, kein Widget —, und HUM-048 baut die Zuordnung ohnehin aus; es gehört dorthin.
- [x] `Ctrl+R` auf einer Auswahl erzeugt ein `<CUSTOM_n>` oder gelabeltes Pseudonym. **Gemessen**: `ctrl_r_pseudonymises_the_selection` (Auswahl `[0, 9)`, Label „client" ⇒ `<CLIENT_1> writes`), `replaceSelection_createsCustomFinding` (Label `PROJECT` ⇒ `<PROJECT_1>`, Fund `custom:PROJECT`, Status `replaced`), `an empty label becomes CUSTOM`.
- [x] `Host` und `Content-Length` sind im Header-Tab sichtbar gesperrt. **Neu gemessen am 2026-09-18**, nachdem der Review gezeigt hatte, dass die erste Messung leer war — damals war *jede* Zeile gesperrt, weil keine sich bearbeiten ließ. Jetzt unterscheiden sich die beiden Arten: `a locked header has no field at all` (die Zeilen `Host` und `Content-Length` haben kein `editor-header-name-*`), `an unlocked header can be typed into` (in `Content-Type` getippt, landet im Entwurf), `an added header can be named and then travels`, `host and content-length are visibly locked` (Schloss-Glyph, „set by Humanitl", kein Löschknopf).
- [x] Senden mit geändertem Host ist im UI unmöglich und wird vom Daemon mit `EDIT_001` abgelehnt (Test). **Gemessen**: Im UI ist die Authority ein `Text` mit Schloss, kein Feld (`editor-authority`). Im Daemon `daemon/crates/proxy/src/edit.rs`: `authority_change_rejected` (`evil.io` ⇒ `EDIT_001`), `port_change_rejected` (8443), `scheme_downgrade_rejected` (`https` nach `http`), `authority_case_insensitive_ok` (`API.GITHUB.COM.` geht durch).
- [x] Nach dem Senden trägt die Anfrage in Queue-Abgang, History und Detail den Chip „Edited", Zustand `allowedEdited`. **Gemessen am 2026-09-18.** Der Chip ist das `HBadge` der Findings-Chips in `state.allowedEdited`, einmal festgelegt in `core/ui/edited_badge.dart` (`EditedBadge`, ARB `flowEditedChip`), und steht an allen drei Stellen: im Abgang (`_ConfirmationStrip`, `trailing`), in der Historien-Spalte „Edited" (statt des Akzentpunkts; Spalte 28 auf 92 px, `historyEditedColumnWidth`) und im Kopf des Details neben dem Zustand. `queue_edited_chip_test.dart` fährt `allowEdited` über den Fake-Daemon und `interceptDecisionProvider` bis in den Streifen: Chip da, Wort „Edited", Fläche `state.allowedEdited`, Wort in `stateText.allowedEdited`; nach `allow` kein Chip. `history_edited_chip_test.dart`: jede sichtbare bearbeitete Zeile genau ein Chip, eine erlaubte keiner; „Edited" passt ungeschnitten in die Spalte (Testschrift); Detail mit Chip, nach Wechsel auf eine erlaubte ohne; im Detail ist der Chip für den Screenreader stumm (`ExcludeSemantics`), weil Zustand und Tatsache „Decision“ „Allowed, edited“ schon sagen, auch nach einem Fehler des Ziels (am Semantikbaum gemessen, beide Fälle). Dabei gefunden und behoben: Ein `HBadge` ohne `onTap` in einer `HRow` ohne `onTap` erbte vom `shad.Clickable` der Zeile den Zustand `disabled` und schrieb sein Wort in `fg2`; `HBadge` zieht jetzt `WidgetStatesProvider.boundary`. Goldens: neu `queue_row_edited_*`, `history_detail_edited_*`; die sechzehn History-Goldens nachgezogen (Spaltenbreite, Chip). Mutationen F1 bis F7 und S1 alle rot.
- [x] Der Upstream erhält `content-length` passend zur Bytelänge, kein `transfer-encoding`, kein `content-encoding` (Integrationstest mit axum-Fake-Upstream aus HUM-017). **Auf dem Draht gemessen am 2026-09-18**, `daemon/crates/proxy/tests/edited_wire.rs`: Der Agent schickt `chunked` mit `content-encoding: gzip`, die Freigabe bringt selbst `transfer-encoding`, `content-encoding`, `expect` und `content-length: 999` mit; das `/echo` der Karte `Matrix` meldet, was es empfangen hat. `an_edited_body_arrives_with_its_byte_length` (16 Bytes ⇒ `content-length: 16`, sha256 des bearbeiteten Rumpfs), `a_multi_byte_body_is_counted_in_bytes` („Grüße an 🌍 Berlin ✓", 19 Zeichen, 26 Bytes ⇒ `content-length: 26`), `an_emptied_post_arrives_with_content_length_zero` (POST mit leerem Rumpf ⇒ `content-length: 0`, Rumpf 0 Bytes) und `an_emptied_get_arrives_without_content_length` (die Bearbeitung macht ein GET ohne Rumpf daraus ⇒ kein `content-length`). Von selbst schrieb hyper für einen leeren `Full`-Rumpf keinen `content-length`, auch nicht für einen POST; `upstream::build_outgoing` setzt ihn jetzt ausdrücklich nach `upstream::wire_content_length`, derselben Regel, die `edit::apply_edit` für die Aufzeichnung nimmt (RFC 9110 8.6). Das gilt auch für eine gewöhnliche Freigabe; kein bestehender Test änderte sich. In allen drei: kein `transfer-encoding`, kein `content-encoding`, kein `expect`, freie Kopfzeile und `content-type` der Bearbeitung kommen an. `transfer-encoding` fällt an zwei Wänden, `DAEMON_OWNED_HEADERS` und `HOP_BY_HOP` in `upstream.rs`; erst beide zusammen entfernt macht den Test rot.
- [x] Alle Tests aus dem Abschnitt Tests grün, `flutter analyze` und `cargo clippy -D warnings` sauber. **Neu gemessen am 2026-09-18** nach der Review-Runde: `flutter analyze --fatal-infos` 0 Befunde (App und `packages/ui`), `dart format --set-exit-if-changed` sauber (426 Dateien), `flutter test` 1166 grün / 4 übersprungen / **2 rot** (nach der Delta-Review 1178 grün, nach der dritten 1186 grün, jeweils 4 übersprungen und dieselben 2 rot) — die beiden TLS-Goldens, die gegen den Rollbalken von HUM-150 gezogen sind, den dieser Arbeitsbaum nicht hat; mit dem Code von `main` sind alle 14 Goldens von `intercept_golden_test.dart` grün (siehe Stand). Unter `test/features/editor` 81 grün (nach der Delta-Review 93, nach der dritten 101), `packages/ui` 129 grün, `cargo test -p humanitl-proxy -p humanitl-ipc -p humanitl-core` ohne einen roten Test, darunter 26 in `edit::tests`. `rustfmt --edition 2024 --check` sauber, `tools/check-deps.sh` und `tools/check_coupling.py` grün. **`cargo clippy` ist auf diesem Rechner nicht installiert und wurde nicht gefahren**; CI mit `STRICT=1` ist die Messung.
- [x] Neue ARB-Schlüssel `editor*` in `en` und `de`. **Gemessen am 2026-09-18**: 35 Schlüssel mit Präfix `editor` (vier neu: `editorMethod`, `editorPath`, `editorTimedOut`, `editorSendFailed`), in beiden Dateien dieselbe Menge; nach der Delta-Review 38 (dazu `editorHeaderOwned`, `editorHeaderInvalidName`, `editorHeaderInvalidValue`), wieder in beiden Dateien dieselbe Menge, dazu `interceptKeyEdit` für die Taste auf dem Control.

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

### Stand (2026-09-13)

Der Editor steht, der `AllowEdited`-Pfad im Daemon steht, und an fünf Stellen
weicht die Umsetzung von der Spezifikation ab. Jede Abweichung steht hier mit
ihrem Grund; drei davon sind Entscheidungen, die für Folge-Issues gelten.

**1. Keine Proto-Änderung, kein `ReplacementProto`, keine neue Minor-Version.**
Die Spezifikation sieht `repeated ReplacementProto replacements = 10` in
`DecideRequest` vor. Gebaut ist es nicht, und zwar aus demselben Grund, aus dem
dieses Proto schon einmal ein Feld zurückgenommen hat: Nummer 6 trug
`acknowledge_findings`, „kein Handler hat es je gelesen und kein Client hat es
je gesetzt", und der Kommentar an `reserved 3, 6` sagt, warum das falsch war —
„ein Feld, das eine Zusage macht und nichts bewirkt, wäre eine falsche Aussage
über den Vertrag" (`backlog/CONVENTIONS.md` 4.13). Die Ersetzungen hätten in
diesem Issue keinen Leser: `findings.resolved` und das Audit-Log füllt HUM-048,
und das Mapping lebt bis dahin ausdrücklich nur im Speicher. Der Editor trägt
die Liste deshalb bis an die Naht (`EditorHost.onSend` bekommt sie) und legt sie
dort ab. Wer HUM-048 baut, fügt Feld **10** hinzu (3, 4 und 6 sind belegt oder
gesperrt, 7 ist `allow_edited`, 8 und 9 gehören HUM-049), hebt `PROTO_MINOR` von
11 auf 12 und zieht `docs/PROTOCOL.md` und `app/lib/core/ipc/proto_version.dart`
nach.

**2. Kein `daemon/crates/ipc/src/decide.rs`.** Der Weg von `EditedRequest` zu
`HttpRequest` gibt es schon, an genau einer Stelle: `validate.rs::decide_plan`
liest die Entscheidung und ruft `convert::request_from_proto`, prüft den Body
gegen `limits.hold_body_cap_bytes` und lehnt eine unlesbare Anfrage mit `IPC_004`
ab, statt sie still zu `allow` zu ergänzen. Eine zweite Datei daneben wäre eine
zweite Tür in denselben Raum.

**3. `hold.rs` ist unverändert.** Die Warteschlange trägt die `Decision`, sie
führt sie nicht aus; ausgeführt wird in `FlowHandler::carry_out`
(`handler.rs`), und dort hängt jetzt `edit::apply_edit`. Die Spezifikation
nennt `hold.rs`, weil sie vor HUM-015 geschrieben wurde.

**4. Kein `re_editor`.** Das Paket ist keine Abhängigkeit dieser Anwendung, und
es aufzunehmen hieße, `pubspec.lock` anzufassen — die Datei, die versioniert
ist, damit ein Release eines fremden Pakets den Bau nicht ohne Commit ändert.
Flutter kann dasselbe ohne Paket: `TextEditingController.buildTextSpan` ist der
Haken, den `EditableText` bei jedem Zeichnen ruft, und `HDecoratedTextController`
in `packages/ui` hängt sich dort ein. Der Unterschied zu `re_editor` ist, dass
der ganze Text in einem `TextSpan`-Baum steht statt einer sichtbaren Zeile; für
die Rümpfe, die dieser Editor annimmt — Binärdaten und Übergroßes sind
ausgeschlossen —, ist das dieselbe Arbeit, die `EditableText` ohnehin tut. Der
Fallstrick „`re_editor` rendert nur sichtbare Zeilen" entfällt damit, der
Fallstrick der Offset-Räume nicht: Er ist gelöst, siehe unten.

**5. Die Naht liegt in der Shell, nicht in der Warteschlange.** Der Editor
ersetzt den mittleren Pane des Intercept-Bildschirms, aber kein Feature
importiert ein anderes (`docs/ARCHITECTURE.md` 5, `tools/check-deps.sh` prüft
das und war beim ersten Anlauf rot). `InterceptScreen` nimmt deshalb einen
`InspectorEditorBuilder` entgegen — eine Funktion über Kerntypen —, die Shell
reicht `buildEditorPane` aus `features/editor/editor_host.dart` hinein, und
`ActionBar` bekommt `onEdit` als Rückruf statt eines fremden Providers. Ohne
Builder tut `E` nichts und das Control steht gar nicht erst da. Der Editor holt
sein Detail über `editorDetailProvider` selbst aus `core/ipc`; dass jedes
Feature sein Detail aus einem eigenen Provider liest, steht schon in
`core/body/body_providers.dart`.

**Die zwei Offset-Räume.** Der Daemon zählt Bytes, Dart zählt
UTF-16-Code-Units. Umgerechnet wird **einmal beim Laden**, in `buildDraft`: für
den Rumpf über `mapBodyFindings` aus `core/body/body_span.dart` (derselbe
Dekodierer, der den Text auch zeichnet, HUM-030), für Kopfzeilen und Query über
`charSpanOfBytes` mit demselben `byteToCharOffsets`. Zurückgerechnet wird gar
nicht: Was hinausgeht, ist der fertige Text, und die Bytes zählt der Daemon
(`utf8.encode` in `buildEditedRequest`, `body.len()` in `apply_edit`).

**Neu im Diagnose-Register**: Bereich `edit` (`EDIT_001..009`), belegt sind
`EDIT_001` Ziel, `EDIT_002` Methode, `EDIT_003` Pfad, `EDIT_005` Rumpf über der
Grenze. **`EDIT_004` bleibt mit Absicht frei**: Die Spezifikation gab ihm die
gesperrte Kopfzeile im Edit, und die wird nicht abgelehnt, sondern still
verworfen und als `edit.locked_header_dropped` vermerkt — der Daemon setzt diese
Werte selbst. `backlog/CONVENTIONS.md` 4.6 führt den Bereich noch nicht; die
Zeile gehört dorthin nachgetragen.

**`EDIT_005` misst gegen `limits.hold_body_cap_bytes`, nicht gegen
`preview.cap_bytes`.** Das ist die Grenze, gegen die der Hold wirklich gepuffert
hat und die der Proxy zur Hand hat (`ProxyLimits.body_cap_bytes`);
`preview.cap_bytes` ist die Anzeigegrenze, und über ihr schaltet der Editor das
Bearbeiten client-seitig ab (`Draft.bodyIsEditable`).

**Zwei Fehler, die beim Bauen aufgefallen sind und mitbehoben wurden.** Erstens
fiel `carry_out` bei einer Bearbeitung ohne Inline-Bytes auf den **ursprünglichen**
Rumpf zurück — wer den ganzen Rumpf löschte, schickte damit genau das hinaus,
was er entfernt hatte. Jetzt ist ein leerer Rumpf ein leerer Rumpf
(`an_emptied_body_goes_out_empty`, `a_detached_body_goes_out_empty`). Zweitens
lief die Aktionsleiste mit dem dritten Control bei 652 px um 142 px über;
`actionBarWrapWidth` steht deshalb auf 800 statt 640, und achtzehn Goldens sind
nachgezogen.

**Die Naht zur Shell ist der einzige Eingriff ausserhalb der Pfade dieses
Issues**, neben `lib.rs` (Modulzeile), `handler.rs` (der `AllowEdited`-Zweig),
`codes.rs` (Register, angehaengt), `core/shortcuts/intents.dart` (`EditIntent`),
`features/intercept/intents.dart` (Bindung `E`), `humanitl_ui.dart` (Export) und
achtzehn Goldens. Wer in derselben Runde `remember_grid.dart` anfasst, muss die
Goldens der Aktionsleiste nach dem Merge noch einmal ziehen
(`flutter test --update-goldens test/goldens`).

### Stand nach der Review-Runde (2026-09-18)

Antigravity und der Ersatz-Reviewer für Codex fanden zusammen drei blockierende,
sechs größere und dreizehn kleinere Punkte. Behoben ist, was hier steht; offen
ist, was unter „Was offen bleibt" steht, jeweils mit dem Issue, dem es gehört.

**Blockierend, behoben.**

- *`Content-Type` erreichte den Draht nicht.* Der Rückfall auf den Typ der
  gehaltenen Anfrage stand nur in `BodyRef.content_type`, und
  `upstream::build_outgoing` baut die Anfrage allein aus `request.headers`.
  **In der Delta-Review aufgehoben**: Der Rückfall selbst ist entfernt, siehe
  den nächsten Abschnitt. Die Tests prüfen weiter die **Kopfzeile**, nicht das
  Feld (`content_type_of_the_edit_wins`, `no_content_type_anywhere_invents_none`,
  `a_deleted_content_type_stays_deleted`).
- *Kopfzeilen, Methode und Pfad ließen sich nicht bearbeiten.* Freie Kopfzeilen
  sind zwei `HTextField` und melden jede Eingabe an `setHeader`; eine
  hinzugefügte Zeile lässt sich benennen und geht mit (`header_table.dart`).
  Methode und Pfad stehen im Kopf des Editors als Felder, das Ziel bleibt Text
  mit Schloss (`editor_screen.dart`, `_Header`). Der Query-Reiter bleibt eine
  **Ansicht**: Die Query ist Teil des Pfad-Felds, und zwei Ablagen für
  dieselbe Zeichenkette liefen auseinander (Fallstrick dieses Issues).
- *Ein abgelehntes Senden war stumm.* `EditorHost` wartet jetzt auf `decide`,
  fängt die `DaemonException` und zeigt ihren `Diagnostic` im Editor
  (`editor-send-error`); geschlossen wird erst nach einer Antwort ohne Fehler.

**Größer, behoben.** Gleichnamige Kopfzeilen werden über `DraftLocation.headerIndex`
unterschieden — eine Ersetzung in der zweiten `Via` lässt die erste stehen, und
`buildDraft` legt einen Fund in die gleichnamige Zeile, die ihn überhaupt
enthalten kann. Der Entwurf wird verworfen, sobald der Fluss `Recorded` ist
(er trägt jeden Originalwert im Klartext); nach `TimedOut` bleibt er lesbar,
das Senden ist aus und sagt warum (`editorTimedOut`). Der Rumpf hat die
Zeigergesten des Desktops (`TextSelectionGestureDetectorBuilder`: Doppelklick,
Ziehen, Kontextmenü), gemessen unter `TargetPlatformVariant.only(linux)`. Die
Pseudonyme gehören der **Sitzung** (`sessionPseudonymsProvider`): Die zweite
gehaltene Anfrage zählt weiter, derselbe Wert behält seinen Namen über
Anfragen hinweg.

**Kleiner, behoben.** Der zweite Scan läuft vor dem `tracing`-Makro und damit
immer, nicht nur unter `RUST_LOG=debug`; `daemon_headers` nimmt
`HeaderMap::new()` statt `with_capacity`, das oberhalb von rund 24 577
Einträgen panikte (`thirty_thousand_headers_do_not_stop_the_daemon`); `Host`
wird wie auf dem Draht ohne Standard-Port aufgezeichnet (`upstream::host_header`
ist jetzt `pub(crate)` und die eine Stelle dafür); ein ignorierter Fund
verbraucht keinen Zähler mehr; der Diff-Glow wandert nach freier Eingabe mit
seinem Pseudonym; eine IME-Komposition behält ihren Unterstrich; Kopfzeilenwerte
werden als UTF-8 gelesen und nicht über `Header.text` (das Byte für Byte, also
Latin-1, liest); die Aktionsleiste ist über Fensterbreiten von 1300 bis 1900 px
ohne Überlauf gemessen; `backlog/CONVENTIONS.md` 4.6 zählt die Bereiche nicht
mehr auf, sondern verweist auf `AREAS` — die Aufzählung nannte zehn von
achtzehn, und nur `edit` nachzutragen hätte sie vollständig aussehen lassen.

**Richtiggestellt.** `apply_edit` setzt `content-length` für die
**Aufzeichnung**; auf dem Draht setzt hyper ihn aus der Länge von
`Full<Bytes>`, weil `build_outgoing` den eigenen Wert überspringt. Für einen
leeren Rumpf lässt hyper die Kopfzeile weg: `post_with_empty_body_says_zero`
gilt für die Aufzeichnung, nicht für den Draht, und der ausstehende
Integrationstest muss die Kopfzeile prüfen, die hyper schreibt. `EDIT_005` ist im
Betrieb nicht erreichbar, weil `validate::decision_of` einen Rumpf über
derselben Grenze schon mit `IPC_004` ablehnt; er bleibt als zweite Wand im
Proxy, der die Grenze selbst kennt, und ist genau deshalb eine andere Aussage
als das unbelegte `EDIT_004`.

### Stand nach der Delta-Review (2026-09-18)

Beide Reviewer fanden unabhängig dieselbe Lücke, dazu drei weitere Punkte;
alle vier sind behoben.

**Der Entwurf überlebte seinen Editor.** Der Horcher auf `Recorded` stand im
`EditorHost`, und den gibt es nur, solange der Editor offen ist. Nach dem
Senden und nach `Esc` ist er zu, bevor `Recorded` kommt — erst nach der
Antwort des Ziels —, und jeder Originalwert blieb im Klartext bis zum Ende der
Anwendung liegen. Der grüne Test dazu war grün, weil `FakeDaemonClient.decide`
`Recorded` schon im Aufruf meldet. Jetzt hört `DraftNotifier` selbst: Er baut
den Horcher in `build` auf und setzt sich bei `Recorded` seines Flusses auf
`null`. Gemessen mit einem Fake, dessen `decide` nur mitschreibt, und
`Recorded` erst drei Sekunden später aus dem Skript: einmal nach dem Senden,
einmal nach `Esc`. Der Horcher ist ein Abo am Container, kein `ref.listen`:
Riverpod 3 hält die `ref.listen`-Abos eines Providers an, an dem niemand mehr
horcht, und nach dem Schließen des Editors horcht am Entwurf niemand mehr. Mit
`ref.listen` blieb der Test nach dem Senden rot, weil dort sonst niemand am
Ereignisstrom hing und der Strom mit ruhte; in der App hält ihn der Tray wach,
darauf soll sich das Aufräumen aber nicht verlassen.

**Der Stand der Sitzung hielt Klartext und endete nie.** Der Schlüssel einer
Auswahl aus `Ctrl+R` ist `manual:<LABEL>:<Text>`, also der Text selbst, und der
Stand ist `keepAlive`. Er nimmt solche Schlüssel jetzt nicht an; sie bleiben im
Entwurf und verschwinden mit ihm. Dieselbe Auswahl heißt innerhalb einer Anfrage
gleich, über Anfragen hinweg nicht mehr — das ist der Preis, und HUM-048 hebt ihn
auf, weil dort der Daemon hasht. Außerdem gehört der Stand jetzt **einer**
Sitzung: `DraftSource.session` trägt die `SessionId` des Flusses, und nennt ein
Entwurf eine andere, beginnt der Stand leer.

**Der Rückfall auf den `Content-Type` des Originals ist entfernt.** Er war die
Antwort auf den blockierenden Befund der ersten Runde, dass der Rückfall den
Draht nicht erreichte — und er war schon als Idee falsch: Der Editor beginnt
immer mit der Zeile des Originals, nennt die Bearbeitung also keinen Typ, hat
der Mensch die Zeile gelöscht, und die Spezifikation erlaubt das Löschen
ausdrücklich. Der Rückfall machte die Löschung rückgängig; ein geleerter Rumpf
ging weiter als `application/json` hinaus. Jetzt gilt allein, was die
Bearbeitung nennt (`a_deleted_content_type_stays_deleted`).

**Kopfzeilen, die nie ankämen, meldete der Editor als gesendet.** Eine freie
Zeile namens `Host` oder `Connection` streicht der Daemon, einen Namen, der kein
Token ist, oder einen Wert mit Zeilenumbruch lässt `headers_from_proto` fallen
— der Draht blieb sauber, der Bildschirm nicht. `checkHeaders`
(`draft_ops.dart`) prüft jetzt Token (RFC 9110 5.6.2), Feldwert (5.5) und die
gesperrten Namen, hält das Senden an und nennt die Zeile. Die Zeile selbst
bleibt editierbar; eine, die sich beim Umbenennen sperrte, ließe sich weder
zurückbenennen noch löschen.

**Nachgetragen, kein Fehler dieses Diffs.** `EDIT_001`, `002`, `003` und `005`
entstehen in `carry_out`, nachdem `decide` schon `applied` gemeldet hat. Sie
erreichen `editor-send-error` deshalb nie: Der Fluss wird zu `Block` revidiert,
und der Editor schließt sich. Methode und Pfad prüft die Oberfläche genauso wie
`check_method` und `check_path`, ein gewöhnlicher Mensch trifft darauf nicht;
dass ein Befund des Daemons zu einer bearbeiteten Anfrage den Editor erreicht,
ist eine eigene Nacharbeit.

**Wo die Pause von Riverpod gemessen ist.** Nur der Test „Recorded after the
editor closed on send clears the draft" fängt den Rückbau von
`ref.container.listen` auf `ref.listen`. Im Test nach `Esc` horcht der Bildschirm
der Warteschlange weiter am Ereignisstrom, und die Pause greift dort nie. Der
Fall „niemand sonst horcht" hängt damit an diesem einen Test.

### Stand nach der dritten Delta-Review (2026-09-18)

Drei kleinere Befunde, alle behoben:

**Ein alter Entwurf setzte den Stand der Sitzung zurück.** `naming()` leerte
den Stand bei **jeder** anderen Sitzung, auch einer älteren. Ein Entwurf von
vor einem Neustart des Daemons, der noch offen war, setzte ihn so auf die alte
Sitzung, und der nächste Entwurf der laufenden setzte ihn wieder zurück: Ihre
Zähler begannen von vorn, und `<EMAIL_1>` stand in ihr für zwei Werte. Jetzt
leert nur eine **neuere** Sitzung den Stand. Eine `SessionId` ist eine UUIDv7,
der Textvergleich der kanonischen Form ordnet nach der Zeit. Ein alter Entwurf
bekommt einen Namensgeber nur über seiner eigenen Zuordnung, der von deren
höchstem Namen weiterzählt (`countersOf`), und `remember` verwirft ihn.

**Drei Kopfzeilen fehlten in der Sperre.** Der Daemon streicht vor dem
Weiterleiten auch `keep-alive`, `te` und `trailer` (`HOP_BY_HOP` in
`upstream.rs`). Eine freie Zeile mit diesem Namen kam nie an, ohne dass es
jemand sah. Sie stehen jetzt in `lockedHeaderNames`. Ein Test liest
`DAEMON_OWNED_HEADERS` und `HOP_BY_HOP` als Text aus dem Rust-Quelltext und
hält die Dart-Liste in beide Richtungen dagegen. Die Sperre aller `proxy-*` ist
bewusst weiter als der Daemon und steht so im Kommentar.

**Das Aufräumen hing an einem einzigen `Recorded`.** Ging es in einer Lücke
verloren, blieben die Originalwerte bis zum Ende der Anwendung liegen, und
jeder je geöffnete Entwurf hielt einen Horcher am Strom. Jetzt besteht der
Horcher nur, solange ein Entwurf steht, und schließt sich beim Wegräumen. Auf
`Lagged` fragt der Entwurf mit `GetFlow` nach seinem Fluss und räumt sich weg,
wenn der Daemon ihn nicht mehr kennt (`IPC_003`, nach einem Neustart) oder er
`recorded` oder `failed` ist. Ein Fluss dazwischen behält ihn, sein `Recorded`
steht noch aus.

**Die zwei TLS-Goldens stehen auf `main`.** HUM-150 hat nach dem Abzweig
dieses Arbeitsbaums `intercept_diagnostic_tls_dark.png` und `_light.png` um einen
Rollbalken an der Befund-Leiste ergänzt. Beide Bilder sind hier gegen den Code
von `main` neu gezogen — die vier Dateien, die `main` in `app/lib` geändert hat,
lagen dafür vorübergehend im Baum und sind danach byte-genau zurückgelegt. Der
Streifen des Rollbalkens (x 385..389, y 85..475) ist in beiden Bildern Pixel
für Pixel der von `main`; es unterscheiden sich allein 3296 Pixel in y 632..651,
die Zeile mit dem Edit-Control. Mit dem Code von `main` sind alle 14 Goldens von
`intercept_golden_test.dart` grün. **In diesem Arbeitsbaum allein** schlagen
die zwei TLS-Goldens deshalb fehl: Ihm fehlt der Rollbalken-Code von HUM-150.
Nach dem Zusammenführen stimmen Code und Bild überein.

**Was offen bleibt**, jeweils mit der Stelle:

- *Die Aktionen je Fund* — „Ersetzen", „Alle mit diesem Wert ersetzen",
  „Ignorieren" — und das Hover-Popover am Diff-Glow. Beide hätten im Popover
  gewohnt; `DraftNotifier.replace`, `replaceAllOfValue` und `ignore` sind
  gebaut und getestet, haben in `lib/` aber keinen Aufrufer. Der Satz des
  Ziels „Jeder Fund kann einzeln ersetzt, für alle Vorkommen ersetzt oder
  ignoriert werden" ist damit **nicht** erfüllt. `HPopover` nimmt heute nur
  zwei Zeichenketten; das Popover braucht ein Widget-Popover in
  `packages/ui` und gehört zu HUM-048, das die Zuordnung ohnehin ausbaut.
- *Aliasse aus `findings.user_terms`.* `EditorHost` reicht keine herein, weil
  die Anwendung die Konfiguration noch nicht liest: `GetConfig` bekommt seinen
  Dart-Client mit HUM-069. Bis dahin bekommt ein Nutzerbegriff `<TERM_n>`, und
  `replaceAllOpen_userTermAlias` prüft nur die reine Funktion.
- *Die dauerhafte, verschlüsselte Zuordnung* und die Vergabe der Namen durch
  den Daemon: HUM-048, samt `ReplacementProto` auf Feld 10 und
  `PROTO_MINOR` 12. Bis dahin liegt die Zuordnung nur im Speicher der Sitzung.
- *Senden mit offenen Funden*: HUM-049. Der Editor sendet heute ohne Rückfrage,
  wie viele Funde auch offen sind; der Zähler daneben sagt es.
- Der Text-Chip „Edited" im Queue-Abgang (`queue_row.dart`), der
  Integrationstest gegen den Fake-Upstream (`daemon/crates/proxy/tests/`),
  `remaining_findings` als Feld am `Decided`-Ereignis (braucht `core-types`,
  Proto und `ipc`; heute steht die Zahl nur im Tracing), und der Rescan mit
  300 ms Debounce nach freier Eingabe (heute wandern offene Rumpf-Funde mit
  ihrem Treffertext mit und werden ignoriert, sobald er verschwindet oder
  mehrdeutig wird).

---

## HUM-048 · Pseudonym-Mapping und Schlüsselverwaltung
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-026, HUM-047 · Blockiert: HUM-049, HUM-050, HUM-055

> **Verschoben nach dem MVP** (Umfangsentscheidung 2026-09-18, siehe Kopf dieser Datei). Die Spezifikation gilt für die spätere Umsetzung unverändert.

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
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-047, HUM-062 · Blockiert: HUM-055

> **Verkleinert** (Umfangsentscheidung 2026-09-18, siehe Kopf dieser Datei): gebaut werden die Inline-Pause mit „Trotzdem senden", „Pseudonymisieren", „Blockieren" und die harte Sperre `HOLD_004`. „Ignorieren", „Immer ignorieren" und die Allowlist entfallen im MVP; Kriterien dazu werden als „verschoben" markiert, nicht abgehakt.
>
> **Aufteilung 2026-09-19** (nach den Reviews von HUM-049): Kriterium 3 — die Anfrage halten, „Senden nicht möglich" anzeigen und `Decide(Allow)` über gRPC mit `HOLD_004` ablehnen — geht an **HUM-159**; der Teil von Kriterium 2, der `unresolved_findings` in der History verlangt, geht an **HUM-160**. Gründe: Der Daemon blockt eine Anfrage mit bestätigtem Geheimnis unter `hold.hard_block_checksum_secrets` seit HUM-023 sofort und ohne Halten; das ist strenger als die Spezifikation, weil auch eine Regel `allow` und die Durchreiche daran nicht vorbeikommen. Die Weigerung an `Decide(Allow)` gehört nach `daemon/crates/ipc/src/server.rs`, und diese Datei baut HUM-156 gleichzeitig um. HUM-049 liefert deshalb `HOLD_004` an der sofortigen Sperre und an einer bearbeiteten Fassung (`backlog/CONVENTIONS.md` 4.33). Beide Folge-Issues bleiben im MVP und blockieren HUM-055.

### Kontext
Usability-Review, Abschnitt 3 und 5 in BACKLOG.md 5: Der Nutzer darf nicht genervt, aber auch nicht überrascht werden. Bei offenen Funden muss „Allow" sichtbar anders aussehen, und es muss eine Pause geben, keine Modal-Dialoge. Das Security-Review verlangt, dass die Durchsetzung im Daemon liegt, nicht im UI. Dieses Issue liefert die Pause und die harte Sperre `HOLD_004`; das Setting `hold.hard_block_checksum_secrets` gab es schon seit HUM-023.

### Ziel
**Was HUM-049 liefert (Stand 2026-09-19).** Hat eine gehaltene Anfrage offene Findings, heißt der Allow-Button „Senden mit 2 Findings" und ist amber. Ein Klick, `Enter` oder `A` öffnen bei einer einzelnen Anfrage statt des Sendens eine Pause innerhalb der Karte: Liste der offenen Funde mit Art, gekürztem Wert (`display_prefix` des Daemons) und Ort, dazu „Trotzdem senden" (`S`), „Pseudonymisieren" (`P`, öffnet den Editor mit allen Funden ersetzt), „Blockieren" (`B`) und „Zurück" (`Esc`). Das Halten der Freigabe sendet weiter ohne Pause; über eine Gruppe bleibt es beim Halten. Im Daemon trägt die sofortige Sperre unter `hold.hard_block_checksum_secrets` den Befund `HOLD_004`, und dieselbe Prüfung sperrt eine bearbeitete Fassung, die nach dem zweiten Scan noch ein bestätigtes Geheimnis trägt (`backlog/CONVENTIONS.md` 4.32).

**Ziel vor der Aufteilung** (nicht mehr Umfang von HUM-049; „Ignorieren", „Immer ignorieren" nach dem MVP, die Weigerung an `Decide(Allow)` und „Senden nicht möglich" in HUM-159): Hat eine gehaltene Anfrage offene Findings, heißt der Allow-Button „Senden mit 2 Findings" und ist amber. Ein Klick (oder Enter) öffnet statt des Sendens eine Inline-Pause innerhalb der Karte: Liste der offenen Funde mit Typ und maskiertem Wert, drei Buttons „Trotzdem senden", „Pseudonymisieren" (öffnet Editor mit „Alle ersetzen" bereits ausgeführt), „Blockieren". Jeder Fund hat „Ignorieren" (für diese Anfrage) und „Immer ignorieren" (Allowlist). Ist `hold.hard_block_checksum_secrets = true`, verweigert der Daemon `Allow` für Anfragen mit ungelösten Findings der Tier `Checksum` in den Kinds `ApiKey`, `Jwt`, `Iban`, `CreditCard` mit Diagnostic `HOLD_004`; die UI zeigt in diesem Fall „Trotzdem senden" nicht an.

### Nicht-Ziel
- Findings in Responses.
- Automatisches Pseudonymisieren ohne Klick (Regel-Aktion `redact` kommt in einem eigenen Issue nach dem MVP).

### Betroffene Pfade
Geliefert:
- `app/lib/features/intercept/widgets/action_bar.dart`, `app/lib/features/intercept/widgets/findings_pause.dart` (neu), `app/lib/features/intercept/providers/findings_pause.dart` (neu), `app/lib/features/intercept/providers/decision.dart` (`allow({remember, acknowledged})`, `findingsPauseVisibleProvider`), `app/lib/features/intercept/intents.dart`, `app/lib/features/intercept/intercept_screen.dart`
- `app/lib/features/editor/editor_host.dart`, `app/lib/features/editor/editor_screen.dart` (`replaceAllOnOpen`)
- `daemon/crates/proxy/src/findings.rs` (`check_allow`, `hard_blocks`), `daemon/crates/proxy/src/handler.rs`, `daemon/crates/proxy/src/edit.rs`
- `daemon/crates/core-types/src/diagnostics/codes.rs` (`HOLD_004` am Ende), `docs/DIAGNOSTICS.md` (erzeugt), ARB-Dateien

Vor der Aufteilung genannt und nicht angefasst (HUM-160 und nach dem MVP):
- `app/lib/features/intercept/providers/decision.dart` (`acknowledgedFindings`, `ignoreAlways`)
- `daemon/crates/proxy/src/hold.rs` (Prüfung vor `Allow`; die Prüfung steht in `findings.rs`, `hold.rs` kennt keine Funde)
- `daemon/crates/recorder/src/writer.rs` (ändern: `write_findings` schreibt `resolved`) und `daemon/crates/recorder/src/query.rs` (ändern: `findings_of` liest es, Allowlist-Abfrage)
- `daemon/crates/proxy/src/findings.rs` (Trait `Scanner`) und `daemon/crates/findings/src/registry.rs` (`scan`; ändern: Allowlist beim Scan anwenden)
- `daemon/crates/config/src/model.rs` (ändern: neues Feld in `HoldConfig`)
- `daemon/crates/core-types/src/diagnostics/codes.rs` (ändern: `HOLD_004` ans Ende anhängen) und `docs/DIAGNOSTICS.md` (erzeugt)
- ARB-Dateien

### Spezifikation

> **Stand vor der Aufteilung.** Was hier folgt, ist die ursprüngliche Spezifikation. Geliefert ist davon, was unter „Ziel" steht. `acknowledged_findings`, `unresolved_findings` im `Decided`-Ereignis und im Audit sind **HUM-160**; die Weigerung an `Decide(Allow)`, die Zeile „Senden nicht möglich" der Tabelle und `hard_block_hides_send_anyway` sind **HUM-159**; `ignore_always`, die Allowlist, `allowlisted` und die Knöpfe „Ignorieren" und „Immer ignorieren" je Fund sind **nach dem MVP** (BACKLOG.md Abschnitt 9, Punkt 15). Die Zeile „Editor-Entwurf vorhanden" ist **HUM-161**.

**Config** (gab es schon, HUM-023): `hold.hard_block_checksum_secrets: bool`, Default `false`, Tier `advanced`, Beschreibung: „Anfragen mit prüfsummen-verifizierten Secrets (API-Keys, JWT, IBAN, Kreditkarte) können nicht ungeändert gesendet werden." Sicherheitsrelevant, daher im Settings-Screen mit Hinweis.

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

`acknowledged_findings` markiert die Funde in der Tabelle `findings` als `resolved = 'acknowledged'`. Die Spalte `resolved` ist bereits `TEXT` (`daemon/crates/recorder/migrations/V1__init.sql`, Kommentar `NULL | replaced | ignored`); eine Migration entfällt, es kommen nur die Werte `acknowledged` und `allowlisted` hinzu (Korrektur aus HUM-089). `HOLD_004` gibt es im Register noch nicht; der Code wird in diesem Issue angelegt, ans Ende von `daemon/crates/core-types/src/diagnostics/codes.rs`. `ignore_always` schreibt `finding_allowlist(value_hash, kind, scope)`; `scope` ist `global`, wenn kein Projekt-Profil aktiv ist, sonst SHA-256-Hex des kanonischen Projektpfads. Beim nächsten Scan (HUM-025 `Scanner`) werden Findings mit Allowlist-Treffer mit `allowlisted = true` markiert, erscheinen im UI ausgegraut in einer eingeklappten Zeile „3 ignoriert in diesem Projekt" und zählen nicht als offen.

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
Geliefert: `check_allow` und `hard_blocks` in `findings.rs` mit `HOLD_004` an beiden Sperren; die Pause mit ihrem Provider, ihren Tasten und dem einen Prädikat; „Pseudonymisieren" über `replaceAllOnOpen`; Widget-Tests.

Vor der Aufteilung (siehe Hinweis unter „Spezifikation"):
1. Config-Feld, die Werte `acknowledged` und `allowlisted` für `findings.resolved` (die Spalte ist schon `TEXT`, keine Migration), Allowlist-Abfrage im Recorder.
2. `Scanner` wendet Allowlist an; Test.
3. `check_allow` in `hold.rs` mit `HOLD_004`; Tests.
4. `Decide`-Handler verarbeitet `acknowledged_findings` und `ignore_always`.
5. `action_bar.dart` Zustände, `findings_pause.dart`, Provider.
6. Fake-Daemon: liefert `HOLD_004`, wenn ein Flag gesetzt ist.
7. Widget-Tests.

### Tests
Geliefert: Unit in `daemon/crates/proxy/src/findings.rs` (`allow_with_open_regex_findings_ok`, `allow_with_checksum_secret_blocked_when_setting_on`, `allow_ok_when_setting_off`, `only_checksum_secrets_of_the_four_kinds_hard_block`, `the_refusal_names_a_header_by_its_name`, `the_refusal_names_the_query_and_says_it_was_blocked`); Proxy in `daemon/crates/proxy/tests/findings.rs` (`a_checksum_secret_is_blocked_when_the_switch_is_on` mit `HOLD_004`, `an_edited_checksum_secret_is_blocked_when_the_switch_is_on`, `an_edited_checksum_secret_goes_out_when_the_switch_is_off`); Widget in `app/test/features/intercept/findings_pause_test.dart` und `findings_pause_predicate_test.dart`.

Vor der Aufteilung (siehe Hinweis unter „Spezifikation"): Unit (`hold.rs`): `allow_with_open_regex_findings_ok`, `allow_with_checksum_secret_blocked_when_setting_on` (`HOLD_004`), `allow_with_checksum_secret_ok_when_acknowledged`, `allow_ok_when_setting_off`, `allowlisted_not_counted`.
Unit (`findings`): `allowlist_marks_finding` (Wert in Allowlist mit `scope = global` ⇒ `allowlisted == true`), `allowlist_project_scope_does_not_leak_to_other_project`.
Widget: `button_label_with_findings`, `enter_opens_pause_not_send` (Fake-Daemon zählt `decide` = 0), `send_anyway_acknowledges_all`, `hard_block_hides_send_anyway`, `ignore_always_calls_decide_with_ignore_always`.

### Akzeptanzkriterien
- [x] Anfrage mit einer E-Mail im Body: Button heißt „Senden mit 1 Finding", Enter öffnet die Pause, kein Request geht raus (Fake-Daemon-Zähler). Gemessen: `app/test/features/intercept/findings_pause_test.dart`, `button_label_with_findings` (en), `the valve and the pause speak German` (de: „Senden mit 1 Finding", „1 Fund in dieser Anfrage", die drei Knöpfe), `enter_opens_pause_not_send` und `a click on the valve opens the pause and sends nothing` (`client.decisions` leer).
- [ ] „Trotzdem senden" leitet weiter; History zeigt `unresolved_findings = 1`. **Verschoben nach HUM-160** (Aufteilung 2026-09-19, oben). Weiterleiten ist gemessen: `send_anyway_sends_the_request_unchanged` und `S in the pause sends` (genau ein `Decide(Allow)` am Fake). Die Zahl in der History fehlt: `acknowledged_findings` ist nicht auf dem Draht, und `Decided` trägt keine `unresolved_findings`.
- [ ] Mit `hold.hard_block_checksum_secrets = true` und einer gültigen IBAN: „Senden nicht möglich", Daemon lehnt `Decide(Allow)` über die CLI (`humanitl flows decide` existiert nicht; Test über gRPC-Client) mit `HOLD_004` ab. **Verschoben nach HUM-159** (Aufteilung 2026-09-19, oben; `backlog/CONVENTIONS.md` 4.33). Die Anfrage wird mit dem Schalter gar nicht erst gehalten: Das System blockt sie sofort und meldet jetzt `HOLD_004` am Flow, gemessen mit `a_checksum_secret_is_blocked_when_the_switch_is_on` (`daemon/crates/proxy/tests/findings.rs`). Dieselbe Sperre greift an einer bearbeiteten Fassung, gemessen mit `an_edited_checksum_secret_is_blocked_when_the_switch_is_on` und der Gegenprobe `an_edited_checksum_secret_goes_out_when_the_switch_is_off`; die Regel selbst mit den Einheitstests in `daemon/crates/proxy/src/findings.rs`. Gebaut und gemessen ist davon nur `HOLD_004` an beiden Sperren.
- [ ] „Immer ignorieren" auf einen Wert ⇒ nächste Anfrage mit demselben Wert zeigt ihn in der eingeklappten „ignoriert"-Zeile, Button ist „Senden". **Verschoben** (Umfangsentscheidung 2026-09-18, BACKLOG.md Abschnitt 9, Punkt 15).
- [x] Alle Tests grün, ARB-Schlüssel `interceptFindingsPause*` in `en` und `de`. Gemessen: `STRICT=1 make check` grün am 2026-09-18 und nach der Review-Runde am 2026-09-19; neun Schlüssel `interceptFindingsPause*` in `app/l10n/app_en.arb` und `app/l10n/app_de.arb` (der zehnte, `interceptFindingsPauseMasked`, ist seit der Review-Runde entfernt: `display_prefix` kommt vom Daemon schon maskiert). Die Beschriftung der Freigabe, `interceptSendWithFindings`, gab es schon vor HUM-049.

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
| `audit.resumed` | `log_seq`, `anchor_seq` — der erste Record hinter einer Lücke, wenn das Log beim Start vor einem Anker endet (`AUDIT_007`, aus dem Review von HUM-050) |
| `recorder.retention_applied` | `deleted_flows`, `deleted_blobs`, `cutoff` (Zeitpunkt im Format des Logs) — nach jedem Aufräumlauf der Aufzeichnung, auch wenn nichts zu löschen war; bei `recorder.retention_days = 0` läuft keiner (nachgetragen von HUM-051) |

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
- [x] Nach einer Session enthält `audit.jsonl` `session.started`, mindestens ein `flow.received`, `flow.decided`, `session.ended`, und `audit.anchor` bei jedem `anchor_every`-ten Record. (Gemessen 2026-09-11 mit dem Merge-Stand von HUM-050: `decided_event_produces_record_without_payload` in `daemon/bin/humanitld/tests/daemon_end_to_end.rs` fährt eine Sitzung gegen einen echten Daemon und prüft die Folge der Arten bis zum Anker am Ende; `anchor_written_every_n` prüft die Anker bei jedem `anchor_every`-ten Record in Datei und `SQLite`. Beide grün, und beide unter ihrer Mutation rot gesehen.)
- [x] `grep -c` nach einem im Test verwendeten Klartext-Wert über `audit.jsonl` liefert 0. (Gemessen 2026-09-11 mit dem Merge-Stand von HUM-050: derselbe Ende-zu-Ende-Test sucht die Mail-Adresse aus dem Body, dessen Schlüssel `kontakt` und den Pfad `/contact` im Log und zählt null Treffer; `path_is_hashed` findet den Token aus der Query nicht und den Pfad nur als `path_hash`. Die Mutation „Pfad im Klartext" macht `path_is_hashed` rot.)
- [x] Alle acht Tamper-Tests grün, inklusive des dokumentierten Nicht-Erkennungsfalls. (Gemessen 2026-09-11 mit dem Merge-Stand von HUM-050: `cargo test -p humanitl-audit`, `tests/tamper.rs` 8 von 8; sechs Prüfungen des Verifiers (Hash, MAC, `seq`, kanonische Form, Anker, Kürzung unter einen Anker), einzeln abgeschaltet, machen je ihren Test rot.)
- [x] `verify` über 100 000 Records dauert unter 5 s (Bench-Test, `#[ignore]` in normalem Lauf). (Gemessen 2026-09-11 mit dem Merge-Stand von HUM-050: `cargo test -p humanitl-audit --release --test bench -- --ignored` grün, der ganze Test samt Erzeugen der Kette in 3,03 s, gedrosselt mit `nice -n 19` auf sechs Kernen.)
- [x] `docs/SECURITY.md` enthält den Abschnitt „Was die Audit-Kette beweist" mit den vier Grenzen. (Nachgelesen 2026-09-11: Abschnitt 8, vier nummerierte Grenzen; dazu der Absatz zum Start hinter dem letzten Anker, `AUDIT_007`.)
- [x] Der Golden-Vektor-Test verhindert unbemerkte Änderungen an der Kanonisierung. (Gemessen 2026-09-11 mit dem Merge-Stand von HUM-050: `hash_golden_vector` wird rot, wenn die Schlüssel umgekehrt sortiert werden oder `ts` nicht in den Hash geht.)

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
- [x] `Ctrl+5` öffnet den Screen; Status-Karte zeigt nach ≤ 2 s ein Ergebnis. Gemessen 2026-09-18 mit HUM-156: Gegen einen echten `humanitld` liefern `auditHeadProvider` und `auditVerifyProvider`, die die Karte beim Öffnen liest, beide nach 25 ms (`a_real_daemon_answers_within_two_seconds_with_the_head_of_the_cli`, `app/test/features/audit/audit_daemon_live_test.dart`, `make flutter-test-daemon`; Frist 2 s im Test). `Ctrl+5` öffnet den Abschnitt `Section.audit` (`test/features/shell/shell_test.dart`, `pressCtrl(… digit5)`); gemessen ist die Kette Taste, Abschnitt, Provider, nicht ein Bildschirm mit einem echten Daemon in einem Widget-Test.
- [x] Nach Manipulation der JSONL-Datei (Test aus HUM-050) zeigt der Screen „gebrochen ab Sequenz n" mit Grund. Gemessen 2026-09-18 mit HUM-156: Ein geändertes Zeichen im ersten Record eines echten `humanitld` ergibt in den Providern des Bildschirms `firstBadSeq 1`, `hashMismatch` und `AUDIT_001`, und `humanitl audit verify` gegen denselben Daemon dieselbe Nummer und denselben Grund (`a_changed_line_is_broken_at_its_seq_in_the_screen_and_the_cli`). Dass die Karte aus genau diesem Bericht „Kette gebrochen ab Sequenz n (Grund)" macht, misst `status_broken_shows_seq_and_reason`.
- [x] Head-Hash im UI == `humanitl audit verify --json | jq .head` (HUM-070). Gemessen 2026-09-18 mit HUM-156: `auditHeadProvider` gegen einen echten `humanitld` nennt denselben Hash und dieselbe Nummer wie `.head.hash` und `.head.seq` der Kommandozeile (`a_real_daemon_answers_within_two_seconds_with_the_head_of_the_cli`).
- [x] CSV-Export enthält die zwölf Spalten aus HUM-050, JSONL ist bytegleich mit der Quelldatei für den Zeitraum. Gemessen 2026-09-18 mit HUM-156 am Dienst, den die Oberfläche ruft: `export_writes_the_chain_and_the_twelve_columns_and_overwrites_nothing` und `export_takes_the_range_inclusively` (`daemon/crates/ipc/tests/audit_rpc.rs`); dass Format, Pfad und Zeitraum der Oberfläche dort ankommen, weiter `export_csv_calls_export_with_range`.
- [x] Retention: Flow mit `ts` vor 200 Tagen ist nach Job weg, Audit-Records bleiben. Gemessen: Der Flow geht in `deletes_older_than_cutoff_only` (`daemon/crates/recorder/tests/retention.rs`, echte SQLite-Datenbank). Die Audit-Records bleiben in `a_retention_run_keeps_the_audit_log_and_its_chain` (`daemon/bin/humanitld/src/main.rs`): Ein Record, der vor dem Lauf im Log stand, steht danach Byte für Byte noch da, dahinter genau `recorder.retention_applied`, und `AuditVerifier::verify` mit Schlüssel meldet die Kette als heil; `audit_untouched` belegt dasselbe für `audit_anchors`.

### Fallstricke
- `verify` über die gesamte Datei kann bei sehr großen Logs Sekunden dauern; UI zeigt Fortschritt aus dem Daemon? Im MVP: Spinner und Ergebnis, `verify` läuft in `spawn_blocking`.
- Tabelle nie clientseitig komplett laden; Cursor-Paging, Seiten von 200.
- Die Zusammenfassungs-Spalte darf nur Felder anzeigen, die im Record stehen. Kein Nachladen des Flows (wäre Payload).

### Referenzen
- HUM-050 Spezifikation, BACKLOG.md Abschnitt 5 (IA), DSGVO Art. 5 Abs. 1 lit. e

### Stand (2026-09-18): gebaut gegen den Fake, der Daemon antwortet noch nicht

Gebaut sind Bildschirm, Port, Proto und die Aufbewahrungsregel. Der Dienst
`Audit` im Daemon antwortet weiterhin `unimplemented`; ihn baut HUM-156. Drei
der fünf Kriterien (Ergebnis nach höchstens 2 s gegen einen echten Daemon,
Head-Hash gleich `humanitl audit verify --json`, Export-Inhalt) sind deshalb
nur gegen `FakeDaemonClient` gemessen und bleiben offen, bis HUM-156 steht. Das
Kriterium zur Aufbewahrung ist erfüllt (`daemon/crates/recorder/tests/retention.rs`).

Abweichungen von dieser Spezifikation, jede mit Grund:

- **`AUDIT_001` statt `AUDIT_010`, mit `CopyCommand` statt `OpenUrl`.** Der
  Bereich `AUDIT_001..009` ist reserviert (`backlog/CONVENTIONS.md` 4.6), und
  `AUDIT_001` „Hash-Kette gebrochen" ist genau dieser Befund. Der Bildschirm
  zeigt den Befund des Daemons unverändert (`VerifyReport::diagnostic` in
  `daemon/crates/audit/src/verify.rs`), samt dessen Vorschlag, die Datei
  beiseitezulegen. Einen eigenen Satz erfindet die Oberfläche nicht.
- **`ListView` mit `itemExtent` statt `TableView`.** Dieselbe Begründung wie
  bei der History-Tabelle (`history_table.dart`): Die Zeilenhöhe steht fest,
  also bleibt das Blättern billig, und eine Zeile ist ein Semantik-Knoten.
- **Ein Punkt statt eines Hakens bei `Ok`.** `HGlyph` kennt keinen Haken; eine
  neue Form gehört nach `packages/ui`. Der gebrochene Fall zeigt `shield-x`.
- **Zusammenfassung von `flow.decided`.** Der Record trägt weder Methode noch
  Host (`FlowDecided` in `daemon/crates/audit/src/kinds.rs`); die stehen in
  `flow.received`. Die Spalte zeigt, was im Record steht: Entscheidung und wer
  entschied, Methode und Host erst, wenn ein Record sie führt.
- **Aufbewahrung mit der Vorgabe statt des geltenden Werts, und ohne Knopf
  „Einstellungen".** Die beiden Sätze folgen der Skizze. Der Client hat aber
  kein `GetConfig` (HUM-069) und kann die geltende Frist nicht lesen; der
  erste Satz nennt deshalb 180 als Vorgabe zusammen mit dem Schlüssel, der
  sie ändert, und dass 0 nie heißt. Der zweite sagt, dass die Kette nie
  gelöscht wird, und dass `audit.retention_days` in dieser Fassung nicht
  wirkt. Statt des Knopfs „Einstellungen" kopiert ein Knopf den Befehl, der
  beide Werte ausgibt; einen Einstellungs-Bildschirm gibt es noch nicht.
- **Der Export schränkt nur nach Zeitraum ein.** Art und Sitzung der
  Filterleiste gelten für die Tabelle, nicht für die Datei; `Export` im Proto
  kennt nur den Zeitraum.
- **Der Export fragt nach einem Ordner, nicht nach einer Datei.** Der
  Speichern-Dialog von `file_picker` legt die Datei selbst an und hätte eine
  gewählte vorhandene Datei geleert, bevor der Daemon gefragt war. Die
  Anwendung wählt im Ordner einen freien Namen und schreibt nichts; der Daemon
  überschreibt nie (HUM-156).
- **`recorder.retention_applied` schreibt der Daemon, nicht `retention.rs`.**
  Die Aufzeichnung darf die Audit-Crate nicht kennen
  (`backlog/CONVENTIONS.md` 3.1). `humanitld::purge_once` hat beide: Er
  bestimmt die Grenze über `Retention::horizon`, lässt die Aufzeichnung
  löschen und schreibt danach den Record mit `deleted_flows`, `deleted_blobs`
  und `cutoff`, auch wenn nichts zu löschen war. Die Art steht in `kinds.rs`
  und in der Tabelle von HUM-050.
- **`recorder.retention_days`: Vorgabe 180 statt vorher 90, und 0 ist
  erlaubt.** Die Vorgabe stand in `humanitl-config` und in
  `humanitl_recorder::RecorderSettings` bei 90; beide stehen jetzt bei 180.
  Die Prüfung nahm 1 bis 3650 an und nimmt jetzt 0 bis 3650, weil die
  Spezifikation „0 = nie" verlangt. Die Beschreibung im Schema nennt DSGVO
  Art. 5 Abs. 1 lit. e; `docs/CONFIG.md` ist daraus neu erzeugt.
- **`audit.retention_days` hat weiter keinen Leser.** Die Spezifikation
  verlangt nur, dass die Kette bei `0` unberührt bleibt, und das tut sie. Die
  Löschung aus der Kette mit dokumentierter Lücke ist HUM-157; Register
  (`config_readers.rs`) und `x-pending-issue` nennen jetzt HUM-157.
- **`PROTO_MINOR` bleibt bei 11, obwohl `docs/PROTOCOL.md` 5 für additive
  Felder eine höhere Nebenversion verlangt.** Das ist eine bewusste Abweichung
  von der Versionsregel: `humanitl_ipc::PROTO_MINOR`
  (`daemon/crates/ipc/src/lib.rs`) und `ProtoVersion.minor`
  (`app/lib/core/ipc/proto_version.dart`) stehen weiter auf 11, weil kein
  Daemon `Audit` beantwortet und eine 12 eine Fähigkeit behauptete, die es
  nicht gibt. HUM-156 baut den Dienst und hebt beide im selben Zug auf 12. Damit
  ein älterer Daemon nicht als „null Anker" gelesen wird, trägt die Antwort
  `anchors_reported`; ohne das Feld zeigt die Karte „Anker nicht gemeldet".
- **`recorder.retention_days = 0` heißt in der Aufzeichnung „nie".**
  `Recorder::purge_expired` rechnete vorher `jetzt − 0 Tage` und hätte alles
  gelöscht; das war ein echter Fehler, solange die Konfiguration die Null
  abwies nur verdeckt. Mit der erlaubten Null ist `Retention::from_days` die
  Stelle, die ihn verhindert (`zero_means_never`).
- **Der Code für einen Exportnamen ohne freien Platz ist `IPC_006`.** Passender
  wäre `AUDIT_008` „Audit-Export nicht schreibbar" aus HUM-070, das zur Zeit
  dieses Issues noch nicht auf `main` stand. Der Wechsel ist HUM-158.

**Nachtrag 2026-09-18 (HUM-156).** Der Dienst `Audit` antwortet, `PROTO_MINOR`
und `ProtoVersion.minor` stehen auf 12. Die drei Kriterien, die an ihm hingen,
und das zum Export sind gegen einen echten Daemon gemessen und oben abgehakt;
`IPC_006` für einen vollen Ordner ist mit HUM-158 `AUDIT_008`.

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

> **Verkleinert** (zweite Umfangsentscheidung 2026-09-18, siehe Kopf dieser Datei): gebaut wird ein handgebautes Formular mit neun Einstellungen zusammen mit HUM-140, geschrieben über einen Daemon-RPC mit dem Konfig-Schreiber aus HUM-070, wirksam nach Daemon-Neustart. Die Kriterien zu Schema-Rendering, Suche, Live-Neuladen, Dateiüberwachung, Herkunftsanzeige und Audit je Änderung werden als „verschoben" markiert, nicht abgehakt.

### Kontext
ADR-011 und Prinzip 8: Eine Konfigurationsquelle mit Schema; das UI wird aus dem Schema generiert, damit kein Setting nur im UI oder nur in der CLI existiert. Drei Stufen `basic`, `advanced`, `expert`. Herkunft jedes Wertes ist sichtbar. Das ist der Ort, an dem der Nutzer „viel tun kann", ohne dass der Standardweg davon belastet wird.

Seit HUM-151 (2026-09-11) antwortet `SetConfig` für genau einen Fall, eine CA-Variable unter `sandbox.env` mit dem Zertifikat der Sandbox als Wert; alles andere ist `CONFIG_014`. Geschrieben wird mit `humanitl_config::edit::set_sandbox_env`, das `config.toml` über `toml_edit` ändert, ohne Kommentare und Reihenfolge zu verlieren, das Ergebnis gegen den Parser prüft und atomar ersetzt. Dieses Issue weitet die Tür (`accepted` in `daemon/crates/ipc/src/config_rpc.rs`) und den Schreiber auf jedes Feld des Schemas; Gegenprobe und atomares Schreiben bleiben.

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
- [x] `humanitl config set hold.timeout_secs 5m && humanitl config get hold.timeout_secs` ⇒ `300`. Test `config_set_duration_parsing`.
- [x] `humanitl config set hold.ask_mode banana` ⇒ Exit 1, stderr enthält `CONFIG_003`. Umformuliert: Die Spezifikation nannte `CONFIG_001`, das im Register „Config-Datei ungültig" heißt; der Wert ist falsch, nicht die Datei (`backlog/CONVENTIONS.md` 4.31). Test `config_set_invalid_exit_1_with_config_003` prüft, dass der Zweig der Aufzählung ablehnt, und `config_set_an_enum_value_is_written`, dass jeder erlaubte Wert durchgeht.
- [x] `humanitl audit verify` nach Manipulation ⇒ Exit 4 und `BROKEN at seq`. Gemessen mit `--file` und über den Rückfall auf die Datei (`audit_verify_broken_exit_4`, `audit_verify_ok_exit_0`); nicht gegen einen Daemon, dessen `Audit`-RPC noch `unimplemented` ist.
- [ ] `humanitl daemon install` auf einer frischen Debian-VM mit systemd-user-Session ⇒ Socket aktiv, `humanitl daemon status` Exit 0. **Offen, aus zwei Gründen.** Keine VM gemessen; die Unit ist nur in einem Wegwerf-`HOME` mit einem `systemctl`-Ersatz geprüft. Und „Socket aktiv" gibt es nicht: `daemon install` aktiviert `humanitld.service`, weil der Daemon `LISTEN_FDS` noch nicht liest und ein von systemd gehaltener Socket ihn mit `DAEMON_003` stilllegte. HUM-053 stellt auf den Socket um.
- [x] `NO_COLOR=1` und Pipe ⇒ keine ANSI-Sequenzen (Test mit Regex). Test `no_color_and_a_pipe_carry_no_ansi`; die Kommandozeile färbt gar nicht.
- [x] `docs/cli.md` enthält jedes Subkommando mit einem Beispiel. Das Dokument heißt seit HUM-064 `docs/cli.md`; Test `docs_cli_names_every_subcommand_with_an_example`.

**Stand 2026-09-18.** Gebaut: `config get|set|schema|edit`, `audit verify|export`, `daemon install|status|logs`, `packaging/systemd/humanitld.socket` (liegt bereit, wird nicht installiert), `humanitl_config::edit::set_value` als der eine Schreiber von `config.toml` für Kommandozeile und `SetConfig`. Abweichungen von der Spezifikation stehen in `backlog/CONVENTIONS.md` 4.31. Offen und benannt:

- **Metaschema.** `config_schema_is_valid_json_schema` prüft die Form (`$schema`, Objekte bis zum Blatt, `x-tier` und Typ an jedem Blatt), nicht gegen das Metaschema von JSON Schema: Die Crate `jsonschema` ist keine Abhängigkeit des Workspace. Folgearbeit: sie aufnehmen und das Schema gegen Draft 2020-12 prüfen.
- **`Audit`-RPC im Daemon.** Solange sie `unimplemented` ist, prüft `audit verify` ohne `--file` die Datei ohne Schlüssel und Anker und sagt das in `warnings` und `mode: "file"`. Erledigt mit HUM-156: Der Daemon prüft mit Schlüssel und Ankern, der Rückfall greift nur noch, wenn kein Daemon antwortet, und der CSV-Export hat die zwölf Spalten aus HUM-050 statt der acht Felder eines Records.
- **Socket-Aktivierung** mit HUM-053, siehe oben.
- **Messung auf einer VM** für `daemon install`, siehe oben.

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
Maintainer: Niko Burkert <humanitl@nurkert.de>
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
- [ ] `just package` erzeugt `dist/humanitl_<ver>_amd64.deb` und `dist/Humanitl-<ver>-x86_64.AppImage`. Offen: Das Ziel heißt `make package` (kein `just` im Repository, `backlog/CONVENTIONS.md` 4.34) und lief nicht in einem Stück, weil auf diesem Rechner `patchelf` fehlt. Gemessen am 2026-09-19 sind seine Schritte einzeln: `build-binaries.sh` nach `app/linux/bundle-extra/`, `flutter build linux --release` (das Bundle hat danach `bin/humanitld`, `bin/humanitl`, `bin/humanitl-shim`, gelegt von `CMakeLists.txt`), dann `build-deb.sh` und `build-appimage.sh` in einem Ubuntu-24.04-Container; heraus kamen `dist/humanitl_0.0.0_amd64.deb` und `dist/Humanitl-0.0.0-x86_64.AppImage`, und `check-appimage.sh` bestand.
- [ ] Frische Debian-13-VM: `sudo dpkg -i …deb && humanitl daemon install && humanitl daemon status` ⇒ Exit 0; Setup-Screen zeigt Daemon grün. Offen: braucht eine VM mit systemd-Sitzung und Bildschirm (Test-Matrix in `docs/INSTALL.md`). Gemessen sind die Teile: `check-install.sh` im Container (Paket installiert, `daemon install --print` plant `enable --now humanitld.socket humanitld.service` und schreibt nichts, Purge sauber) und derselbe Daemon unter echtem systemd per Socket-Aktivierung mit `humanitl daemon status` Exit 0 (siehe nächste Zeile).
- [x] Unter der Unit läuft eine Session mit `rw`-Projekt unter `$HOME` und Escape-Tests 1–3 sind grün (Härtung bricht bwrap nicht). Gemessen am 2026-09-18: `tests/escape/run.sh` unter `systemd-run --user` mit genau den `[Service]`-Zeilen von `packaging/systemd/humanitld.service`: 124 von 124 Fällen grün (ohne Unit ebenfalls 124). Der Daemon als socket-aktivierter Transient-Dienst mit denselben Zeilen und `Type=notify`: `systemctl --user` zeigt `active (running)`, das Protokoll `socket passed by systemd`, `humanitl daemon status` endet mit 0, und `humanitl sandbox run --work <Projekt unter $HOME> -- touch /work/x` legt die Datei an. Auf dem Weg dahin fielen drei Zeilen, die jede für sich die Sandbox stilllegten: `SystemCallArchitectures=native`, `PrivateDevices`, und `sethostname` fehlte im Filter.
- [x] `systemd-analyze security` Wert dokumentiert, `RestrictNamespaces` nicht gesetzt. Gemessen am 2026-09-18: `systemd-analyze --user security --offline=true` nennt 3.7 (Grenze 4.0), festgehalten in `docs/INSTALL.md#hardening`; `the_exposure_stays_at_or_below_the_documented_value` misst nach (Mutant ohne `CapabilityBoundingSet`: 4.8, rot). `the_hardening_lets_bubblewrap_work` verbietet `RestrictNamespaces` und die übrigen gemessen schädlichen Zeilen.
- [x] `lintian` ohne Errors. Gemessen am 2026-09-19: `packaging/deb/lintian.sh` (lintian 2.117.0ubuntu1.5 im Ubuntu-24.04-Container) auf `humanitl_0.0.0_amd64.deb`: keine Fehler, keine Warnungen; nur Hinweise (`I:`) und die begründeten Ausnahmen aus `lintian-overrides`.
- [ ] AppImage startet auf Wayland (GNOME) ohne gebündeltes GTK. Offen: braucht eine Wayland-Sitzung (Test-Matrix in `docs/INSTALL.md`). Gemessen am 2026-09-19 nur ohne Bildschirm: `check-appimage.sh` findet im Bild keine `libgtk*`, `libglib*`, `libwayland*`, `libEGL*`, `AppRun` setzt kein `LD_LIBRARY_PATH`, `--cli --version` erreicht die Kommandozeile.

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

> **Vorgriff 2026-09-18 (Vorabversionen 0.0.x):** `.github/workflows/release.yml` baut mit `packaging/deb/build-deb.sh` schon ein `.deb` nach den Pfaden dieses Issues (ohne fastforge, ohne Maintainer-Skripte, lintian ohne Fehler und Warnungen, Installation und Purge im Container geprüft). Offen bleibt hier: `humanitld.socket` und `LISTEN_FDS`/`sd_notify`, die Härtung der Unit (die ausgelieferte Unit ist die ungehärtete Vorlage aus HUM-044), `daemon install` für System-Units, PNG-Symbole, `CMakeLists.txt`-Install, Setup-Schritt „Dienst aktivieren", `docs/INSTALL.md`, AppImage und die manuelle Test-Matrix.

> **Stand 2026-09-19 (HUM-053):** Gebaut sind `humanitld.socket` im Paket, `LISTEN_FDS` und `READY=1`/`STOPPING=1` im Daemon (`daemon/bin/humanitld/src/systemd.rs`, `DAEMON_013`), die gehärtete Unit (Exposure 3.7, Escape-Tests unter ihr 124/124), `daemon install` für die Units des Pakets (schreibt nichts, `enable --now humanitld.socket humanitld.service`), PNG-Symbole, der `CMakeLists.txt`-Install nach `bin/`, `make package`, das AppImage mit `AppRun --cli` und `check-appimage.sh`, `docs/INSTALL.md` mit Härtung und Test-Matrix. Der Setup-Schritt blieb der Knopf aus HUM-044: Er ruft `humanitl daemon install`, und das erkennt die Units des Pakets selbst. Abweichungen mit Grund in `backlog/CONVENTIONS.md` 4.34, Folgearbeit in HUM-164 bis HUM-168. Das AppImage ist nicht Teil von `release.yml`; das gehört HUM-060.

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

> **Angepasst** (Umfangsentscheidung 2026-09-18, siehe Kopf dieser Datei): Die Prüfungen „Mapping enthält drei Einträge" und „ein Export enthält keine Originale" entfallen mit HUM-048.

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
| `Humanitl.Decide` | `humanitl flows decide <id> allow|block [--note] [--remember PATTERN]` | `intercept/action_bar` |
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
- [x] `docs/reference/parity.md` existiert, ist eingecheckt, deckt alle RPCs ab. Gemessen 2026-09-19: `cargo xtask docs` erzeugt 16 Zeilen, eine je Methode von `Humanitl` im Vertrag (`the_real_contract_lists_the_service_methods` liest dieselben Methoden aus `proto/`), 14 mit Unterkommando, `GetConfig` und `SetConfig` als begründete Ausnahme; `make parity-check` (Paritätstests der CLI, dann `cargo xtask docs --check`) meldet `unchanged`. Löschen oder von Hand ändern der Datei macht die Prüfung rot („is missing or stale").
- [x] Neuer RPC ohne CLI-Zeile bricht `parity-check`. Gemessen 2026-09-19: `rpc Watch` im Vertrag ohne Zeile in `PARITY` lässt `scripts/ci/parity-check.sh` mit Exit 1 enden, „`Humanitl.Watch` has no CLI subcommand" (`make parity-check` meldet daraufhin Exit 2); ebenso eine gelöschte Zeile (`Humanitl.Decide`), eine umbrochene Zeile, ein Eintrag auf der Anfangszeile, eine Zeile zu einer RPC, die es nicht gibt, und ein UI-Ort ohne Datei. Test `missing_cli_fails`; wird mit ausgeschalteter Prüfung rot. Ein Eintrag in `PARITY`, den `clap` nicht kennt (`flows watch`, `run --ask terminal bogus`), macht `every_entry_names_a_subcommand_and_its_flags` rot und damit das Skript (Exit 101 aus `cargo test`, `make` meldet Exit 2). Eine fehlende UI-Zeile bleibt eine Warnung (`warn: parity: …`, Exit 0).
- [x] Ausnahmen haben Begründung. Gemessen 2026-09-19: Eine Ausnahme mit leerer Begründung lässt das Skript mit Exit 1 enden („has no reason"), eine ohne das Feld `reason` scheitert schon beim Lesen (`an_exemption_needs_a_reason_and_a_real_rpc`, rot mit ausgeschalteter Prüfung); eine begründete Ausnahme steht im Abschnitt „Ausnahmen" mit ihrem Grund (`exempt_listed`). Heute sind es zwei, `GetConfig` und `SetConfig`, beide mit Grund (`backlog/CONVENTIONS.md` 4.35).

### Fallstricke
- Die Dart-Registry kann nicht aus Rust gelesen werden; der Generator parst die `.dart`-Datei mit einer Regex auf `'Humanitl.X': '…'`; Format deshalb strikt halten.
- Streams (`Subscribe`, `Terminal`, `Browser`) haben oft kein sinnvolles CLI; `flows watch` als CLI-Entsprechung für `Subscribe` trotzdem anbieten.

### Referenzen
BACKLOG.md ADR-018; `docs/ARCHITECTURE.md` 3b; HUM-059.


## HUM-079 · Rücktausch von Pseudonymen in Text-Antworten
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-048, HUM-026 · Blockiert: HUM-055

> **Verschoben nach dem MVP** (Umfangsentscheidung 2026-09-18, siehe Kopf dieser Datei). Die Spezifikation gilt für die spätere Umsetzung unverändert.

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

Zwei Dinge werden dabei oft falsch erzählt, und dieses Issue behauptet sie nicht. **Erstens** nutzt `repeated flow_ids` kein einziger Client: `Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember})` (`app/lib/core/ipc/daemon_client.dart:39`) und `..flowIds.add(flowId.value)` (`app/lib/core/ipc/convert.dart:501-502`) schicken je eine Id, und die Oberfläche schleift selbst über die Flows und hängt `remember: i == 0 ? rule : null` an den ersten Aufruf (`app/lib/features/intercept/providers/decision.dart:615-620`). Der Unterschied zwischen Oberfläche und Kommandozeile ist nicht eine Stapel-Anfrage, sondern dass die Oberfläche die Schleife und das Anlegen der Regel selbst fährt und die Kommandozeile beides nicht hat. **Zweitens** meldet keine Prüfung die Lücke, auch nicht die geplante: Der CI-Job `parity-check` ruft `scripts/ci/parity-placeholder.sh` (`.github/workflows/ci.yml:572-582`), das Skript endet mit `exit 0`, solange `daemon/xtask/src/parity.rs` fehlt — und es fehlt, `daemon/xtask/src/` enthält nur `main.rs`. HUM-078 vergleicht auf RPC-Ebene („CI-Job `parity-check` schlägt fehl, wenn ein RPC keine CLI-Zeile hat", `backlog/sprint-4.md:1614`), und seine Beispielzeile `| Humanitl.Decide | humanitl flows decide <id> allow oder block [--note] | intercept/action_bar |` (`:1629`) ist der heutige Stand: grün, mit Lücke. *Stand 2026-09-19, nach HUM-078:* `parity-check` läuft jetzt auf `scripts/ci/parity-check.sh`: die Paritätstests der Kommandozeile gegen `clap`, dann `cargo xtask docs --check` mit `daemon/xtask/src/parity.rs`. Er bricht bei einer RPC ohne Unterkommando und ohne begründete Ausnahme; eine Prüfung auf Feld-Ebene (ob `flows decide` alle Felder von `DecideRequest` anbietet) macht er weiterhin nicht, die Lücke oben bliebe grün.

Zu klären ist außerdem ein Widerspruch im Repository. Der Doc-Kommentar `daemon/bin/humanitl/src/cmd/flows.rs:308-311` nennt die Ein-Id-Regel Absicht („auf der Kommandozeile wäre er die bequeme Art, versehentlich mehr freizugeben als gemeint"), `backlog/CONVENTIONS.md:1305-1318` (4.22) nennt dieselbe Sache eine Paritätslücke, die in ein eigenes Issue gehört. Beides stimmt für verschiedene Hälften: Die Sorge gilt einem Stapel, den niemand einzeln benannt hat; für `--remember` gibt es nirgends eine Begründung. Dieses Issue löst den Widerspruch auf, statt eine der beiden Stellen zu überschreiben.

### Ziel
`humanitl flows decide` entscheidet mehrere ausdrücklich genannte Flows in einer einzigen `Decide`-Anfrage und legt dabei auf Wunsch die Regel an, die die Oberfläche an derselben Stelle anlegt — mit `created_from_flow_id`, damit die Herkunft der Regel auch ohne Maus belegt ist. Der Vertrag, der Daemon und der Fake bleiben, wie sie sind; es entsteht nur der Client, der sie nutzt.

### Nicht-Ziel
Keine Massenentscheidung über einen Filter (`decide --filter state:held`, `--all`): Das ist genau die bequeme Art, mehr freizugeben als gemeint, vor der `flows.rs:308-311` warnt, und sie bleibt ausgeschlossen. Kein `allow_edited` auf der Kommandozeile (der Vertrag lässt dafür genau eine Id zu, `daemon/crates/ipc/src/validate.rs:67-79`, und einen Editor gibt es im Terminal nicht). Kein Flag für die Bestätigung offener Funde: Das Feld `acknowledge_findings` gibt es seit HUM-089 nicht mehr, Nummer 6 und der Name sind gesperrt, und HUM-049 baut die Bestätigung als `acknowledged_findings = 8` samt Leser; ein Flag dafür gehört dorthin. Keine Paritäts-Tabelle und kein Generator — das ist HUM-078; eine mechanische Prüfung auf Feld-Ebene (jedes Proto-Feld einer Anfrage hat eine CLI-Entsprechung) baut auch dieses Issue nicht, sie wird in `CONVENTIONS.md` 4.22 ausdrücklich als offene Grenze festgehalten, damit die nächste Lücke dieser Art nicht für geprüft gehalten wird.

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

**Entschieden am 2026-09-12 vom Projekteigentümer: Es gilt die Form von HUM-095.** `humanitl flows decide <ID> allow|block --remember <PATTERN>` nimmt das Muster direkt am Flag, und ohne `--remember` bleibt `--json` Byte für Byte das heutige Einzelobjekt `{flow_id, decision, note, applied}`; mit `--remember` kommen `created_rule_id` und `created_rule` dazu. Dieses Issue übernimmt die Flagge unverändert und entscheidet nur noch, wie die Ausgabe für mehrere Ids aussieht; `--remember-host` entfällt, und die Spezifikation oben wird beim Bau darauf gezogen.

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

Maschinell ist die Klasse unsichtbar. Der CI-Job `parity-check` läuft seit HUM-078 (2026-09-19) auf `scripts/ci/parity-check.sh`: Paritätstests der Kommandozeile gegen `clap`, dann `cargo xtask docs --check`. Er prüft nach ADR-0018 Zeile 39 bis 42 nur „RPC ohne CLI-Zeile" und ob `docs/reference/parity.md` aktuell ist, nie „Fähigkeit nur im Client" und nie Parität auf Feld-Ebene. Ein Export ohne RPC fällt keiner Prüfung auf.

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


---

## HUM-150 · Ein langer Befund wird mitten im Wort abgeschnitten
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-106 · Blockiert: nichts; aber der Satz des Daemons ist das, wofür die Karte da ist

### Kontext
Gemessen am 2026-09-11 auf Xvfb, echter Daemon, echter curl (Messung zu HUM-149): Die Karte `TLS_001` über der Warteschlange zeigt den `why`-Satz des Daemons bis „… keeps its own certificate pool, pins a c" und bricht dort mitten im Wort ab, ohne Auslassungszeichen. Darunter folgen Abzeichen und Kopierzeile. Der Rest des Satzes, der sagt, was der Mensch tun kann, ist nicht zu sehen und nicht zu erreichen.

`HDiagnosticCard` (`app/lib/core/ui/h_diagnostic_card.dart:73-117`) legt die Karte in `ClipRRect` und `IntrinsicHeight`, der `why`-Text trägt kein `maxLines`. Vermutung, nicht gemessen: Die intrinsische Höhe wird bei einer anderen Breite geschätzt als der, mit der der umbrechende Text am Ende gezeichnet wird, und der Rest wird abgeschnitten. Die Sätze von `TLS_001` sind mit HUM-149 länger geworden, der Fehler ist aber älter. Mit HUM-151 ist die Karte `TLS_001` außerdem um den Knopf „In config.toml schreiben" höher geworden: In den Goldens `intercept_diagnostic_tls_*` reicht der Deckel der Streifen über der Warteschlange (`interceptStripsMaxHeight`, 420 px, `app/lib/features/intercept/widgets/queue_pane.dart:49`) nicht mehr, und der untere Rand der Karte samt „Open request" liegt unter der Kante. Der Streifen scrollt, aber im Bild ist kein Hinweis darauf zu sehen, dass dort mehr steht.

### Ziel
Ein `why`-Satz steht vollständig auf der Karte, bei jeder Breite des Streifens und bei doppelter Textskalierung; wo der Platz des Streifens nicht reicht, wird gescrollt, nicht abgeschnitten.

### Nicht-Ziel
Kürzere Sätze im Daemon. Der Satz gehört dem Daemon (`docs/UX.md` 4.4), und die Karte zeichnet ihn, wie er ist.

### Akzeptanzkriterien
- [x] Ein Widget-Test mit dem längsten `TLS_001`-Satz (Hinweis curl, Wiederholungszähler) findet den letzten Satz des Befundes auf dem Bildschirm, bei 280 px Breite des Streifens und bei Textskalierung 2. `the_longest_tls_001_sentence_stands_whole_in_a_280_px_strip` und `a_card_with_too_little_room_scrolls_instead_of_cutting` in `app/test/features/intercept/diagnostic_card_test.dart`: Der Absatz trägt den Satz vollständig (kein `maxLines`, Höhe gleich `getMaxIntrinsicHeight`), und nach dem Scrollen liegt das Ende des Satzes mit dem Zähler ganz im Ausschnitt des Streifens beziehungsweise der Karte. Der erste dieser beiden Tests ist auch mit dem alten Aufbau grün, in jeder Zusicherung — im Streifen steht die Karte in einer Liste ohne Höhenschranke, und dort hat die Klemme nie zugeschlagen; er hält den Satz, nicht den Aufbau. Drei weitere Tests laufen unter `TargetPlatformVariant.only(TargetPlatform.linux)`, weil `flutter test` sonst als `TargetPlatform.android` läuft und die Plattform-Balken dort gar nicht entstehen: `on_linux_the_strip_carries_exactly_one_bar`, `and_the_page_key_still_reaches_the_strip`, `a_card_with_room_enough_brings_no_bar_of_its_own`.
- [x] Mutationsprobe: das bisherige Layout zurück, Test rot. Vier Proben, jede danach Byte für Byte zurückgesetzt und mit `cmp` geprüft:
  1. `IntrinsicHeight` und die `Row` statt `SingleChildScrollView` und `Stack`: `a_card_with_too_little_room_scrolls_instead_of_cutting` rot mit „Expected: null / Actual: FlutterError:<A RenderFlex overflowed by 4600 pixels on the bottom.>" (Karte 280 mal 420 px bei Textskalierung 2: die Spalte lief um 4600 px über, und das `ClipRRect` der Karte schnitt sie ohne Auslassungszeichen ab).
  2. Ohne den Balken: `the_strip_shows_that_something_stands_below_its_edge` und `on_linux_the_strip_carries_exactly_one_bar` rot, beide mit „Found 0 widgets with type `RawScrollbar`".
  3. Die Ansicht der Karte in jeder Lage statt nur unter einer Höhenschranke: `a_card_with_room_enough_brings_no_bar_of_its_own` rot („Found 1 widget with type `Scrollable`") und `and_the_page_key_still_reaches_the_strip` rot. Gemessen dazu auf Linux: Der Fokus im Kartenrahmen findet als nächstes `Scrollable` eines mit `maxScrollExtent` 0.0, „Bild ab" lässt den Streifen auf 1486.0 stehen; ohne die Ansicht findet er den Streifen mit 1680.0 und fährt auf 1680.0.
  4. Ohne das `ScrollConfiguration` im Streifen: `on_linux_the_strip_carries_exactly_one_bar` rot mit „Found 2 widgets with type `RawScrollbar`".
- [x] Die Goldens mit Karte sind geprüft oder mit Erklärung neu abgenommen. Von den 97 CI-Goldens ändern sich genau zwei, `intercept_diagnostic_tls_dark` und `intercept_diagnostic_tls_light`; der Unterschied ist der Balken in `fg2` am rechten Rand des Streifens: je 1949 Pixel, alle im Band x 385 bis 389, y 85 bis 475 — vier volle Spalten x 385 bis 388 und die Kantenspalte 389. Außerhalb dieses Bandes hat sich kein Pixel bewegt. Jede andere Karte (Setup, Sandbox, Rules, History, Aktionsleiste) bleibt unverändert: Der neue Aufbau lässt die Geometrie der Karte, wie sie war. Was ein Golden hier **nicht** zeigt: Die CI-Variante von alchemist zeichnet als `TargetPlatform.android`, also fehlen darin die Balken, die Linux über jedes `Scrollable` legt; dafür stehen die drei Tests mit `TargetPlatformVariant`.

### Was gemessen wurde, und was offen bleibt
Die Vermutung des Kontextes trifft nicht zu: `IntrinsicHeight` schätzt die Höhe richtig (280 px Breite, Textskalierung 1: Zeile 1408 px, geschätzt 1408; 560 px: 648 und 648; der `Wrap` der Abzeichen bei Textskalierung 2 zwei Zeilen, 68 und 68). Abgeschnitten wurde, weil `IntrinsicHeight` die richtige Zahl durch `BoxConstraints.tighten` reicht, das sie in die Schranke des Elternteils klemmt; unter einer zu kleinen Schranke bekam die Zeile eine zu kleine feste Höhe, die Spalte lief über, und das `ClipRRect` der Karte schnitt stumm ab. Im Streifen selbst greift diese Klemme nicht, weil die Karte dort in einer Liste ohne Höhenschranke steht; dort war der Defekt ein anderer: Die Karte ist bei 280 px Breite und Textskalierung 2 um ein Vielfaches höher als die 420 px des Deckels, und nichts im Bild sagte, dass unter der Kante noch etwas steht.

Zwei Defekte kamen erst durch die Plattform ans Licht und stehen deshalb hier: Auf Linux, macOS und Windows hängt `ScrollBehavior.buildScrollbar` über **jedes** `Scrollable` einen eigenen `RawScrollbar` (`WidgetsApp` ohne `scrollBehavior`, `app/lib/app.dart:47`), und `ScrollAction` sucht zu „Bild ab" das innerste `Scrollable` vom Fokus aus. Eine Ansicht in jeder Karte hätte damit auf Linux drei Balken im Streifen ergeben und die Bild-Tasten getötet, sobald der Fokus im Kartenrahmen steht (`docs/UX.md` 5.3). Deshalb baut die Karte ihre Ansicht nur unter einer Höhenschranke, und der Streifen schaltet unter seinem Balken die Plattform-Balken mit `ScrollConfiguration(scrollbars: false)` ab. Von den achtzehn Stellen, an denen `HDiagnosticCard` gebaut wird, hat genau eine eine Schranke und baut die Ansicht wirklich: die Fehlkarte im History-Detail (`app/lib/features/history/history_detail.dart:658`, `_Failure` in einem `Flexible` der unteren Hälfte des geteilten Panes). Dort steht auf Linux auch der Balken der Plattform über der Karte; er stört heute nichts, weil diese Karte keinen Vorschlag trägt, ihr Doku-Verweis reiner Text ist und es in ihr keinen Fokus-Halt gibt. **Das ist aus dem Bau der Widgets belegt, nicht gemessen: Kein Test fährt diesen Zweig.**

**Offen:** Das Bildschirmfoto `hum149-curl/app.png` liegt nicht im Repository und nicht auf dieser Maschine; der berichtete Anblick — der Satz bricht mitten im Wort ab, Abzeichen und Kopierzeile stehen darunter weiter — ließ sich deshalb gegen keine der beiden gemessenen Ursachen halten. Zu beiden passt er nicht genau: Die Klemme schneidet alles unterhalb des Schnitts mit ab, ein Schnitt an der Kante des Streifens ebenso. Wer das Foto hat, prüft es gegen diesen Abschnitt nach; bis dahin ist nur belegt, dass beide gemessenen Defekte den Satz unerreichbar machten, nicht, dass sie genau den fotografierten Anblick erzeugt haben.

### Befund am Rand (eigenes Issue, nicht in HUM-150 behoben)
`QueueEmptyState` (`app/lib/features/intercept/widgets/queue_pane.dart:744`) läuft über, sobald der Streifen seine 420 px nimmt und die Warteschlange leer ist: „A RenderFlex overflowed by 273 pixels on the bottom" bei 1400 mal 900 px und Textskalierung 2, „by 189 pixels" bei 1000 mal 700 px und Textskalierung 1 (gemessen am 2026-09-13 über den ganzen Bildschirm mit einem langen `TLS_001`). Ohne Karte im Streifen tritt er nicht auf. `docs/UX.md` 6 verlangt ausdrücklich „Bis `TextScaler.linear(2.0)` ohne `RenderFlex`-Overflow und ohne abgeschnittenen Absatz.", also ist das ein eigener Defekt; er ist älter als HUM-150 und wurde von ihm nicht verschlimmert.

### Referenzen
Bildschirmfoto `hum149-curl/app.png` vom 2026-09-11 (nicht im Repository, siehe oben); `app/lib/core/ui/h_diagnostic_card.dart`; `app/lib/features/intercept/widgets/diagnostic_card.dart`; HUM-106, HUM-149.

---

## HUM-148 · Die Sandbox-Goldens lesen die Wanduhr
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-040 · Blockiert: jedes `tools/verify-commit.sh` auf diesem Rechner und bald jeden CI-Lauf

### Kontext
Am 2026-09-11 schlug `tools/verify-commit.sh` zweimal fehl, obwohl die geprüften Commits `app/` nicht berührten: acht Goldens (`isolation_panel_*` und `sandbox_header_running_*`, je hell und dunkel) wichen um 0,02 Prozent ab, 176 Pixel, alle an derselben Stelle. Das Differenzbild zeigt den Grund: Unten links in der Fußzeile des Sandbox-Bildschirms ist ein Textblock ein Zeichen breiter als im Referenzbild.

Der Block ist die Laufzeit der Sandbox (`_Uptime` in `app/lib/features/sandbox/sandbox_screen.dart`). Sie rechnet `nowProvider` gegen `startedAt`. Das Gerüst der Sandbox-Tests (`app/test/features/sandbox/harness.dart`) setzt `startedAt` fest auf `sandboxTestNow` (2026-09-04 12:00 UTC), überschreibt `nowProvider` aber nicht. Die Laufzeit liest deshalb die Wanduhr: Am 2026-09-05, als die Referenzbilder entstanden, stand dort eine zweistellige Stundenzahl; nach 100 Stunden, am 2026-09-08 16:00 UTC, wurde sie dreistellig. Die CI war auf `2ef3a24` noch grün und wird aus demselben Grund rot, sobald ihr Lauf die Grenze überschreitet.

### Ziel
Die Goldens des Sandbox-Bildschirms zeigen an jedem Tag und auf jedem Rechner dasselbe Bild.

### Nicht-Ziel
Eine Toleranz für Golden-Vergleiche: Sie verdeckte genau diese Sorte Fehler. Eine Änderung an `_Uptime` oder an `nowProvider`; beide tun, was sie sollen.

### Betroffene Pfade
- `app/test/features/sandbox/harness.dart` (die stehende Uhr in `sandboxUnderTest`)
- `app/test/harness/fixed_now.dart` (neu: `FixedNow`, bisher nur in den Intercept-Fixtures)
- `app/test/features/intercept/fixtures.dart` (exportiert `FixedNow` weiter)
- `app/test/goldens/goldens/ci/isolation_panel_*.png`, `sandbox_header_running_*.png` (neu abgenommen)

### Spezifikation
`sandboxUnderTest` überschreibt `nowProvider` mit `FixedNow(sandboxTestNow + sandboxTestUptime)`, vor den Überschreibungen des Aufrufers, damit ein Test die Uhr weiter selbst stellen kann. `sandboxTestUptime` ist 12 Minuten 5 Sekunden, gezeichnet als `12:05`. `FixedNow` zieht in das geteilte Gerüst; die Intercept-Fixtures exportieren es weiter, damit kein Aufrufer umziehen muss.

### Akzeptanzkriterien
- [x] Die acht Goldens sind mit der stehenden Uhr neu abgenommen, und kein anderes Referenzbild hat sich geändert. **Gemessen am 2026-09-11**: `git status` zeigt genau die acht PNGs; im neuen Bild ist allein der Laufzeitblock der Fußzeile kürzer.
- [x] Ohne die Überschreibung der Uhr im Gerüst werden die Goldens rot (Mutationsprobe). **Gemessen am 2026-09-11**: sechs der sechs Isolation-Goldens rot.
- [x] `flutter analyze`, `dart format` und die Tests unter `test/features/sandbox`, `test/features/intercept` und `test/goldens` sind grün. **Gemessen am 2026-09-11**: keine Befunde, 0 Dateien umformatiert, 438 Tests grün.
- [x] `tools/verify-commit.sh` ist über den fertigen Commit grün. **Gemessen am 2026-09-11** über `24af9af` (enthält `079cc5b`), alle acht Schritte; CI-Lauf 34585656949 ebenfalls grün.

### Fallstricke
- Ein neu abgenommenes Referenzbild beweist nichts, solange es aus der Wanduhr entsteht: Erst die stehende Uhr, dann das Bild.
- Die Goldens laufen im CI-Modus von alchemist (Text als Blöcke). Ein hier abgenommenes Bild gilt deshalb auch in der CI.

### Referenzen
Differenzbilder vom 2026-09-11 (`isolation_panel_passed_light_maskedDiff.png`, Fußzeile x 322 bis 332); `app/lib/features/sandbox/sandbox_screen.dart:367-386`; `app/test/features/sandbox/harness.dart:21`, `:103`; Commit `2f2baa7` (Referenzbilder vom 2026-09-05).

---

## HUM-145 · Die Kopplung an einen Adapter wächst nicht weiter
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-074 · Blockiert: nichts; es hält offen, dass ein zweites Sandbox-Backend oder eine andere Proxy-Engine später möglich bleibt

### Kontext
ADR-015 legt Sandbox und Proxy als Adapter an den äußeren Ring. Gemessen am 2026-09-11 hält sich der Code nur zum Teil daran:

- Der Name `bwrap` steht 156-mal in 34 Dateien außerhalb von `daemon/crates/sandbox` und `daemon/bin/humanitl-shim`, Bezeichner wie `BwrapBackend` eingerechnet: in Diagnose-Codes (`core-types/src/diagnostics/codes.rs`), in der Ausgabe der Kommandozeile (`render.rs`), in der Proto, im Daemon (`ipc/src/sandbox.rs`), in der Dart-Domäne und im Fake-Client. Daemon und Kommandozeile halten `BwrapBackend` als konkreten Typ; es gibt kein `dyn SandboxBackend`. `LaunchPlan.argv` ist die wörtliche bwrap-Kommandozeile, und die Mounttabelle der Oberfläche entsteht aus deren Flags. Der Modulkommentar von `launcher.rs` behauptete bis heute, ein zweites Backend berühre nichts außerhalb der Crate; das war falsch und ist mit diesem Issue korrigiert.
- `humanitl_proxy` wird außerhalb des Proxy-Crates an 101 Stellen in 18 Dateien benutzt, vor allem von `humanitld/src/main.rs` und `ipc/src/server.rs`. Das Crate mischt die Protokoll-Engine (hyper, tokio-rustls, rcgen) mit Anwendungslogik (Halte-Warteschlange, Registry, Regelspeicher); es gibt keinen Port, hinter dem die Engine austauschbar wäre.
- Engine-Typen außerhalb des Proxy-Crates: keine. Das ist gut und soll so bleiben.

Ein zweites Sandbox-Backend -- microsandbox als microVM, Docker aus M6 -- oder eine etablierte Proxy-Engine muss heute an all diesen Stellen ansetzen. Das wird hier nicht zurückgebaut. Es soll aber ab jetzt nicht mehr wachsen.

### Ziel
`make deps-lint` und damit der CI-Job `deps-lint` werden rot, sobald eine dieser Kopplungen in einer Datei zunimmt oder eine neue Datei sie aufnimmt.

### Nicht-Ziel
Die bestehende Kopplung abbauen; das geschieht in eigenen Issues, wenn ein zweiter Adapter wirklich kommt. Ein neuer Port ohne ADR und zweiten Adapter (`CLAUDE.md`, „Was wir nicht tun"). Ein Verbot, das Wort `bwrap` in Dokumenten zu schreiben: gezählt werden nur Quelltexte (`.rs`, `.dart`, `.proto`).

### Betroffene Pfade
- `tools/check_coupling.py` (die Zählung)
- `tools/coupling-baseline.toml` (die Grundlinie)
- `tools/tests/check_coupling_test.py` (die Selbstprüfung)
- `Makefile` (`deps-lint`), `CLAUDE.md` (die Regel)
- `daemon/crates/sandbox/src/launcher.rs` (der Modulkommentar, der die Kopplung verneinte)

### Spezifikation
Vier Regeln, je Datei gezählt, außerhalb des jeweiligen Adapters:

| Regel | Was zählt | Zuhause |
|---|---|---|
| `bwrap` | die Zeichenfolge, in jeder Schreibweise, auch in Bezeichnern (`BwrapBackend`, `bwrap_args`) und Kommentaren | Sandbox-Crate, Shim |
| `proxy_engine` | Pfade in `hyper`, `hyper_util`, `http_body_util`, `rustls`, `tokio_rustls`, `rcgen`, `webpki_roots` | Proxy-Crate |
| `proxy_crate` | jedes Element von `humanitl_proxy`, in einer `use`-Liste jedes einzeln | Proxy-Crate |
| `proxy_alias` | `use humanitl_proxy as …`, `use humanitl_proxy;`, `extern crate` | Proxy-Crate |

Die beiden Proxy-Regeln überspringen Kommentarzeilen, weil ein Doc-Kommentar, der auf `humanitl_proxy::ca::ENV_KIT` zeigt, von nichts abhängt. Die `bwrap`-Regel zählt sie mit, weil Prosa über ein bestimmtes Backend genau das Wissen ist, das ein zweites Backend später suchen muss.

Gelesen werden die Dateien aus `git ls-files --cached --others --exclude-standard`, also auch neue, noch nicht hinzugefügte; ohne Git läuft ein Verzeichnisdurchgang ohne `target`, `build`, `.dart_tool` und `generated`. Steigt eine Zahl, ist der Lauf rot und nennt Datei, alten und neuen Wert und den Grund der Regel. Sinkt eine, ist er ebenfalls rot, bis `python3 tools/check_coupling.py --update` die Grundlinie im selben Commit nachzieht -- sonst ließe sich der gewonnene Spielraum unbemerkt wieder verbrauchen.

### Tests
`tools/tests/check_coupling_test.py` baut Wegwerf-Bäume und prüft: Zählung nur außerhalb des Adapters, jede Schreibweise, auch in Bezeichnern; `use`-Listen über mehrere Zeilen und verschachtelt; Kommentare zählen für die Proxy-Regeln nicht, für `bwrap` schon; Aliase; unverändert grün, Anstieg rot, neue Datei rot, Abstieg rot bis `--update`.

### Akzeptanzkriterien
- [x] `make deps-lint` ist auf dem heutigen Stand grün. **Gemessen am 2026-09-11.**
- [x] Eine zusätzliche Erwähnung von `bwrap` in einer Datei der Anwendung macht `make deps-lint` rot, ebenso eine neue, noch nicht hinzugefügte Datei mit dem Wort. **Gemessen am 2026-09-11** am echten Baum (`app/lib/core/domain/sandbox.dart` 2 auf 3, neue Datei 0 auf 1, `BwrapBackend` in `ipc/src/rules.rs`: jeweils Exit 1).
- [x] Ein zusätzliches `use humanitl_proxy::…`, ein Alias und ein `hyper::`-Pfad außerhalb des Proxy-Crates machen es rot. **Gemessen am 2026-09-11**, dazu `{self as p}` und `/* note */ use humanitl_proxy::X;`: jeweils Exit 1.
- [x] Dieselben Namen in einer Kommentarzeile (Proxy) oder im Sandbox-Crate (`bwrap`) lassen es grün. **Gemessen am 2026-09-11**: Exit 0.
- [x] Der CI-Job `deps-lint` ist über den fertigen Commit grün. **Gemessen am 2026-09-11**: CI-Lauf 34585656949 über `24af9af`, alle Jobs grün.

### Fallstricke
- Die Zählung ist textuell, kein Parser. Ein Re-Export (`pub use humanitl_proxy::X` in einem anderen Crate und danach `other::X`) oder ein Makro umgeht sie. Das fängt kein Skript, sondern das Review; die Regel in `CLAUDE.md` nennt deshalb das Ziel und nicht nur die Zahl.
- Wer eine Zahl senkt, muss die Grundlinie mitändern. Das ist lästig und gewollt.

### Referenzen
Messung am 2026-09-11 (`tools/coupling-baseline.toml`); `docs/ARCHITECTURE.md` zu ADR-015; `BACKLOG.md` Abschnitt 6 (Sandbox-Backends) und Abschnitt 9 (M6 Docker, M13 microVM); microsandbox: https://github.com/superradcompany/microsandbox.

---

## HUM-143 · Zwischen zwei Segmenten verschwindet der Klick
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-028 · Blockiert: nichts, aber es ist genau die Sorte Fehler, die `docs/UX.md` 5.3 verbietet

### Kontext
Im Review der Trefferflächen am 2026-09-07 hat der Ersatz-Reviewer nicht nur die Rechtecke gemessen, sondern auch den Raum dazwischen -- und dort ist ein Loch.

`FocusRing` legt seine Ringreserve (`EdgeInsets.all(2)`, `HFocusRing.width = 2`) **außerhalb** des `GestureDetector` (`app/lib/core/ui/focus_ring.dart:71`). Zwei benachbarte Segmente des Remember-Rasters stehen deshalb 4 px auseinander, obwohl sie im Bild aneinanderstoßen: gemessen `Once` von 288,5 bis 352,5 und `Session` ab 356,5. Drei Taps dazwischen (`x = 353,0`, `354,5`, `356,0`) erreichen niemanden -- der Klick verschwindet lautlos, mitten in einem sichtbar zusammenhängenden Bedienelement. Oben und unten gilt dasselbe mit je 2 px.

Das ist genau der Fall, den `docs/UX.md` 5.3 ausschließt („nie Stille") und den der Kommentar in `remember_grid.dart:132-134` selbst beschreibt. Der neue Test aus HUM-028 sieht ihn nicht: Er misst die Größe der Ziele, nicht die Lücken.

### Ziel
Zwischen zwei Segmenten derselben Gruppe gibt es keinen Punkt, an dem ein Klick nichts tut. Was aussieht wie eine zusammenhängende Fläche, ist eine.

### Nicht-Ziel
Den Fokusring abschaffen oder schmaler machen; er gehört zur Tastaturbedienung (`docs/UX.md` 5.1). Die Segmente optisch auseinanderrücken, damit die Lücke „ehrlich" wird -- das Bild ist richtig, die Trefferfläche ist es nicht.

### Betroffene Pfade
- `app/lib/core/ui/focus_ring.dart` (die Verschachtelung)
- `app/lib/features/intercept/widgets/remember_grid.dart` (der Aufrufer, um den es zuerst geht)
- `app/test/features/intercept/hit_targets_test.dart` (die Messung, die fehlt)
- Goldens, falls die Reserve dabei ihren Platz wechselt

### Spezifikation
Der Vorschlag aus dem Review: die Verschachtelung tauschen, also `GestureDetector(behavior: opaque, child: FocusRing(...))` statt `FocusRing(child: … GestureDetector)`. Dann gehört die Reserve zum Ziel, und die Gruppe hat keine Löcher mehr. Zu prüfen ist, was das für die übrigen Aufrufer von `FocusRing` bedeutet -- es gibt mehrere -- und ob ein Golden sich dabei ändert; wenn ja, gehört die Änderung erklärt und nicht nur abgenommen.

### Tests
- Ein Test, der zwischen zwei Segmente tippt (`Offset` genau in die Lücke) und erwartet, dass eines der beiden antwortet. Mutationsprobe: die Verschachtelung zurückdrehen, Test rot.
- Dieselbe Frage für den senkrechten Rand.

### Akzeptanzkriterien
- [x] Ein Tipp zwischen zwei Segmenten erreicht ein Segment. **Gemessen am 2026-09-13** in `app/test/features/intercept/hit_targets_test.dart`, drei neue Messungen. Waagerecht: Die gemalten Kästen von `Once` (288,5 bis 352,5) und `Session` (ab 356,5) stehen unverändert 4 px auseinander, die Trefferflächen stoßen jetzt bei 354,5 aneinander (286,5 bis 354,5 und 354,5 bis 459,5), und die drei Taps des Reviews vom 2026-09-07 (x = 353,0, 354,5 und 356,0 bei y = 280,0) beantwortet `Once` oder `Session`. Senkrecht: Die Trefferfläche von `Once` reicht von y = 264,0 bis 296,0 statt von 266,0 bis 294,0, Taps bei y = 265,5 und 294,5 erreichen das Segment. Bricht die Gruppe bei 140 px Breite in vier Zeilen um, liegen zwischen zwei Zeilen dieselben 4 px (226,0 bis 230,0), und die Taps bei y = 226,5, 228,0 und 229,5 (x = 364,0) erreichen ein Segment. Jede der drei Messungen hält vorher fest, dass zwischen den gemalten Kästen überhaupt Raum liegt; ohne diese Zusicherung tippte sie in ein Segment und bliebe immer grün. Mutationsprobe: die alte Verschachtelung zurück (`FocusRing` um den `GestureDetector`), alle drei rot, etwa „ein Tap bei (364.0, 226.5), zwischen den Zeilen (226.0 bis 230.0), erreicht kein Segment"; die fünf älteren Messungen derselben Datei bleiben dabei grün, weil sie die Segmente messen und nicht die Lücke.
- [x] Die Fokusringe sehen aus wie vorher (Goldens, oder eine erklärte Abweichung). **Gemessen am 2026-09-13**: Getauscht ist nur die Reihenfolge von Detektor und Ring, nicht die Geometrie; die gemalten Kästen liegen nach der Änderung dort, wo der Review sie am 2026-09-07 gemessen hat (`Once` von 288,5 bis 352,5, `Session` ab 356,5), und `flutter test` bleibt mit allen Golden-Tests grün, ohne dass ein Golden neu geschrieben wurde. Die übrigen vier Aufrufer von `FocusRing` bleiben unberührt: `history_table.dart` hat den Detektor schon außen, und bei `release_valve.dart`, `block_button.dart` und `note_field.dart` bleibt die Reserve außerhalb des Ziels, weil dort zwei Ziele absichtlich Abstand halten und jeder gewonnene Pixel ein Pixel näher an der unumkehrbaren Handlung läge (`docs/UX.md` 5.4).
- [x] `make check` grün. **Gemessen am 2026-09-13** über die CI, nicht über den Arbeitsbaum: der Merge `5714119` steckt in den Läufen über `490bf01` und `98984fc`, beide mit allen elf Jobs grün (`rust-check`, `rust-test`, `deps-lint`, `escape-tests`, `goldens`, `e2e`, `e2e-xvfb`, `e2e-agent`, `parity-check`, `proto-lint-and-gen`, `flutter-analyze-test`). Der eigene Lauf über `5714119` war `startup_failure` mit null Jobs, ein Fehler auf Seiten von GitHub, kein Befund am Stand.

### Fallstricke
- `FocusRing` wird an mehreren Stellen benutzt; wer die Verschachtelung tauscht, ändert überall die Trefferfläche -- das ist erwünscht, muss aber überall stimmen, auch dort, wo zwei Ziele heute absichtlich Abstand haben.

### Referenzen
Review zu HUM-028 am 2026-09-07 (drei gemessene Taps in die Lücke); `docs/UX.md` 5.3; `app/lib/core/ui/focus_ring.dart:71`.

---

## HUM-142 · Der Daemon wartet ohne Frist auf einen Agenten, der nicht gehen will
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-011, HUM-042 · Blockiert: jeden Abschied, an dem ein Vollbild-TUI beteiligt ist

### Kontext
Am 2026-09-07 blieb der M3-Lauf zweimal unabhängig an derselben Zeile stehen -- einmal beim Implementierer, einmal im Review --, und beide Male war die Ursache dieselbe: Die Sitzung mit dem echten OpenCode-TUI endete nicht.

Der Ablauf, gemessen: `humanitl run` läuft in eine Zeitschranke und schickt sein `Sandbox(Stop)`; im Protokoll steht `[humanitl] sandbox stopping` und `[humanitl] stopping the session`. Der Daemon nimmt `SIGTERM`, schreibt `recording flushed` -- und hängt. Das bwrap-Kind lebte danach noch 8,5 Minuten weiter, im zweiten Fall über 19 Minuten, bis jemand es von Hand erschlug. Ein kopfloser Agent (`opencode run …`) stirbt an `SIGTERM`; ein Vollbild-TUI fängt das Signal selbst ab, und `--die-with-parent` hält das Kind dann an einem Daemon fest, der auf genau dieses Kind wartet.

`SandboxHandle::terminate` eskaliert korrekt (`SIGTERM`, nach `KILL_GRACE` `SIGKILL`, `daemon/crates/sandbox/src/handle.rs`). Der Weg, den der Abschied des Daemons nimmt, kommt dort offenbar nicht an -- sonst wäre das Kind nach fünf Sekunden weg.

Für einen Menschen heißt das: `humanitl` beenden und der Daemon bleibt stehen, mit einer Sandbox, die weiterläuft. Das ist die unangenehmste Sorte Fehler, weil sie erst auffällt, wenn man das Werkzeug wieder benutzen will (`CLI_005`: „Es läuft schon eine Sitzung").

### Ziel
Kein Abschied ohne Frist. Ein `Sandbox(Stop)` und das Ende des Daemons beenden die Sandbox in beschränkter Zeit, auch wenn der Agent das Signal ignoriert; danach ist kein bwrap-Kind mehr übrig, und ein neuer Start bekommt seine Sitzung.

### Nicht-Ziel
Den Agenten härter anfassen als nötig: Erst `SIGTERM` und die Frist, die er heute hat, dann `SIGKILL`. Die Frist selbst zu verkürzen -- fünf Sekunden sind richtig für einen Agenten, der aufräumt.

### Betroffene Pfade
- `daemon/crates/ipc/src/sandbox.rs` (der Weg von `Stop` zum Handle und das Ende der begleitenden Aufgaben)
- `daemon/bin/humanitld/src/main.rs` (der Abschied auf `SIGTERM`)
- `daemon/crates/sandbox/src/handle.rs` (`terminate`, `KILL_GRACE`)

### Spezifikation
Zu klären ist zuerst, wo der Abschied hängt: an `wait_exit` ohne Frist, an einer Aufgabe, die den `SandboxHandle` festhält, oder am Einsammeln des Kindes. Danach gilt: Jeder Weg, der eine Sandbox beendet, geht über `terminate(KILL_GRACE)`, und der Daemon wartet auf seine Aufgaben mit einer Frist, nach der er sie abbricht.

### Tests
- Ein Integrationstest mit einem Agenten, der `SIGTERM` ignoriert (`trap '' TERM; while :; do sleep 1; done`): `Stop` beendet ihn trotzdem, gemessen an seiner PID, in weniger als `KILL_GRACE` plus einer Sekunde. Mutationsprobe: die Eskalation entfernen, Test rot (er läuft dann in seine Frist).
- Derselbe Agent, und statt `Stop` bekommt der Daemon `SIGTERM`: Auch dann ist das Kind weg, bevor der Test seine Frist erreicht.

### Akzeptanzkriterien
- [x] Ein Agent, der `SIGTERM` ignoriert, ist nach `Stop` in beschränkter Zeit beendet. **Gemessen am 2026-09-13, und zwar an zwei Stellen, weil eine davon nicht alles zeigen kann.** Ende zu Ende: `a_stop_ends_an_agent_that_ignores_sigterm_even_without_a_listener` in `daemon/bin/humanitld/tests/daemon_end_to_end.rs` startet über den echten Daemon eine Sandbox mit `trap '' TERM; while :; do sleep 1; done` und lässt nach `Sandbox(Stop)` den Ereignisstrom sofort fallen — genau der Fall, an dem der M3-Lauf hing. Nach **24,8 bis 26,1 ms** (drei Läufe) trägt kein Prozess mehr das Skript dieses Laufs, und die PID des Sandbox-Prozesses ist weg; die Frist des Tests ist `KILL_GRACE` + 1 s = 6 s. **Was dieser Test nicht misst:** die Eskalation auf `SIGKILL`. `terminate` schickt sein `SIGTERM` an den Sandbox-Prozess auf dem Wirt, nicht an den Agenten darin; der Prozess hat dafür keinen Handler, endet, und mit ihm endet der PID-Namensraum. Der Trap des Agenten hält den Abschied also nicht auf, und ein `terminate` ohne `SIGKILL` ließe diesen Test grün. Die Eskalation misst deshalb `a_child_that_ignores_sigterm_is_killed_after_the_grace` in `daemon/crates/sandbox/src/handle.rs` an einem Kind ohne Sandbox dazwischen: ohne den `SIGKILL` liefert `terminate` `Stuck` statt `Kill`, nach 5,30 s statt 0,30 s; ohne die Gnadenfrist ist es umgekehrt `Kill` statt `Term` nach 618 µs. Mutationsproben am Ende-zu-Ende-Test: ohne das Beenden im Stopp rot (nach 6 s vier Prozesse des Laufs), und mit dem Rückweg von vor diesem Issue (`return`, sobald `tx.send` scheitert) ebenso rot. Befund von Antigravity, dass die frühere Fassung dieses Kastens mehr behauptete als sie maß; übernommen.
- [x] Dasselbe gilt für das Ende des Daemons. **Gemessen am 2026-09-13**: `the_end_of_the_daemon_ends_the_agent_it_started` schickt dem Daemon `SIGTERM`, während dieselbe Sandbox läuft. Sein Prozess ist nach **5,03 bis 5,05 s** weg (Frist des Tests 20 s), im Protokoll steht das Ende der Sandbox und **kein** `DAEMON_009`, und kein Prozess des Laufs ist übrig. Die fünf Sekunden sind der Ausklang des gRPC-Dienstes (`humanitl_ipc::SHUTDOWN_GRACE`), und der Abschied der Sandbox läuft seit dem Befund von Antigravity **daneben** statt danach: Die Sandbox endet **5004 ms bevor** der Dienst seine letzten Clients verabschiedet hat, gemessen an den Zeitstempeln beider Protokollzeilen. Nacheinander wären es im schlimmsten Fall 5 + 11 + 5 = 21 s gewesen, nebeneinander sind es 16 s. Mutationsproben: ohne `farewell_sandbox` rot, mit `DAEMON_009` im Protokoll — das blockierende `wait` auf den Sandbox-Prozess hing dann noch; mit dem Abschied wieder hinter dem Ausklang rot an der Reihenfolge (Sandbox bei Byte 3328, Dienst bei 3180); mit beiden Rückbauten zusammen (kein Abschied, keine Frist für die Aufgaben) endet der Daemon überhaupt nicht, rot nach 20 s — der Hänger vom 2026-09-07. Die Frist für die Aufgaben misst `a_blocking_task_without_an_end_does_not_hold_the_daemon` in `daemon/bin/humanitld/src/main.rs`: ohne sie 10,0 s statt 0,2 s, und ein verschluckter Befund macht sie ebenso rot.
- [x] Der M3-Lauf braucht die Hilfskonstruktion `m3_end_sandbox_from` nicht mehr; sie verschwindet mit diesem Issue aus `tests/e2e/m3_agent_inside/run.sh`. **Entfernt am 2026-09-13**: die Funktion (Zeilen 463-485), ihre drei Aufrufe (1403, 1530, 1592) und der Kommentar, der sagte, warum der Lauf selbst aufräumt; an seiner Stelle stehen fünf Zeilen, die sagen, dass der Daemon das seit HUM-142 selbst tut. `sh -n` über die Datei ist grün, und `m3_end_sandbox_from` kommt darin nur noch als Prosa in dem Kommentar vor, der sagt, warum sie weg ist (`grep -c` zählt 1, Zeile 464). **Nicht neu gemessen ist der M3-Lauf selbst**: Er braucht das echte OpenCode, das Mock-Modell und mehrere Minuten, und diese Sitzung hat ihn nicht gefahren. Dass er die Hilfskonstruktion nicht mehr braucht, steht deshalb auf den beiden Tests oben — `Stop` ohne Zuhörer und das Ende des Daemons, beide gegen den echten Daemon und beide unter ihrer Mutation rot — und nicht auf einem frischen M3-Durchlauf. Wer den nächsten M3-Lauf fährt, sieht es zuerst. **Nachgemessen am 2026-09-18**: Der CI-Job `e2e-agent` über `22ace03` fährt den M3-Lauf mit dem Skript ohne `m3_end_sandbox_from` und ist grün; der Lauf endet also, ohne dass das Skript die Sandbox selbst abräumt.
- [x] `make check` grün. **Gemessen am 2026-09-18**: `tools/verify-commit.sh` über `dac5459`, das den Merge `434a081` enthält, ist grün, einschließlich `cargo deny` nach dem Sicherheitsfix für `rustls` (RUSTSEC-2026-0285), der ohne jeden Bezug zu diesem Issue jeden Commit rot gemeldet hatte; die CI über `22ace03` ist mit allen elf Jobs grün, darunter `e2e-agent` (der M3-Lauf, der die entfernte Hilfskonstruktion `m3_end_sandbox_from` früher brauchte) und `escape-tests`.

**Zwei Dinge, die dieses Issue mitgenommen hat, und eines, das es offen lässt.** Mitgenommen: Ein Daemon, der sich verabschiedet, nimmt keinen `Start` mehr an (`IPC_006` statt `CLI_005`), damit keine Sandbox in das Fenster zwischen Abschied und Prozessende hineinstartet; und `SANDBOX_029` behauptet nicht mehr, ein Prozess habe überlebt, wenn nur sein Exit-Status ausblieb — der Status kommt von dem Faden, der die Sandbox gestartet hat, und der sammelt zuerst die Leser der Ausgabe ein, also wird `/proc/<pid>/stat` gefragt, bevor die Stufe `Blocking` vergeben wird. Offen: Die Zusammenfassung eines Laufs (`SessionSummary`) entsteht in der begleitenden Aufgabe, und niemand wartet beim Abschied auf sie; ein Lauf, den erst das Ende des Daemons beendet, kann sie verlieren. Das ist keine Verschlechterung — vorher hing der ganze Daemon —, aber es bleibt liegen und gehört in ein eigenes Issue.

**Zwei Grenzen des Ausweises, den `process_alive` prüft.** Er ist die Startzeit aus `/proc/<pid>/stat`, gelesen beim Anlegen des Handles; sie unterscheidet unseren Prozess von einem fremden, der dieselbe Nummer nach einem vollen Umlauf des Zählers bekommen hat. Erstens: Ein Handle, das vor dem Kind entsteht, hat keine Startzeit und damit keinen Ausweis — im Produkt kommt das nicht vor, weil das Backend `SandboxHandle::new` erst nach einem geglückten Start ruft, aber die Grenze steht hier. Zweitens: Eine `stat`-Zeile, die vor Feld 22 abbricht, beantwortet die Frage nicht, und dann antwortet auch `process_alive` nicht (`None`), statt „er lebt" zu sagen. Beide Fälle stehen im Test.

**Nicht gebaut ist die Ende-zu-Ende-Fassung des Befunds zu einem abgestürzten Faden.** Sie bräuchte einen Subscriber, der mitten in `farewell` in Panik gerät, und `farewell` braucht ein echtes `SandboxHandle`, das eine Prüfung im ipc-Crate nicht bauen kann (`Shared` und `SandboxHandle::new` sind `pub(crate)` im Sandbox-Crate). Geprüft ist deshalb die Quelle des Befunds an einem echten `JoinError`; dass `Inner::stop` ihn sendet, steht als dieselbe Form neben den beiden anderen Stellen derselben Datei.

### Fallstricke
- `--die-with-parent` bindet das Kind an den Daemon: Wer den Daemon härter beendet, ohne das Kind zu erschlagen, lässt es verwaist zurück. Die Reihenfolge ist erst das Kind, dann der eigene Abschied.

### Referenzen
Zwei Messungen am 2026-09-07 (Implementierer und Review zu HUM-067); `daemon/crates/sandbox/src/handle.rs` (`terminate`, `KILL_GRACE`); `tests/e2e/m3_agent_inside/run.sh` (`m3_end_sandbox_from`).

---

## HUM-141 · Das Mock-Modell lässt keinen Werkzeugaufruf zu
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-046, HUM-067 · Blockiert: die letzte Hälfte des Akzeptanzkriteriums von HUM-067

### Kontext
Das Akzeptanzkriterium von HUM-067 lautet: „`humanitl run --profile llm-only` startet OpenCode, der Prompt erscheint, `webfetch` liefert dem Agenten `403` aus der Profilregel, Inferenz funktioniert." Drei der vier Teile sind seit dem 2026-09-07 im M3-Lauf gemessen. Der vierte nicht, und der Grund liegt nicht im Werkzeug, sondern im Modell des Laufs.

Das Mock-Modell (`tests/e2e/mock_llm`) antwortet auf jede Frage mit denselben zehn Token (`tok0 … tok9`). Ein Agent bekommt daraus nie einen Werkzeugaufruf; OpenCode ruft `webfetch` deshalb nie auf, und das `403` der Profilregel kann ihn nie erreichen. Gemessen wird es heute nur mit einem Skript-Agenten, der selbst `curl` aufruft (Schritt 12 desselben Laufs) -- das zeigt die Regel, aber nicht den Weg durch den echten Agenten.

Ebenso unerreicht: Was der echte Agent im TUI tut, wenn ein Mensch etwas tippt. Der Lauf kann tippen (`humanitl sandbox attach` reicht die Standardeingabe an das Pseudoterminal weiter, gemessen mit `/bin/sh`), aber OpenCode nimmt die Zeichen in den ersten Sekunden nicht an, und danach hat der Lauf keine Antwort des Modells, auf die es lohnte zu warten.

### Ziel
Das Mock-Modell kann auf Verlangen eine Antwort geben, die einen Werkzeugaufruf enthält -- im Format, das OpenCode über seinen `openai-compatible`-Anbieter erwartet. Damit misst der M3-Lauf die letzte Hälfte des Kriteriums: Der echte Agent ruft `webfetch` auf, die Regel des Profils antwortet mit `403`, und das steht im Transkript des Agenten.

### Nicht-Ziel
Ein Mock, der ein Sprachmodell nachbaut. Er braucht genau zwei Antworten: die zehn Token wie bisher und, wenn der Lauf es verlangt, einen Werkzeugaufruf mit einer URL, die die Regel blockt.

### Betroffene Pfade
- `tests/e2e/mock_llm/` (die Antwort mit `tool_calls`)
- `tests/e2e/m3_agent_inside/run.sh` (Schritt 13: tippen, warten, das `403` im Transkript suchen)
- `backlog/sprint-3.md`, Kriterium von HUM-067

### Spezifikation
Der Mock bekommt einen Schalter (Umgebungsvariable oder ein Pfad in der Anfrage), der die nächste Antwort als Werkzeugaufruf formt: ein `tool_calls`-Eintrag mit dem Namen des Werkzeugs und einem Argument, das eine URL trägt (`https://models.dev/api.json` oder ein anderes Ziel, das die mitgelieferte Regel blockt). Der Lauf tippt dem Agenten eine Frage ein, wartet auf den Aufruf und sucht danach im Transkript nach dem `403`.

Die Eingabe an das TUI ist der zweite Teil: Sie geht über `humanitl sandbox attach`, weil `humanitl run` keine Tasten weiterreicht (`daemon/bin/humanitl/src/cmd/run.rs`, Kopf). Wie lange OpenCode nach seinem ersten Bild braucht, bis es Zeichen annimmt, ist zu messen und als Zahl in den Lauf zu schreiben, nicht zu raten.

### Tests
- Der M3-Lauf misst den neuen Fall; die Zahl der Zusicherungen des OpenCode-Zweigs steigt entsprechend.
- Eine Selbstprüfung des Mocks für die neue Antwortform, wie für die bestehenden.

### Akzeptanzkriterien
- [x] Der Mock kann eine Antwort mit Werkzeugaufruf liefern, und seine Selbstprüfung deckt sie ab. **Gemessen am 2026-09-11**: `tests/e2e/mock_llm/self_test.sh` mit 26 Zusicherungen. Zwölf davon gelten dem Werkzeugaufruf: Rahmen mit `tool_calls`, Name des angebotenen Werkzeugs und URL, Ende als `tool_calls`, keine gewöhnlichen Token daneben, die Form, die ein OpenAI-kompatibler Client liest (`id`, `type`, `index`, `arguments` als JSON-Text mit der URL darin), dieselbe Antwort ohne Strom samt URL, die zehn Token, wenn kein Werkzeug angeboten ist, der Aufruf auch bei einem Marker in Anführungszeichen, kein zweiter Aufruf nach der Antwort des Werkzeugs, die Antwort darauf mit ihrem Inhalt unter `/_debug/tool`, und die zehn Token für die Antwort eines Werkzeugs, das der Mock nicht aufgerufen hat. Eine weitere hält fest, dass der letzte Rahmen des gewöhnlichen Stroms `finish_reason: "stop"` trägt; ohne ihn las OpenCode die Antwort nicht als beendet. Mutationsproben: Ohne die Prüfung der Aufrufkennung wird die letzte der zwölf rot, mit `arguments` als Objekt die Formprüfung und die Antwort ohne Strom. Grundlage im Review: Codex, der Mock beantwortete zuerst jede Werkzeugantwort, und die Selbstprüfung suchte Text, statt die Form zu lesen.
- [x] Der M3-Lauf gibt dem echten Agenten eine Frage und sieht seinen Werkzeugaufruf. **Umformuliert und gemessen am 2026-09-11.** Die frühere Fassung lautete „tippt dem echten Agenten eine Frage ein". Gebaut ist die kopflose Frage über `opencode run`, denn so bekommt OpenCode ohne Menschen eine Frage und ruft das Werkzeug auf. Den Aufruf sieht der Lauf zweimal: Das Mock-Modell hält unter `/_debug/tool` die Antwort des Werkzeugs, die OpenCode ihm zurückgibt, und der Verlauf des Daemons hat genau die eine Anfrage an `humanitl-probe.example`. Das Tippen ins TUI ist nicht gebaut und liegt als HUM-152 in Sprint 5.
- [x] Das `403` der Profilregel steht im Transkript des echten Agenten. **Gemessen am 2026-09-11**: Das Transkript von `opencode run` zeigt `✗ WebFetch https://humanitl-probe.example/page failed` und `Error: StatusCode: non 2xx status code (403 GET https://humanitl-probe.example/page)`. Die Zusicherung liest das Transkript ohne die Zeile, mit der der Mock die Antwort des Werkzeugs wiederholt. Eine weitere hält fest, dass der gescheiterte Aufruf OpenCodes `WebFetch` war und nicht ein anderes Werkzeug, das dieselbe URL abrief (Codex). Zwei weitere halten fest, dass eine Regel entschied und nicht eine Frist: `rule_id` ist gesetzt, und `decision` ist `block`. Der ganze Lauf steht bei `M3 demo: OK` mit 133 Zusicherungen, 22 davon im OpenCode-Zweig. Die Zusicherung über `/_debug/tool` war in den Läufen vor dem ersten grünen rot, solange OpenCode den Aufruf nicht auslöste. Grundlage im Review: Antigravity, denn zuerst las der Lauf das Transkript des Agenten gar nicht.
- [x] Das Kriterium von HUM-067 ist danach vollständig abgehakt. **Am 2026-09-11**, mit dem Hinweis dort auf das abgeleitete Profil und auf HUM-152.

### Fallstricke
- Ein Werkzeugaufruf, den OpenCode nicht versteht, sieht aus wie „keine Antwort"; die Form gehört gegen die Fassung geprüft, die im Lauf steckt (`opencode --version` steht im Transkript).
- `OPENCODE_PERMISSION` steht in der Sandbox auf `{"webfetch":"ask"}`; ohne Menschen antwortet OpenCode darauf mit einer Verweigerung. Der Lauf setzt es für diesen Fall auf `allow`, sonst misst er die Berechtigung des Agenten statt die Regel von Humanitl.

### Referenzen
`backlog/sprint-3.md` HUM-067; `tests/e2e/m3_agent_inside/run.sh` Schritt 12 und 13; Messungen vom 2026-09-07.

---

## HUM-140 · Modell-Endpunkt und Zugangsschlüssel in der Oberfläche
Sprint: 4 · Größe: L · Abhängigkeiten: HUM-062, HUM-069, HUM-039 · Blockiert: jeden Nutzer, dessen Modell nicht ohne Schlüssel antwortet

> **Zusammengelegt mit HUM-069** (zweite Umfangsentscheidung 2026-09-18): Endpunkt, Modell, Schlüssel und „Prüfen" stehen im selben Formular. Der Weg des Schlüssels (Secret Service, `0600`-Rückfall, nie im Klartext in Datei, `argv`, Log oder Aufzeichnung) bleibt vollständig gefordert.

### Kontext
Am 2026-09-07 hat der Nutzer die Oberfläche einem Kollegen gezeigt und dabei gefragt, wo er seinen eigenen Anschluss für das Modell und seinen Zugangsschlüssel hinterlegt. Die ehrliche Antwort war: nirgends.

Was es gibt, steht in `docs/CONFIG.md`: `llm.endpoint` (ein OpenAI-kompatibler Endpunkt, Stufe `basic`), `llm.models` und `llm.passthrough_paths`. Alle drei nur in `config.toml`, von Hand. Was es nicht gibt:

* **Keinen Zugangsschlüssel.** Im Schema kommt keiner vor. Die Annahme war „ein lokales Modell im eigenen Netz braucht keinen"; `LLM_006` warnt sogar, wenn der Endpunkt nicht in einem privaten Netz liegt. Für einen Anbieter mit Schlüssel fehlt damit alles: der Ort, an dem er liegt, der Weg in die Sandbox und die Maskierung überall dort, wo er sonst auftauchte.
* **Keinen Ort in der Oberfläche.** Der Settings-Screen ist HUM-069; bis dahin antwortet `SetConfig` mit `unimplemented`, und die Anwendung kann Konfiguration lesen, aber nichts schreiben.

Die Anforderung des Nutzers, in seinen Worten: „das muss auch bitte sehr einfach dann in der GUI gehen, dass das hier dann einfach funktioniert."

### Ziel
Ein Mensch trägt in der Oberfläche seinen Endpunkt, sein Modell und, wenn nötig, seinen Zugangsschlüssel ein, drückt einmal auf „Prüfen" und sieht, ob der Endpunkt antwortet und welche Modelle er anbietet. Der Schlüssel liegt danach an einem Ort, den weder Log noch Transkript noch Aufzeichnung zeigen, und der Agent bekommt ihn, ohne dass ein Mensch ihn noch einmal sieht.

### Nicht-Ziel
Ein Schlüsselspeicher mit eigener Kryptographie -- CLAUDE.md verbietet das, und der Speicher des Systems (Secret Service, `libsecret`) ist da. Mehrere Anbieter nebeneinander in einer Sitzung; genau einer je Sitzung reicht für M4. Die Frage, ob ein Modell in der Cloud überhaupt erlaubt sein soll -- das entscheidet der Mensch mit derselben Warnung wie heute (`LLM_006`), diese Arbeit macht sie nur sichtbar.

### Betroffene Pfade
- `daemon/crates/config/src/model.rs`: `llm.api_key_ref` (eine Referenz, nicht der Schlüssel selbst)
- `daemon/crates/secrets/` (neu) oder `humanitl-core`: Zugriff auf den Schlüsselspeicher des Systems
- `daemon/crates/proxy/src/llm_probe.rs`: die Probe schickt den Schlüssel mit, wenn einer hinterlegt ist
- `daemon/crates/sandbox/src/agent/opencode.rs`: der Schlüssel geht als Umgebungsvariable in die Sandbox und steht nicht in `argv`
- `app/lib/features/setup/` und der Settings-Screen aus HUM-069
- `docs/CONFIG.md`, `docs/SECURITY.md` (was mit dem Schlüssel geschieht und was nicht)

### Spezifikation
In der Konfiguration steht **nie der Schlüssel**, sondern eine Referenz: `llm.api_key_ref = "secret-service:humanitl/llm"`. Der Daemon holt ihn beim Start der Sitzung und reicht ihn als Umgebungsvariable in die Sandbox; `argv` trägt ihn nicht, das Log trägt ihn nicht, die Aufzeichnung ersetzt ihn durch `••••` wie jedes andere Geheimnis (`humanitl-findings`, Tier 1). Fehlt der Schlüssel im Speicher, ist das ein `Diagnostic` mit `fix`, kein stiller Fehlschlag.

Die Oberfläche bekommt im Setup und in den Einstellungen eine Karte „Modell": Endpunkt, Modell, Schlüssel (Eingabe maskiert, Wert wird nie zurückgelesen, nur „gesetzt" oder „nicht gesetzt"), und den Knopf „Prüfen", der `ProbeLlm` ruft und die Modelliste zeigt. Ein Endpunkt außerhalb des privaten Netzes trägt die Warnung aus `LLM_006` sichtbar neben sich, nicht in einem Log.

### Tests
- Ein Test, der den Schlüssel aus einer Attrappe des Speichers holt und ihn in der Umgebung der Sandbox findet, aber nicht in `argv`, nicht im Log und nicht in der Aufzeichnung. Mutationsprobe: den Schlüssel in `argv` legen, Test rot.
- Ein Widget-Test der Karte: „gesetzt" ohne den Wert zu zeigen; „Prüfen" ruft `ProbeLlm` und zeigt die Modelle.
- Ein Test für den fehlenden Schlüssel: `Diagnostic` mit `fix`.

### Akzeptanzkriterien
- [ ] Endpunkt, Modell und Schlüssel lassen sich in der Oberfläche setzen, ohne eine Datei zu öffnen.
- [ ] Der Schlüssel steht nirgends im Klartext: nicht in `config.toml`, nicht in `argv`, nicht im Log, nicht in der Aufzeichnung.
- [ ] „Prüfen" zeigt die Modelle des Endpunkts oder sagt mit `why` und `fix`, warum nicht.
- [ ] `docs/SECURITY.md` beschreibt den Weg des Schlüssels.
- [ ] `make check` grün.

### Fallstricke
- Ein Schlüssel in einer Umgebungsvariable steht in `/proc/<pid>/environ` des Agenten. Das ist innerhalb der Sandbox und damit für den Agenten ohnehin lesbar -- er braucht ihn ja --, aber er darf nicht in der Momentaufnahme der Oberfläche auftauchen (`EnvEntry.withheld` gibt es dafür schon).
- Der Secret Service ist auf einem Rechner ohne Desktop nicht da. Dann bleibt eine Datei mit `0600` unter `$XDG_DATA_HOME`, und die Oberfläche sagt, welcher der beiden Wege gilt.

### Referenzen
Frage des Nutzers am 2026-09-07 während einer Vorführung; `docs/CONFIG.md` (`llm.*`); HUM-069 (Settings-Screen); HUM-039 (`ProbeLlm`); `LLM_006`.

---

## HUM-139 · Die Vorprüfung sucht den Agenten im PATH des Hosts
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-037, HUM-135 · Blockiert: einen Start, der funktionieren würde

### Kontext
Beim Vorbereiten einer Vorführung am 2026-09-07 verweigerte der Daemon den Start mit einem **blockierenden** Befund, obwohl der Start funktioniert hätte:

```
blocking[AGENT_004]: Agent-Kommando in der Sandbox nicht erreichbar
  why: /home/nburkert/.local/bin/opencode is on this machine, but the sandbox mounts only
       /usr, …, /home/nburkert/.opencode/bin read-only, so the command is not there …
```

Die Lage: Das Profil mountet `/home/nburkert/.opencode/bin` read-only **und** trägt dieses Verzeichnis in `[env].PATH` der Sandbox ein. Ein `exec` von `opencode` in der Sandbox hätte also die gemountete Datei gefunden. Die Vorprüfung in `daemon/crates/sandbox/src/agent/opencode.rs` löst das nackte Kommando aber gegen den **PATH des Hosts** auf (`AgentContext::host_path`), findet dort zuerst `~/.local/bin/opencode` -- ein Wrapper-Skript, das nicht gemountet ist -- und prüft dann, ob **diese** Datei in der Sandbox sichtbar ist. Sie ist es nicht, und der Befund steht.

Der Befund ist also nicht falsch über die Datei, die er nennt, sondern über die Frage, die er beantworten soll: Erreicht die Sandbox das Kommando? Umgangen wurde es für die Vorführung mit einem absoluten Pfad in `agent.command`; das ist die Krücke, nicht die Antwort.

### Ziel
Die Vorprüfung beantwortet die Frage, die sie stellt: Sie löst ein nacktes Kommando gegen den **PATH der Sandbox** auf, beschränkt auf Verzeichnisse, die die Sandbox wirklich sieht. Nur wenn dort nichts liegt, meldet sie `AGENT_004`, und dann nennt sie den PATH der Sandbox statt eines Pfads auf dem Host.

### Nicht-Ziel
Die Prüfung streichen -- sie hat einen echten Fall gefunden (ein Programm unter `$HOME`, das die Sandbox nicht sieht, HUM-135). Den PATH des Hosts ganz ignorieren: Wenn `agent.command` einen absoluten Pfad trägt, bleibt die Frage dieselbe wie heute.

### Betroffene Pfade
- `daemon/crates/sandbox/src/agent/opencode.rs` (die Stelle mit `AGENT_004`)
- `daemon/crates/sandbox/src/agent/mod.rs` (`AgentContext`: der PATH der Sandbox gehört hinein, heute steht dort nur der des Hosts)
- `daemon/crates/sandbox/tests/opencode_adapter.rs`

### Spezifikation
`AgentContext` bekommt neben `host_path` den PATH der Sandbox aus dem Profil (`[env].PATH`) und die Liste der read-only-Mounts, die es schon hat. Für ein nacktes Kommando wird zuerst im Sandbox-PATH gesucht, und zwar nur in Verzeichnissen, die unter einem Mount liegen; ein Treffer dort beendet die Prüfung ohne Befund. Erst wenn dort nichts liegt, gilt der heutige Weg: auf dem Host suchen, und wenn dort etwas liegt, `AGENT_004` mit beiden Angaben -- was auf dem Host liegt und welcher PATH in der Sandbox gilt.

### Tests
- Ein Test, in dem das Kommando nur über den Sandbox-PATH in einem gemounteten Verzeichnis erreichbar ist: kein Befund. Mutationsprobe: die Suche im Sandbox-PATH entfernen, Test rot.
- Der bestehende Fall bleibt: Programm unter `$HOME`, nicht gemountet, kein Eintrag im Sandbox-PATH ⇒ `AGENT_004`.

### Akzeptanzkriterien
- [x] Ein Kommando, das über den Sandbox-PATH in einem gemounteten Verzeichnis liegt, erzeugt keinen Befund. **Gemessen am 2026-09-13** mit `opencode_preflight_accepts_a_command_on_the_sandbox_path` und `opencode_preflight_accepts_a_bare_override_on_the_sandbox_path` (`daemon/crates/sandbox/tests/opencode_adapter.rs`): `preflight` liefert je 0 Befunde, obwohl auf dem Host ein zweites, nicht eingehängtes `opencode` zuerst käme. Mutationsproben: früher Rücksprung in `command_preflight` gestrichen — rot mit `AGENT_004`; die Suche auf das Standardkommando beschränkt, sodass `agent.command` sie nicht mehr benutzt — rot mit `AGENT_002`.
- [x] `AGENT_004` nennt bei einem echten Fehlschlag den PATH der Sandbox. **Gemessen am 2026-09-13** mit `opencode_preflight_reports_a_sandbox_path_entry_without_a_mount`: `why` enthält den vollständigen PATH der Sandbox und den Pfad auf dem Host, `severity` ist `Blocking`, `fix` und `docs` sind gesetzt. Mutationsprobe: den PATH aus `why` gestrichen — rot. `AGENT_001` nennt ihn ebenfalls (`opencode_preflight_missing_binary`, eigene Mutationsprobe).
- [x] `make check` grün. **Gemessen am 2026-09-13**: lokal `cargo build --workspace --all-targets`, `cargo test -p humanitl-sandbox -p humanitl` (356 + 248 Tests, 0 rot), `cargo clippy -p humanitl-sandbox -p humanitl --all-targets -- -D warnings`, `RUSTDOCFLAGS=-D warnings cargo doc -p humanitl-sandbox`, `rustfmt --check` über die sieben geänderten Dateien, `tools/check-deps.sh`, `scripts/ci/lint-docs.sh`, `scripts/ci/lint-no-string-errors.sh` und `tools/check_coupling.py`; der Rest einschließlich Flutter über die CI, Lauf über `98984fc` mit allen elf Jobs grün.

### Wie die Vorprüfung auflöst (Messungen vom 2026-09-13)
Drei Review-Runden haben sich an dieser Stelle widersprochen; entschieden wurde
mit `bwrap`, einem Projektbaum als `/work` und `env` als Testprogramm.

| Messung | Ergebnis |
|---|---|
| `bwrap … --chdir /work … -- /usr/bin/pwd` | `/work` |
| `--setenv PATH bin`, Datei in `<work>/bin/opencode` | startet `/work/bin/opencode` |
| `--setenv PATH :/usr/bin`, Datei in `<work>/opencode` | startet `/work/opencode` |
| `env -i PATH=bin env prog` auf dem Host, `prog` in `./bin` | startet `./bin/prog` (glibc `execvp`) |
| Verweis in der Einhängung auf eine Datei außerhalb | `exec` endet mit 127 |
| Kette Einhängung → außerhalb → Einhängung | `readlink -f` endet drinnen, `exec` endet mit 127 |
| `--symlink usr/bin /bin`, `--setenv PATH /bin` | startet das Programm aus `/usr/bin` |

**Das Modell rechnet in Pfaden der Sandbox** (`SandboxView`, `Mount { src, dst }`).
Ein Verweis trägt den Text seines Ziels, und den liest der Kern drinnen: `/work`
gibt es dort, den Host-Pfad des Projektbaums nicht. Aufgelöst wird Stück für
Stück, `..` wirkt beim Gehen und nicht vorher, und der zurückgegebene Pfad ist
der aufgelöste Ort auf dem Host — nur den kann der Aufrufer selbst anfassen.

### Was das Modell beantwortet und was es offen lässt
Beantwortet, jeweils mit Test und Mutationsprobe:

| Fall | Antwort |
|---|---|
| Nackter Name im Sandbox-PATH unter einer Einhängung | erreichbar, kein Befund |
| Einhängung aus `[mounts].extra_rw` | zählt wie jede andere |
| Verzeichnis eingehängt, aber nicht im Sandbox-PATH | `AGENT_004` mit `sandbox.env.PATH` als Weg hinaus |
| Ort unter einem tmpfs oder einer Maske (`/work/.direnv`, `/work/.envrc`) | nicht erreichbar |
| Profil ohne `[env].PATH` | Vorgabe der C-Bibliothek, gemessen `/bin:/usr/bin` |
| Eintrag, den keine Einhängung deckt | kein Treffer; `AGENT_004` bleibt möglich |
| Relativer oder leerer Eintrag | gegen `[mounts].work.dst` aufgelöst |
| Absoluter Eintrag über einen Verweis des Profils (`/bin`) | erreichbar |
| Relatives Kommando (`./bin/opencode`) | im Projektbaum gesucht |
| Verweis aus der Einhängung heraus, auch über mehrere Schritte | nicht erreichbar |
| Verweis oberhalb einer Einhängung (`/home` auf `/var/home`) | zählt nicht, Einhängung bleibt erreichbar |
| Verweis im Projektbaum auf `/work/…` | erreichbar; auf den Host-Pfad desselben Baums: nicht |
| `..` hinter einem Verweis | wie im Kern, nach dem Verweis |
| Datei ohne Ausführungsrecht | `AGENT_002`, nicht `AGENT_001` |
| `sandbox.env.PATH` | gewinnt über `[env].PATH` des Profils |

Bewusst **stumm**, weil ein Befund einen Beleg braucht
(`backlog/CONVENTIONS.md` 4.13) und ein falscher Block schlimmer ist als ein
später sichtbarer Fehlschlag (HUM-137 meldet ihn mit `126`/`127`):

- Ein Verzeichnis oder ein Verweis, den dieser Prozess nicht lesen darf
  (`Reach::Unknown`); ebenso eine Kette, die den Deckel von 40 Verweisen reißt.
- Ein Suchpfad der Sandbox, den der Aufrufer nicht hereingereicht hat: dann
  gilt die Vorgabe der C-Bibliothek, und was sie nicht findet, geht den alten
  Weg über den Host.
- Was die Sandbox aus Quellen bekommt, die kein Pfad des Hosts sind: der
  Inhalt der tmpfs, die `--ro-bind-data`-Dateien des Adapters, `/proc`, `/dev`,
  der Proxy-Socket, die CA, der Shim. Ein Kommando, das dort läge, meldet die
  Vorprüfung nicht als fehlend, sondern gar nicht — sie sucht nur, was sie
  kennt. Die **Orte** dieser Überdeckungen kennt sie dagegen: Ein tmpfs über
  `/work/.direnv` macht alles darunter unerreichbar, und genau das sagt sie.
- Ein Wechsel des Dateibaums zwischen Vorprüfung und Start. Die Prüfung ist
  eine Aussage über den Augenblick, in dem sie läuft.

### Wer wobei recht hatte
Codex hatte recht mit den relativen und leeren Einträgen, mit der Kette über
zwei Verweise, mit den Verweisen des Profils im Kontext, mit `..` vor der
Auflösung, mit dem wirksamen `PATH` aus `sandbox.env` und mit den
Sandbox-Koordinaten für `/work`. Antigravity hatte recht mit dem Verweis aus
der Einhängung heraus, mit den unkanonisierten Wurzeln (gelöst, indem gar nicht
mehr kanonisiert wird), mit `/bin`, mit der Reihenfolge im Doctor, mit dem
zurückgegebenen Pfad, mit `AGENT_001` statt `AGENT_002` und mit zwei Tests, die
aus einem anderen Grund grün waren als ihrem Namen nach. Nicht übernommen wurde
Antigravitys Schluss, ein unbekannter Host-PATH mache das Fehlen des Kommandos
sicher; beide Reviewer haben dieser Ablehnung danach zugestimmt.

### Offene Punkte

**Der Doctor des Daemons fragt weiter zuerst den Host.** Beim Merge am 2026-09-13 verdrahtet, was ohne Bedeutungsfrage ging: `daemon/crates/ipc/src/sandbox.rs` baut die Sicht jetzt aus dem Profil (`SandboxView::of_profile`), nimmt den wirksamen Suchpfad (`sandbox.env` vor `[env]`) und kennt `[mounts].work.dst`; die Übergangsform `with_sandbox_ro_paths` ist damit ohne Aufrufer und entfernt. Nicht verdrahtet ist `Probe::with_sandbox` im Daemon (`daemon/crates/ipc/src/server.rs`, `DoctorSetup`): `DoctorSetup` trägt kein Profil, und die Profilsuche liegt als Methode an `SandboxService`. Das ist nicht nur eine fehlende Zeile — es müsste entschieden werden, welches Profil der Doctor meint, wenn gerade keine Sitzung läuft. Diese Frage gehört in ein Issue, das sie entscheiden darf, nicht in einen Merge. Bis dahin antwortet der Doctor des Daemons für einen Agenten, der nur über den Suchpfad der Sandbox erreichbar ist, anders als die Vorprüfung; der Doctor der Kommandozeile ist verdrahtet und antwortet richtig.

*Eine Verdrahtung fehlt im Daemon.** `daemon/crates/ipc/src/sandbox.rs` gehört
gerade einem anderen Issue; dort braucht der Aufbau des `AgentContext` neben
`.with_sandbox_ro_paths(…)` dasselbe wie
`daemon/bin/humanitl/src/cmd/sandbox.rs` (`with_sandbox_view`): den wirksamen
Suchpfad (`config.sandbox.env` vor `profile.env`), die Verweise des Profils und
`[mounts].work.dst`.

**Der Doctor des Daemons kennt sein Profil nicht.** `DoctorSetup`
(`daemon/crates/ipc/src/server.rs`) wird aus der Konfiguration gebaut, und die
Suche nach der Profildatei liegt als private Methode in
`daemon/crates/ipc/src/sandbox.rs`. Damit `Probe::with_sandbox` auch dort
gesetzt werden kann, muss diese Suche erreichbar werden; nachbauen wäre eine
zweite Kopie einer Suchreihenfolge, die es nur einmal geben darf. Der lokale
Doctor (`humanitl doctor` ohne Daemon) ist verdrahtet.

**`docs/DIAGNOSTICS.md`** beschreibt `AGENT_004` noch host-seitig; die Datei
gehört gerade einem anderen Issue. Der Satz, der dort hingehört, steht im
Bericht zu diesem Issue.

**`AGENT_002` aus der Sandbox-Suche bleibt eine Warnung**, obwohl der
Fehlschlag feststeht (die Datei liegt im Suchpfad, ist nicht ausführbar, und
`exec` endet mit 126). Zwei Gründe: Der Registereintrag in
`daemon/crates/core-types/src/diagnostics/codes.rs` sagt „die Sandbox startet
trotzdem", und die Sitzung ist auch ohne Agenten gültig — ein Mensch startet
darin etwas anderes (Nicht-Ziel von HUM-137). Den Exit-Code meldet HUM-137 mit
einem eigenen Befund. Wer das ändert, ändert den Registereintrag mit.

### Fallstricke
- Der PATH der Sandbox kann Verzeichnisse nennen, die kein Mount abdeckt; die dürfen nicht als Treffer zählen, sonst verschwindet der Befund, den HUM-135 gebaut hat.

### Referenzen
Beobachtung am 2026-09-07 beim Aufsetzen der Vorführung; `daemon/crates/sandbox/src/agent/opencode.rs` (`AGENT_004`); HUM-135.

---

## HUM-138 · Geblockter Verkehr außerhalb von HTTP ist unsichtbar
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-002, HUM-021 · Blockiert: die Zusage „du siehst, was dein Agent tut"

### Kontext
Ein Mensch hat am 2026-09-07 gefragt, was passiert, wenn der Agent eine SSH-Verbindung aufbaut. Die Antwort in zwei Teilen, und der zweite ist der Mangel.

**Geblockt wird zuverlässig.** `ssh github.com` scheitert schon an der Namensauflösung: Der seccomp-Filter des Shims erlaubt `socket()` nur für `AF_INET`/`AF_INET6` mit `SOCK_STREAM`, und DNS ist `SOCK_DGRAM` (`EPERM`). Mit einer nackten IP kommt der Agent bis `connect`, und dort endet es mit `ENETUNREACH`: Im Netz-Namensraum existiert nur `lo`, die Routing-Tabelle ist leer. Beides ist gemessen (`tests/escape/`, ESC-3, und die Zeilen in BACKLOG.md 3).

**Berichtet wird nichts.** Der Proxy sieht diese Verbindung nie, also entsteht kein Fluss, keine Zeile in der Historie, kein Eintrag in der Warteschlange und keine Meldung im Fenster. Der Agent liest seinen eigenen Fehler, der Mensch sieht eine ruhige Oberfläche. Dasselbe gilt für jeden Versuch außerhalb von HTTP: ein eigener TCP-Port, ein UDP-Paket, ein Unix-Socket nach draußen, `git://`.

Für ein Werkzeug, dessen Versprechen „du siehst, was dein Agent tut" lautet, ist die stille Blockade die halbe Antwort. Wer den Agenten beobachtet, um ihm zu vertrauen, muss auch sehen, was er **versucht** hat.

### Ziel
Ein Versuch des Agenten, an der einen Tür vorbei ins Netz zu gehen, erscheint in demselben Strom, den die Oberfläche ohnehin liest: mit Zeitpunkt, Ziel (soweit bekannt), Grund der Verweigerung und der Zahl der Wiederholungen. Die Historie zeigt ihn als eigene Art von Eintrag, nicht als Fluss.

### Nicht-Ziel
Den Verkehr durchlassen oder einen zweiten Weg nach draußen bauen. Jeden verweigerten Syscall einzeln melden -- ein Agent in einer Schleife erzeugte damit eine Flut; gezählt und zusammengefasst wird, nicht protokolliert. Die Zusicherung der drei Garantien ändert sich nicht.

### Betroffene Pfade
- `daemon/crates/sandbox/src/shim/` beziehungsweise das Shim-Binary: der seccomp-Filter und was er meldet
- `daemon/crates/core-types/src/`: eine Ereignisart für „Versuch verweigert"
- `daemon/crates/ipc/src/sandbox.rs`: der Weg in den `Sandbox`-Strom
- `app/lib/features/`: die Zeile, die es zeigt
- `docs/SECURITY.md`, `docs/THREAT-MODEL.md`: die Aussage, dass Blockaden jetzt auch sichtbar sind

### Spezifikation
Der Shim hält den Filter ohnehin; zwei Wege sind zu prüfen und einer zu wählen. Entweder `SECCOMP_RET_USER_NOTIF` statt `SECCOMP_RET_ERRNO` für die verweigerten Familien -- dann kennt der Shim jeden Versuch mit seinen Argumenten und kann ihn melden, bevor er `EPERM` zurückgibt -- oder ein Zähler je Familie und Typ im Shim, der beim Ende der Sitzung und alle `n` Sekunden gemeldet wird. Der erste Weg ist genauer und teurer, der zweite billig und gröber; die Entscheidung gehört in den ADR-Teil dieses Issues.

Ein `connect`, das an `ENETUNREACH` scheitert, sieht der Filter nicht (es ist kein verweigerter Syscall). Für diesen Fall bleibt nur der Weg über das Kind selbst: `strace` kommt nicht in Frage (`ptrace` ist verboten, und das bleibt so). Die ehrliche Fassung des Ziels ist deshalb: gemeldet wird, was der Filter verweigert, und die Netzlosigkeit steht als Zustand daneben -- nicht als Ereignis je Versuch.

### Tests
- Ein Escape-Fall in `tests/escape/`, der aus der Sandbox `socket(AF_UNIX)` und `socket(AF_INET, SOCK_DGRAM)` versucht und danach den Bericht erwartet.
- Ein Test im `Sandbox`-Strom, der den Bericht als Ereignis sieht.
- Ein Widget-Test für die Zeile.

### Akzeptanzkriterien
- [ ] Ein verweigerter `socket()`-Aufruf des Agenten erscheint im `Sandbox`-Strom, zusammengefasst und mit Zahl.
- [ ] Die Oberfläche zeigt ihn, ohne die Warteschlange der Flüsse zu berühren.
- [ ] Die drei Garantien bleiben gemessen grün (`tests/escape/`).
- [ ] `docs/SECURITY.md` sagt, was sichtbar wird und was nicht.
- [ ] `make check` grün.

### Fallstricke
- `SECCOMP_RET_USER_NOTIF` hält den Aufruf an, bis jemand antwortet: Wer den Shim damit verzögert, verlangsamt jeden Aufruf des Agenten. Antwortzeit messen, sonst wird aus einer Meldung eine Bremse.
- Ein Agent in einer Wiederholungsschleife erzeugt tausende Versuche je Sekunde. Ohne Zusammenfassung ist das eine Flut, die nichts zeigt.

### Referenzen
Frage des Nutzers am 2026-09-07; BACKLOG.md 3 (seccomp-Absatz); `tests/escape/` ESC-3; `docs/SECURITY.md` 5.

---

## HUM-137 · Ein Agent, den es nicht gibt, fällt lautlos aus
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-040, HUM-067 · Blockiert: das Vertrauen in den grünen Ring

### Kontext
Am 2026-09-07 hat der Live-Test des Sandbox-Bildschirms (`app/test/features/sandbox/daemon_live_test.dart`, HUM-040) einen Fall sichtbar gemacht, den vorher niemand gesehen hat: Die Sandbox startet, die drei Isolationsprüfungen sind grün, der Bildschirm zeigt `running` — und der Agent ist nie gelaufen. Im Test lag `opencode` nicht auf dem `PATH` der Sandbox (`/usr/local/bin:/usr/bin:/bin`), das `exec` scheiterte nach Millisekunden, und die Momentaufnahme trug `agentRunning = false` und `diagnostics = []`.

Kein Befund, keine Zeile, nichts. Ein Mensch vor diesem Bildschirm sieht einen grünen Ring und eine Sitzung, in der nichts passiert; er wartet auf einen Agenten, der nie kommt, und das Werkzeug sagt ihm nicht, warum. Für ein Werkzeug, dessen ganzer Zweck es ist, über den Agenten Auskunft zu geben, ist das die falsche Art zu schweigen (`docs/UX.md` 4.4: kein toter Winkel).

Gefunden hat es der Ersatz-Reviewer im Review zu HUM-040, mit einer eigenen Messung: `argvPreview` endet auf `-- /run/humanitl/humanitl-shim --proxy-port 3128 -- opencode`, und dieses `opencode` gibt es in der Sandbox nicht.

### Ziel
Startet der Agent nicht, sagt es der Daemon: ein `Diagnostic` mit Code, `why` und, wo möglich, `fix`, das über den `Sandbox`-Strom geht und im Bildschirm erscheint. Der Ring darf grün sein — die Sandbox steht ja —, aber die Zeile darunter nennt den Grund, aus dem nichts läuft.

### Nicht-Ziel
Den Start der Sandbox scheitern lassen, wenn der Agent fehlt: Die Sandbox ist auch ohne ihn eine gültige Sitzung, in der ein Mensch etwas anderes startet. Den `PATH` der Sandbox aufbohren, damit `opencode` vom Host gefunden wird — was gemountet wird, entscheidet das Profil.

### Betroffene Pfade
- `daemon/crates/sandbox/src/bwrap.rs` beziehungsweise `launcher.rs`: der Ausgang des `exec` im Kind
- `daemon/crates/core-types/src/diagnostics/codes.rs`: ein neuer Code für „der Agent startete nicht"
- `daemon/crates/ipc/src/sandbox.rs`: der Befund geht als `SandboxEvent.diagnostic` hinaus
- `app/lib/features/sandbox/`: die Zeile, die ihn zeigt

### Spezifikation
Endet der Agent, bevor er das erste Byte geschrieben hat, und ist sein Exit-Code einer der Schalen-Codes für „nicht gefunden" oder „nicht ausführbar" (`127`, `126`), erzeugt der Daemon einen Befund mit dem Namen des Kommandos, dem `PATH` der Sandbox und dem Hinweis, dass das Profil entscheidet, was gemountet wird. Der Befund geht denselben Weg wie die übrigen Befunde des Starts.

### Tests
- Ein Test in `daemon/crates/ipc/tests/`, der eine Sitzung mit `command = ["gibt-es-nicht"]` startet und den Befund im Strom erwartet. Mutationsprobe: den Zweig entfernen, Test rot.
- Eine Ergänzung in `app/test/features/sandbox/daemon_live_test.dart`: Die Momentaufnahme trägt den Befund, wenn der Agent fehlt.

### Akzeptanzkriterien
- [x] Eine Sitzung mit einem Kommando, das es nicht gibt, erzeugt einen Befund mit Code, `why` und `fix`. **Vorher gemessen am 2026-09-19:** kein Befund; der Daemon-Weg sendete nach dem Exit-Code `127` nur `Exit`, und die Vorprüfung des Adapters läuft für ein fremdes Kommando nicht. **Gemessen am 2026-09-19** mit `a_command_that_does_not_exist_is_said_in_the_stream` (`daemon/crates/ipc/tests/sandbox_start.rs`, echter Shim): `AGENT_005` als `Error` vor dem Exit-Code `127`, `why` mit Kommando und `PATH` der Sandbox, `fix` gesetzt, drei grüne Garantien, `running`, kein `failed`. Gegenprobe `an_agent_that_wrote_before_127_is_no_finding`: `/bin/sh -c gibt-es-nicht` endet mit `127`, schreibt aber, also kein Befund. Mutationsproben: Zweig in `report_exit` stillgelegt — rot; `first.observe` gestrichen — Gegenprobe rot; Shim-Zeile als Ausgabe gezählt — rot; `Blocking` statt `Error` — rot; `PATH` ohne Rückhalt — rot. **Nachgemessen am 2026-09-23 nach dem Review** (Codex blockierend, Antigravity): Das gescheiterte `exec` meldet der Shim jetzt auf dem Berichtskanal (`EXEC fail errno=<n>`), der Text im Terminal entscheidet nie. Neue Tests `a_forged_shim_line_on_the_terminal_is_no_finding`, `whitespace_before_127_is_output_and_no_finding`, `the_agent_cannot_write_the_exec_line_into_the_report`, `a_command_name_with_a_newline_is_still_said`, dazu `exec_failure_is_reported_on_the_report_channel` und `an_agent_that_ran_leaves_no_exec_line` im Shim; jede mit Mutationsprobe (Meldung im Shim gestrichen, `FD_CLOEXEC` entfernt, Leser ignoriert die Zeile, Daemon ignoriert den Bericht, Ausgabe egal, Leerraum zählt nicht, Exit-Code ungeprüft).
- [x] Der Bildschirm zeigt ihn, und der Ring bleibt bei der Wahrheit über die Sandbox. **Gemessen am 2026-09-19** mit `app/test/features/sandbox/agent_never_ran_test.dart` (Linux-Variante): Karte `AGENT_005` unter dem Kopf mit dem Satz des Daemons, Zustand `running`, alle drei Segmente `passed`, kein blockierender Befund; und gegen den echten Daemon mit `a_missing_agent_is_a_finding_in_the_snapshot` in `daemon_live_test.dart` (`PATH=/usr/local/bin:/usr/bin:/bin` aus dem mitgelieferten Profil). Mutationsproben: Befundblock nicht gezeichnet — rot; Ring färbt sich an einem Fehlerbefund — rot; Provider verwirft den Befund — Widget- und Live-Test rot; Zweig im Daemon stillgelegt — Live-Test rot.
- [x] `make check` grün. **Gemessen am 2026-09-19** mit `STRICT=1 make check` in einem privaten `/tmp`: Exit 0. `make escape` dazu: 124 bestanden, 0 rot.

### Fallstricke
- Ein Agent, der sich selbst sofort beendet (`--version`), ist kein Fehler: Unterschieden wird an `126`/`127` und daran, dass nichts geschrieben wurde.

### Referenzen
Review zu HUM-040 am 2026-09-07; `app/test/features/sandbox/daemon_live_test.dart`; `docs/UX.md` 4.4.

---

## HUM-136 · Die Oberfläche verschwindet, und niemand weiß warum
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-042 · Blockiert: das Vertrauen in ein Werkzeug, das stundenlang offen steht

### Kontext
Am 2026-09-07 hat der Nutzer die Oberfläche zum ersten Mal längere Zeit mit einem echten OpenCode in der Sandbox benutzt. Sein Bericht: „ja, aber nach einiger Zeit war die OpenCode-View dann weg", und die Anwendung war später von selbst beendet. Der Daemon lief weiter, die Sandbox lief weiter, der Agent lief weiter — nur das Fenster war fort.

Was in den Protokollen steht, ist nichts. Beide Läufe (`/tmp/hum-gui/app.log`, `app2.log`) enden mit derselben Zeile, die auch ein sauberes Schließen erzeugt:

```
embedder.cc (2615): 'FlutterEngineRemoveView' returned 'kInvalidArguments'. Remove view info was invalid. The implicit view cannot be removed.
```

Kein Dart-Stack, kein Fehlerfeld, keine Diagnose. Der zweite Lauf trägt davor eine Warnung des Renderers (`Timed out waiting for OpenGL frame of size 2556x1376 (have 1916x1016)`), der erste nicht. `coredumpctl` und das Journal geben für den Zeitraum nichts her.

**Ein dritter Fall am selben Tag, und diesmal steht eine Spur darin.** Um 12:01 endete ein weiterer Lauf (`/tmp/hum-gui/app3.log`), und vor der Zeile über die Ansicht steht:

```
(dev.humanitl.humanitl:682832): GLib-GObject-CRITICAL **: 12:01:12.016: object_ref: assertion '!object_already_finalized' failed
```

Ein Zugriff auf ein GTK-Objekt, das schon abgeräumt war. Das ist kein Beweis, aber die erste Aussage über die Schicht, in der es passiert: nicht Dart, sondern der Fensterteil. Wer dieses Issue baut, fängt dort an -- und die Zeile zeigt zugleich, warum das Ziel oben richtig ist: Diese Meldung stand nur deshalb im Protokoll, weil jemand die Ausgabe der Anwendung in eine Datei geleitet hatte. Ohne das wäre auch sie fort.

**Zwei Erklärungen sind offen, und keine ist belegt.** Erstens ein Absturz in der Renderer-Schicht (Impeller/OpenGLES), zu dem Flutter nichts schreibt. Zweitens ein Fremdeinfluss von außen: In derselben Sitzung hat ein Agent mehrfach `pkill -f` mit einem Muster benutzt, das auf `humanitl` passt; ein solcher Aufruf hätte das Fenster mitgenommen. Solange die Anwendung ihr eigenes Ende nicht aufschreibt, bleibt beides gleich wahrscheinlich, und genau das ist der Fehler: Ein Werkzeug, das den ganzen Arbeitstag offen steht, muss über sein eigenes Ende Auskunft geben.

### Ziel
Wenn die Anwendung endet, steht danach fest, warum. Ein sauberes Schließen ist als solches erkennbar, ein Absturz hinterlässt eine Spur mit Zeitpunkt, Signal oder Ausnahme, und ein Ende von außen ist von beidem unterscheidbar.

### Nicht-Ziel
Die Ursache raten oder den Renderer wechseln, bevor eine Messung dafür spricht. Ein Absturzbericht, der irgendwohin ins Netz geht — was hier entsteht, bleibt auf dem Rechner des Nutzers. Sitzungswiederherstellung („die Ansicht war weg, hol sie zurück") gehört zum zweiten Teil unten und nicht in dieses Ziel.

### Betroffene Pfade
- `app/lib/main.dart`: ein Zonen-Fehlerhandler (`runZonedGuarded`) und `FlutterError.onError` schreiben in dieselbe Datei
- `app/lib/core/diagnostics/` (neu): die Datei, ihre Rotation und ihre Obergrenze
- `app/linux/runner/`: das Signal, das ein Ende von außen erzeugt (`SIGTERM`, `SIGHUP`), wird notiert, bevor die Schleife endet
- `docs/DIAGNOSTICS.md`: wo die Datei liegt und was darin steht — **nicht möglich, und zwar dauerhaft** (festgestellt am 2026-09-13): Diese Datei wird vollständig aus `daemon/crates/core-types/src/diagnostics/codes.rs` erzeugt, und `docs_in_sync` in `daemon/crates/core-types/tests/diag_docs.rs` vergleicht sie Byte für Byte mit dem erzeugten Text. Ein von Hand angehängter Abschnitt macht `make check` rot und verschwindet beim nächsten `UPDATE_DIAG_DOCS=1`. Die Beschreibung des Protokolls steht deshalb dort, wo sie mit dem Code altert: im Bibliotheks-Kommentar von `app/lib/core/diagnostics/app_log.dart` (Ort, Format, beide Grenzen, warum nicht `/tmp`) und im Kopf von `app/linux/runner/exit_log.h` (was der Runner schreibt und warum async-signal-safe). Wer die Datei trotzdem in einem Dokument nennen will, nimmt ein handgeschriebenes — etwa `docs/UX.md` oder ein neues `docs/LOGS.md`.

### Spezifikation
Die Anwendung schreibt beim Start eine Zeile mit Zeitpunkt, Version und Prozesskennung nach `$XDG_STATE_HOME/humanitl/app.log` (Vorgabe `~/.local/state/humanitl/app.log`) und beim geordneten Ende eine zweite. Dazwischen landen dort nur Ausnahmen: `FlutterError.onError`, `PlatformDispatcher.instance.onError` und die Zone um `runApp`. Die Datei ist auf 256 KiB begrenzt und wird bei Überschreitung einmal rotiert (`app.log.1`); mehr Platz darf ein Protokoll auf der Platte des Nutzers nicht kosten (`docs/DIAGNOSTICS.md`, Sparsamkeit).

Ein Ende durch ein Signal wird im Runner abgefangen (`SIGTERM`, `SIGHUP`, `SIGINT`) und als eigene Zeile geschrieben, bevor die Schleife endet. Damit ist die Frage „Absturz oder von außen beendet" nach dem nächsten Vorfall in einer Zeile beantwortet.

Der zweite Teil betrifft die verschwundene Ansicht: `TerminalPane` zeigt heute nichts, was erklärt, warum ein Terminal fort ist. Der Zustand des Stroms (`TerminalPhase`) bekommt für das Ende einen sichtbaren Platz — beendet der Agent, steht dort sein Exit-Code; endet der Strom ohne Exit, steht dort der Befund des Daemons; endet die Verbindung, steht dort, dass die Sitzung weiterläuft und wie man sich wieder anhängt.

### Tests
- Ein Widget-Test, der `FlutterError.onError` auslöst und prüft, dass die Datei danach genau eine Zeile mehr hat und die Zeile den Fehlertext trägt.
- Ein Test für die Rotation: 300 KiB geschrieben, danach zwei Dateien, die neuere unter 256 KiB.
- Ein Test des Terminals, der den Strom ohne Exit enden lässt und die Erklärung im Fenster erwartet (Mutationsprobe: die Erklärung entfernen, Test rot).
- Manuell: die laufende Anwendung mit `SIGTERM` beenden, danach steht die Zeile in der Datei.

### Akzeptanzkriterien
- [x] Nach einem Ende der Anwendung steht in `~/.local/state/humanitl/app.log`, ob es geordnet, durch eine Ausnahme oder durch ein Signal kam. **Gemessen am 2026-09-13 mit dem gebauten Binary** (`flutter build linux --debug`, gestartet unter `xvfb-run` mit eigenem `XDG_STATE_HOME`): Start schreibt `start pid=2088198 version=0.0.0`, ein `kill -TERM` darauf `signal pid=2088198 name=SIGTERM`, und der Prozess stirbt weiterhin an `SIGTERM` statt an einem eigenen `exit`. Ein Lauf ohne Bildschirm, den GTK selbst mit `exit(1)` beendet („cannot open display"), schreibt `stop pid=2086261 status=1`; vor dieser Änderung stand dort nichts. Die Datei und ihr Verzeichnis stehen dabei auf `0600` in `0700` (am selben Lauf mit `ls -l` nachgesehen: `-rw-------` und `drwx------`), und was in eine Ausnahme-Zeile darf, geht vorher durch `redactForLog`. Dazu 23 Prüfungen in `app/linux/runner/exit_log_test.cc` (Format, Pfad, Rotation, `SIGTERM`, `SIGABRT`, fremdes `exit`) und 54 in `app/test/core/diagnostics/` (Startzeile, Ende, Ausnahme mit Text und Herkunft, alle drei Handler einschließlich der Zone, Rechte, Redaktion). `0600` gilt auch für die Datei, die eine Rotation der Dart-Seite neu anlegt, gemessen unmittelbar nach der rotierenden Zeile und keiner weiteren („auch die Datei nach einer Rotation steht auf 0600"); bis zum 2026-09-18 legte die nächste Zeile sie mit der Maske des Prozesses an, `rw-rw-r--`. Die Rechte hängen an einer Marke **je Ziel**: Ein Verzeichnis, an dem `chmod` scheitert -- gemessen an `/tmp`, das dem Systemverwalter gehört --, darf der Datei darin ihren eigenen Versuch nicht wegnehmen, und nur ein `chmod`, das gelingt und trotzdem nichts ändert, gilt als aussichtslos.
- [x] Die Datei bleibt unter 512 KiB (256 KiB plus eine Rotation), gemessen mit einem Test. **Gemessen am 2026-09-13**: `app/test/core/diagnostics/app_log_test.dart`, „300 KiB hinterlassen zwei Dateien, die neuere unter 256 KiB" — nach 300 KiB liegen genau zwei Dateien im Verzeichnis, beide unter `AppLog.maxBytes`, Summe unter 512 KiB. Dieselbe Grenze prüft `exit_log_test.cc` für den Runner, weil er als zweiter Schreiber an derselben Datei hängt. Mutationsprobe: Rotation entfernt, Test rot (`Expected: true, Actual: <false>`).
- [x] Ein Terminal, dessen Strom endet, erklärt im Fenster, was passiert ist, und nennt den Weg zurück. **Gemessen am 2026-09-13 und 2026-09-18**, neun Tests in `app/test/features/sandbox/terminal_end_test.dart`. Gebaut wurde die Phase `TerminalPhase.detached`: `_onDone` fällt nicht mehr stillschweigend auf `idle` zurück, und `_onError` verwirft einen Fehler ohne `Diagnostic` nicht mehr, sondern nimmt ihn als das, was er ist — die Leitung, nicht die Sitzung. Unter dem Terminal steht dann ein Streifen (`sandbox-terminal-detached`) mit einem von zwei Sätzen, und keiner behauptet, dass die Sitzung lebt (`backlog/CONVENTIONS.md` 4.13): Bei `DAEMON_001` antwortet gar kein Daemon mehr, und die Sandbox lebt in diesem Daemon — dort steht `sandboxTerminalConnectionLost`, dass der Zustand der Sitzung unbekannt ist und `humanitl sandbox attach` wieder verbindet, sobald der Daemon antwortet; der Befund darüber trägt den Fix, der ihn startet. Endete nur der Strom bei antwortendem Daemon, steht `sandboxTerminalDetached`: kein Exit-Code, dieses Fenster hängt nicht mehr daran, und falls die Sitzung noch läuft, führen dieser Bildschirm oder `humanitl sandbox attach` hin. Auch ein Strom, der vor der ersten Geometrie endet, landet dort und nicht still in `idle`. Ein alter `DAEMON_001` wird geräumt, sobald der Daemon wieder antwortet — bei der ersten Geometrie, nicht schon beim Versuch: Ein Daemon, der die Verbindung annimmt und dann schweigt, ließe sonst ein Fenster ohne jedes Wort zurück. So steht über einem lebenden Terminal nicht „kein Daemon", und ein späteres gewöhnliches Ende wird nicht als verlorene Verbindung erklärt; `TERM_001` bleibt stehen, weil `_watchInstead` ihn für den neuen Anschluss setzt. Ein Exit-Code gewinnt über die Erklärung, weil er die genauere Auskunft ist; der Befund des Daemons steht weiterhin über dem Terminal. Der Fall, der in der Anwendung wirklich eintritt, ist ein `DaemonException` mit `DAEMON_001`: `GrpcDaemonClient.terminal` verpackt jeden Fehlschlag so, also entscheidet der Code und nicht der Typ der Ausnahme — `IPC_001`, der einzige andere Code, der auf diesem Weg als Status ankommt, bleibt eine Absage mit Befund; `TERM_001` kommt als Frame und nie als Fehler. Mutationsproben: `_onDone` zurück auf `idle` — zwei Tests rot (`Expected: detached, Actual: idle`); jeder `DaemonException` als Absage — ein Test rot (`Expected: detached, Actual: refused`); `_onError` ohne Zustandswechsel — ein Test rot (`Actual: attached`); den Streifen entfernt — zwei Tests rot (`Found 0 widgets with key sandbox-terminal-detached`); ein Satz für beide Enden — ein Test rot (`Found 0 widgets with text containing is unknown`).
- [x] `make check` grün. **Gemessen am 2026-09-18** über das Maß, das `CLAUDE.md` Schritt 5 verlangt: `tools/verify-commit.sh` über den Merge `db06779` ist grün, und die CI über denselben Commit ist mit allen elf Jobs grün, darunter `flutter-analyze-test`, `goldens`, `e2e-xvfb` und `escape-tests`. Die frühere, teilweise Messung dieses Kästchens ist damit überholt.

### Fallstricke
- Ein Protokoll, das bei jedem Bild schreibt, ist ein Protokoll, das die Platte füllt: Nur Start, Ende und Ausnahmen.
- `runZonedGuarded` fängt nichts, was im nativen Teil abstürzt; das Signal aus dem Runner ist deshalb kein Zierrat, sondern die zweite Hälfte der Antwort.
- Kein Pfad in `/tmp`: Ein Protokoll, das der nächste Neustart wegräumt, beantwortet die Frage nie.

### Referenzen
Bericht des Nutzers am 2026-09-07; `/tmp/hum-gui/app.log` und `app2.log` desselben Tages; `docs/DIAGNOSTICS.md`; HUM-042 für den Terminal-Zustand.

---

## HUM-134 · Zwei Tests werden unter Last rot
Sprint: 4 · Größe: S · Abhängigkeiten: — · Blockiert: eine verlässlich grüne Pipeline

### Kontext
Am 2026-09-07 fiel `cargo test -p humanitl-sandbox --lib` in einem sonst grünen `STRICT=1 make check` mit

```
thread 'bwrap::tests::find_program_reads_path_from_the_given_env_only' panicked at crates/sandbox/src/bwrap.rs:1317:
runs: Diagnostic { code: SANDBOX_001, why: "cannot run /tmp/.tmpyk03Mt/bwrap --version: Text file busy (os error 26)" }
```

Derselbe Test lief unmittelbar danach fünfmal einzeln grün. Der Fehler ist `ETXTBSY` und damit ein bekanntes Rennen zwischen `fork` und `exec` in einem Testbinary mit mehreren Threads: Der Test schreibt in `crates/sandbox/src/bwrap.rs:1305-1309` ein ausführbares Skript und startet es sofort (`query_version`). Forkt ein anderer Test desselben Binaries genau in dem Augenblick, in dem die Datei noch zum Schreiben offen ist, erbt sein Kind den Deskriptor; bis das Kind `exec` erreicht, hält es die Datei zum Schreiben offen, und unser `exec` bekommt `ETXTBSY`. `O_CLOEXEC` hilft nicht, weil das Fenster genau zwischen `fork` und `exec` liegt.

**Ein dritter Fall, gefunden am 2026-09-07 in der CI und dort behoben:**
`body_cap_blocks` (`daemon/crates/proxy/tests/authority.rs`) ist im Lauf
`34097563533` mit `hyper::Error(BodyWrite, BrokenPipe)` gefallen, lokal in
zwölf Einzelläufen und sechs Läufen der ganzen Datei auf zwei Kernen unter Last
nicht reproduzierbar. Ursache: Der Proxy lehnt ein angekündigtes
`Content-Length` über dem Cap mit `413` ab, **ohne den Body zu lesen**
(Absicht), und lässt die Verbindung fallen; ein Client, der die angekündigten
64 KiB wirklich schreibt, bekommt dabei `EPIPE`, und hyper gibt den
Schreibfehler zurück statt der Antwort, die längst auf der Leitung stand. Der
Test schreibt seitdem nur den Kopf und sechzehn Bytes und liest die Antwort
über `Proxy::raw_exchange`. Er ist damit erledigt; die Zeile steht hier, weil
das Muster dasselbe ist und wer dieses Issue baut, die drei Fälle zusammen
sehen soll.

**Ein zweiter Fall am selben Tag, andere Stelle, dieselbe Art:** `cargo test -p humanitld --test daemon_end_to_end` fiel einmal mit `the_configured_llm_endpoint_becomes_a_passthrough_rule` und `DAEMON_001: cannot reach the daemon on /tmp/hum0CSC7p/run/humanitl/daemon.sock: transport error`; derselbe Test lief danach dreimal einzeln und einmal als ganze Datei (10 von 10) grün. Auch das ist ein Rennen unter Last und keine Aussage über den Daemon; wer dieses Issue baut, sieht sich beide Stellen an, denn eine Pipeline, die ohne Grund rot wird, kostet jedes Mal dieselbe Suche.

Das Rennen ist ein Fehler des Tests, nicht des Codes: `BwrapBackend::find_program` und `query_version` tun das Richtige, und ein Nutzer trifft die Lage nicht. Rot wird davon aber die Pipeline, und ein Lauf, der ohne Grund rot ist, kostet jedes Mal die Suche nach einer Ursache, die es nicht gibt.

### Ziel
`cargo test -p humanitl-sandbox` ist unter paralleler Last verlässlich grün, ohne dass der Test weniger prüft als heute.

### Nicht-Ziel
Den Test seriell zu stellen (`--test-threads=1` für die ganze Crate) — das verlangsamt jeden Lauf wegen eines Falls. Auf das Ausführen zu verzichten und `query_version` zu überspringen — dann misst der Test die Zeile nicht mehr, um die es geht.

### Betroffene Pfade
- `daemon/crates/sandbox/src/bwrap.rs` (`mod tests`)

### Spezifikation
Ein `exec`, das mit `ETXTBSY` scheitert, wird im Test bis zu fünfmal mit 20 ms Abstand wiederholt; erst danach gilt er als gescheitert. Alternativ schreibt der Test die Datei über einen eigenen Prozess (`cp`) oder wartet mit `fsync` plus erneutem Öffnen, bis kein Schreiber mehr offen ist. Die Wiederholung steht mit ihrer Begründung im Test, damit niemand sie später für Zierrat hält.

### Tests
Der bestehende Test bleibt; er ist die Messung. Zusätzlich ein Lauf mit `--test-threads=8` in einer Schleife (zwanzigmal), der vor und nach der Änderung gefahren und im Commit-Body mit seinen Zahlen genannt wird.

### Akzeptanzkriterien
- [x] Zwanzig Läufe `cargo test -p humanitl-sandbox --lib -- --test-threads=8` hintereinander sind grün. **Gemessen am 2026-09-07: 20 von 20**, nach dem Umbau der beiden Stellen, die ein Skript schreiben und sofort ausführen (`write_program` in `daemon/crates/sandbox/src/lib.rs` schreibt über einen eigenen Prozess, dessen Deskriptor kein Faden dieses Prozesses erben kann).
- [x] Zwanzig Läufe `cargo test -p humanitld --test daemon_end_to_end` hintereinander sind grün, oder der zweite Fall ist als eigene Ursache benannt und mit einer eigenen Messung erledigt. **Gemessen am 2026-09-07: 20 von 20.** Für diesen Fall war keine Änderung nötig; er bleibt beobachtet, und die Messung steht hier, damit die nächste Rotfärbung eine Zahl hat, gegen die sie sich vergleichen lässt.
- [x] Der Test prüft weiterhin `SANDBOX_001` für einen leeren Pfad, das Finden im zweiten `PATH`-Eintrag, `query_version` und `SANDBOX_002` für eine zu alte Version. Nur das Schreiben der Datei hat sich geändert, keine Zusicherung.
- [x] `make check` grün. **Gemessen am 2026-09-07**, zusammen mit `tools/verify-commit.sh` über den fertigen Commit.

### Fallstricke
- Eine Wiederholung, die jeden Fehler auffängt, verdeckt einen echten: Nur `ETXTBSY` wird wiederholt, jeder andere Befund scheitert sofort.
- **Zwei Mittel gegen dieselbe Ursache stehen jetzt im Repository**, und das ist Absicht: die Wiederholung in `daemon/bin/humanitl/tests/cli.rs` (`output_when_not_busy`) und das Schreiben über einen eigenen Prozess (`write_program`). Keines ersetzt das andere. Offen bleibt eine Stelle: das falsche `systemctl` in `cli.rs:2345` und `:2464` wird noch auf die alte Art geschrieben; ausgeführt wird es vom Kind der Kommandozeile und nicht vom Testbinary, und gescheitert ist es nie -- gefunden im Review am 2026-09-07, aufgeschrieben statt geändert.

### Referenzen
`daemon/crates/sandbox/src/bwrap.rs:1295-1326`; Beobachtung am 2026-09-07 im Lauf zu HUM-039.

---

## HUM-153 · Im History-Detail bleibt dem Body kaum Platz
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-032, HUM-116 · Blockiert: nichts; der Body ist erreichbar, nur nicht auf einen Blick

### Kontext
Seit HUM-116 zeigt das History-Detail Anfrage und Antwort mit derselben `BodyView` wie die Warteschlange. Gemessen am 2026-09-11 in den Goldens: Bei einem Fenster von 1400 × 900, einem Detail-Anteil von 0,4 und einem Kopfzeilen-Anteil von 45 % bleiben dem Body etwa 95 px. `BodyView` zeigt dort nur Titel und Umschalter, der Inhalt liegt unter der Falz; er scrollt, es läuft nichts über. Die Rumpf-Goldens `history_detail_body_json_*` und `history_detail_body_hex_*` werden deshalb in einem Fenster von 1400 × 1500 aufgenommen; der Grund steht als Kommentar in `app/test/goldens/history_golden_test.dart`.

### Ziel
Wer im Verlauf eine Anfrage öffnet, sieht bei 1400 × 900 den Anfang ihres Bodys ohne zu scrollen.

### Nicht-Ziel
Eine andere `BodyView`. Die Aufteilung zwischen Tabelle und Detail als Ganzes neu zu entwerfen.

### Akzeptanzkriterien
- [x] Ein Golden bei 1400 × 900 zeigt den Titel des Bodys und mindestens zehn Zeilen des JSON-Baums.
- [x] Die Rumpf-Goldens laufen wieder im Standardfenster des Tests; der Kommentar dazu entfällt.

### Referenzen
HUM-032 (Split von Tabelle und Detail), HUM-116; `app/lib/features/history/history_detail.dart`, `app/test/goldens/history_golden_test.dart`.

---

## HUM-154 · „No body." steht in einer Farbe, die für Sätze zu schwach ist
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-030 · Blockiert: nichts

### Kontext
`BodyView` (`app/lib/core/body/body_view.dart`, seit HUM-116 in `core`) zeichnet den Satz „No body." in `fg2`. `docs/UX.md` erlaubt `fg2` nicht für Sätze, weil sein Kontrast dafür nicht reicht. Gefunden am 2026-09-11 beim Bau von HUM-116; der Verlauf umgeht es, indem er einen leeren Body selbst mit `historyDetailNoBody` in `fg1` beschriftet. Die Warteschlange zeigt den Satz weiter in `fg2`.

### Ziel
Ein leerer Body wird in beiden Bildschirmen mit einem Satz in `fg1` benannt, und der Verlauf braucht dafür keinen eigenen Weg mehr.

### Akzeptanzkriterien
- [x] `BodyView` zeichnet den Satz für einen leeren Body in `fg1`; ein Widget-Test prüft die Farbe, und die Mutation zurück auf `fg2` macht ihn rot. Gemessen am 2026-09-18: vier Widget-Tests lesen den Stil des Satzes am Schlüssel `body-empty` — `app/test/features/intercept/body/jump_test.dart` „the sentence about an empty body carries fg1", `app/test/features/intercept/body/pending_test.dart` „a body view that is not waiting names the missing body", `app/test/features/history/history_body_test.dart` „an empty body is named by the body view, in fg1" und „a response that never came gets the same sentence". Die Mutation auf `fg2` macht alle vier rot: erwartet `#A3A9B8`, bekommen `#6B7186`.
- [x] Der Sonderweg im History-Detail entfällt, oder er bleibt mit einem Grund, der nicht die Farbe ist. Gemessen am 2026-09-18: er entfällt ganz. `_Body` zeichnet keinen eigenen Satz mehr, jeder Rumpf beider Bildschirme geht durch `BodyView`, und `historyDetailNoBody` samt `@historyDetailNoBody` ist aus beiden ARB-Dateien entfernt (`app_en.arb` 1872 auf 1870 Zeilen, `app_de.arb` 692 auf 691). An die Stelle des Zweigs tritt `BodyView.pending`, ohne Vorgabewert, damit keine Aufrufstelle das Warten vergessen kann: ein fehlender Rumpf ist Warten, solange die Seite noch kommen kann, und sonst dieselbe Aussage wie ein leerer Rumpf — unter dem blanken Titel `Body`, denn ohne Verweis hat niemand eine Größe gemessen. Gewartet wird
  - in der Warteschlange, solange das Detail keinen Wert hat; nicht `isLoading`, das bei riverpods eigenen Wiederholungen zwischen wahr und falsch springt;
  - im Antwort-Tab der History, solange `responseIsFinal` der lebenden Zeile falsch ist — dieselbe Frage, die die Größe im Kopf stellt, deshalb öffentlich und an einer Stelle;
  - im Tab der bearbeiteten Anfrage, solange sie fehlt und der Flow nicht `recorded` ist; `failed` ist kein Ende (`daemon/crates/core-types/src/flow.rs`: `Failed` + `Record` = `Recorded`);
  - in den Kopfzeilen derselben Seite, mit einem Skelett statt „No answer came back." oder „No headers.".

  Damit jedes Warten endet, holt das Detail nach, sobald der Datensatz geschrieben ist, sobald die Zeile eine bearbeitete Anfrage meldet, und bei jedem weiteren Schritt (`forwarded`, `responded`), solange sie noch fehlt — und nur, was es nicht schon hält. Das letzte ist nötig, weil der Daemon `Decided` veröffentlicht, bevor der Schreiber `Dir::RequestEdited` übernimmt, und der schreibt in Stapeln alle 50 ms: ein `GetFlow` gleich nach der Entscheidung kann ohne sie zurückkommen. Das Nachfassen je Schritt verkleinert dieses Fenster, schließt es aber nicht; fallen die Schritte in dieselben 50 ms, wartet der Tab bis `recorded`. Das Blatt, das ein Doppelklick oder `flowRevealProvider` öffnet, liest seine Zeile aus der Seite, fällt nie hinter `recorded` zurück und behält den zuletzt gesehenen Stand, wenn die Zeile aus der Seite fällt. Für einen Flow ohne Zeile horcht es auf `Recorded` und `Lagged` zu genau dieser Id — auch in dem Fenster, in dem der Flow noch geholt wird. `Failed` allein löst nichts aus, weil `Recorded` für jeden fertigen Flow kommt und zwei Abfragen hintereinander dasselbe fragten. Siebenundzwanzig Mutationen, siebenundzwanzig rote Tests, jede Datei danach Byte für Byte zurück: vier aus den engen Delta-Prüfungen (die vollere fertige Zeile wird nicht behalten, jedes Paket fragt erneut nach, eine dünne fertige Zeile nimmt dem Blatt Bekanntes weg, ein älterer Abruf desselben Flows setzt das Blatt), drei aus dem Delta-Review (kein Nachfassen je Schritt, Doppelklick lässt den laufenden Abruf stehen, Blatt friert die ganze Zeile statt nur den Zustand ein), zehn aus der Fix-Runde (Titel mit erfundener Größe, `Lagged` überhört, bearbeitete Anfrage nicht nachgeholt, Datensatz nicht nachgeholt, doppelte Abfrage nach `Failed` und nach bereits geholtem Datensatz oder bearbeiteter Anfrage, Blatt fällt hinter `recorded` zurück, Ende im Abruffenster verloren, Abfrage nach `Lagged` über einem fertigen Blatt) und zehn aus den Vorrunden gegen den Endstand (Farbe, `pending` übergangen, eingefrorener Abzug im Blatt, kein Nachziehen des Abzugs, `isLoading` in der Karte, Fake mit `null` für eine leere Antwort, `pending` aus dem Abzug statt aus der Zeile, bearbeitete Anfrage an `responseIsFinal`, kein Skelett der Kopfzeilen, `historyResponseStreaming` statt `responseIsFinal`).
- [x] Die betroffenen Goldens sind geprüft und mit Erklärung neu aufgenommen. Gemessen am 2026-09-13: 12 von 97 Golden-Bildern, Pixel für Pixel gegen die alten verglichen. Zehn zeigen genau einen Block, nur in der Farbe geändert, von `fg2` `#6B7186` auf `fg1` `#A3A9B8` dunkel und von `#7C8294` auf `#4B5162` hell: `action_bar_note`, `queue_grouped_collapsed`, `queue_grouped_expanded`, `queue_new_pill` mit je 1536 px und `queue_grouped_scale2` mit 6144 px, weil der Block bei `TextScaler.linear(2)` doppelt so groß ist. `history_detail_note_dark` und `history_detail_note_light` ändern 53 792 px: statt des eigenen Satzes über die ganze Breite steht dort jetzt der Rumpf-Abschnitt mit seiner Titelzeile und dem kurzen Satz darunter, in derselben Farbe wie vorher. Keiner der späteren Durchgänge ändert ein weiteres Bild; am 2026-09-18 laufen alle Golden-Tests grün gegen diese zwölf.

### Bewusst so entschieden
- `_sheetFlow = live` steht in `HistoryScreen.build` ohne `setState`. Gebaut wird dort ohnehin, gelesen wird im selben Bau der frischere Wert, und die Zuweisung geht nur vorwärts: nie zurück hinter `recorded`. Sie läuft auch bei Bauten, die mit der Seite nichts zu tun haben (Fokus, Größe, Theme); dann ist `live` gleich dem Abzug, und es ändert sich nichts.
- `HistoryDetail` bleibt im Blatt ohne Key. Solange das Blatt offen ist, steht seine Id fest; ein Doppelklick auf eine andere Zeile setzt `_tab` und `_copied` über `didUpdateWidget` zurück, und `_takeReveal` setzt `_sheetFlow` zuerst auf null, sodass eine andere Id dort ohnehin ein frisches Element bekommt.

### Offen nach HUM-154
- **Kopf eines aufgedeckten Flows, solange er läuft.** Für einen Flow ohne Zeile folgt das Blatt nur `Recorded` und `Lagged`, nicht `ResponseHeaders`. Status und Antwort-Kopfzeilen bleiben deshalb „—" beziehungsweise Skelett, bis der Flow endet — bei einer LLM-Durchleitung dreißig bis sechzig Sekunden, obwohl beides am Anfang ankam. Nachfolger: `ResponseHeaders` für die Id des Blatts nachholen oder das Blatt an eine Quelle je Flow hängen, die nicht von der Seite abhängt.
- **Mindestdauer des Skeletts.** `HistoryWaitGate` setzt nur die 150-ms-Hälfte von `docs/UX.md` 2.11 um, die 400 ms `HMotion.waitMinVisible` fehlen; so stand es schon vorher im eigenen Doc-Kommentar. Das neue Skelett der Kopfzeilen erbt es.
- **Karte der Warteschlange bei gescheitertem Detail.** Ein Detail, das dauerhaft scheitert, zeigt im Rumpf-Abschnitt weiter das Skelett statt einer Diagnose an seiner Stelle, und `SectionHeaders` daneben sagt „No headers.". Pfad der Warteschlange, eigene Strings.
- **Bearbeitete Anfrage eines laufenden Flows im Daemon.** `get_flow` liefert `edited_request` für einen laufenden Flow erst, wenn der Schreiber `Dir::RequestEdited` übernommen hat (Stapel alle 50 ms, `daemon/crates/recorder/src/writer.rs`), obwohl die Entscheidung (`AllowEdited { request }`) sie längst trägt. Nachfolger im Daemon: `get_flow` füllt sie für einen laufenden Flow aus der Registry oder der Entscheidung, unabhängig vom Schreiber. Die Stellen: der RPC in `daemon/crates/ipc/src/server.rs:1089` (`get_flow`, antwortet aus `recorded_detail`) und die Abfrage in `daemon/crates/recorder/src/query.rs:166` (`get_flow`, liest nur, was der Schreiber übernommen hat). Dann braucht der Client das Nachfassen je Schritt aus HUM-154 nicht mehr. Aus dem Code hergeleitet, nicht gegen einen echten Daemon gemessen.
- **Block und Ablauf vor `recorded`.** Der Antwort-Tab wartet dort kurz. Das ist kein Fehler: der Proxy schreibt 403 und 504 selbst, und `fail_closed` bringt jeden Flow nach `Recorded`.

### Referenzen
HUM-030, HUM-116; `docs/UX.md` (Kontrast von Text); `app/lib/core/body/body_view.dart`, `app/lib/features/history/history_detail.dart`.

## HUM-156 · Der Daemon beantwortet `Audit` nicht
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-050, HUM-070, HUM-051 · Blockiert: HUM-055

### Kontext
Die Audit-Kette aus HUM-050 wird geschrieben, verankert und lässt sich aus der Datei prüfen. Wer aber fragt, bekommt vom Daemon keine Antwort: Der Dienst `Audit` in `daemon/crates/ipc/src/server.rs` antwortet `unimplemented`. Aufgefallen ist das am 2026-09-18 an zwei Issues zugleich, die beide annahmen, das jeweils andere liefere ihn. `humanitl audit verify` und `audit export` (HUM-070) fallen deshalb immer auf die schwächere Prüfung aus der Datei zurück, ohne HMAC-Schlüssel und ohne Anker, und sagen das auch. Der Audit-Screen (HUM-051) kann nur gegen den Fake-Daemon gemessen werden; drei seiner fünf Kriterien bleiben dadurch offen.

### Ziel
Der Daemon beantwortet `Audit` mit allen Operationen, die das Proto nach HUM-051 trägt: `Verify` mit Schlüssel und Ankern, `Head`, `Query` mit Filter nach Art, Sitzung und Zeitraum samt Cursor, und `Export` als JSONL oder CSV in eine Datei, die der Daemon schreibt und nie überschreibt. Danach prüfen CLI und Oberfläche dieselbe Kette mit derselben Stärke.

### Nicht-Ziel
Neue Felder im Proto: Die hat HUM-051 schon angelegt. Ein signierter Export (nach MVP).

### Betroffene Pfade
- `daemon/crates/ipc/src/server.rs` und ein eigenes Modul für den Dienst `Audit`
- `daemon/crates/audit/` nur, soweit Abfrage und Export dort eine Schnittstelle brauchen
- `proto/humanitl/v1/humanitl.proto`: `PROTO_MINOR` von 11 auf 12, mit `docs/PROTOCOL.md` und `app/lib/core/ipc/proto_version.dart`

### Akzeptanzkriterien
- [x] `humanitl audit verify` spricht den Daemon und meldet Schlüssel und Anker; der Rückfall auf die Datei greift nur noch, wenn der Daemon nicht antwortet. Gemessen 2026-09-18: `audit_verify_asks_the_daemon_for_key_and_anchors` (`daemon/bin/humanitl/tests/cli.rs`) fragt einen `IpcServer` mit Audit-Log über den Socket und liest `hmac key: checked by the daemon`, die Zahl der Anker samt Zeitpunkt des letzten und `checked by: daemon`, unter `--json` `mode: full`, `hmac: checked`, `anchor_count` gleich der Tabelle; `audit_verify_through_the_daemon_sees_a_foreign_key` zeigt, dass der Schlüssel wirklich rechnet (fremder Schlüssel: `BROKEN at seq 1 (mac_mismatch)`, dieselbe Datei mit `--file`: `OK`). Der Rückfall: ohne Daemon weiter die Datei (`audit_verify_ok_exit_0`), bei einem Daemon, der ablehnt, nicht (`audit_verify_does_not_replace_a_refusing_daemon_with_the_file`, `only_a_silent_daemon_leads_to_the_file`). Jede Aussage unter ihrer Mutation rot gesehen.
- [x] Eine nach dem Schreiben veränderte Zeile ergibt über den Daemon „gebrochen ab Sequenz n" mit Grund, in CLI und Oberfläche gleich. Gemessen 2026-09-18 gegen einen echten `humanitld`: `a_changed_line_is_broken_at_its_seq_in_the_screen_and_the_cli` (`app/test/features/audit/audit_daemon_live_test.dart`, `make flutter-test-daemon`) ändert ein Zeichen im ersten Record, und die Provider des Bildschirms melden `firstBadSeq 1`, `hashMismatch`, `AUDIT_001`, `humanitl --json audit verify` gegen denselben Daemon Exit 4, `first_bad_seq 1`, `hash_mismatch`. Dazu `a_changed_line_breaks_at_its_seq_with_the_reason` (`daemon/crates/ipc/tests/audit_rpc.rs`) und `audit_verify_through_the_daemon_reports_a_changed_line` (CLI, `BROKEN at seq 2 (hash_mismatch)`). Wie die Karte den Bruch in Worte fasst, misst weiter `status_broken_shows_seq_and_reason`.
- [x] Der Head-Hash im Audit-Screen ist derselbe wie in `humanitl audit verify --json`. Gemessen 2026-09-18 gegen einen echten `humanitl`: `a_real_daemon_answers_within_two_seconds_with_the_head_of_the_cli` vergleicht `auditHeadProvider` mit `head.hash` und `head.seq` der Kommandozeile, gleich; `verify_and_head_name_the_same_head_and_the_anchors` hält `Head` und `Verify` des Dienstes auf demselben Hash. Die Mutation „der Kopf ist der erste statt der letzte Record" macht beide rot.
- [x] Der CSV-Export hat die Spalten aus HUM-051, der JSONL-Export ist Byte für Byte die Kette. Gemessen 2026-09-18: `export_writes_the_chain_and_the_twelve_columns_and_overwrites_nothing` (`audit_rpc.rs`) vergleicht die JSONL-Datei des Daemons mit `cmp`-Strenge gegen `audit.jsonl`, liest die zwölf Spalten `seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash` und sieht einen zweiten Export in denselben Pfad mit `AUDIT_008` scheitern, ohne dass der erste sich ändert; `export_takes_the_range_inclusively` den Zeitraum; `audit_export_goes_through_the_daemon_with_its_range` (CLI) dasselbe über `--since`/`--until` und den Daemon.
- [x] `make check` grün. Gemessen 2026-09-18 mit `STRICT=1 make check` im Arbeitsbaum dieses Issues.

### Stand (2026-09-18)

Gebaut: `daemon/crates/ipc/src/audit.rs` (`AuditService`, `parse`), verdrahtet in
`IpcServer::with_audit_log` und in `humanitld` mit dem Schlüssel und der
Anker-Tabelle des Schreibers; `humanitl_audit::query` (Ende und Seiten) und
`humanitl_audit::export` (JSONL und CSV, eine Stelle für Daemon und
Kommandozeile); `VerifyReport.head`. `PROTO_MINOR` 12, `Info.capabilities`
nennt `audit`, sobald der Dienst ein Log hat. Messungen der offenen Kriterien
von HUM-051 stehen dort.

Entscheidungen, jede mit Grund:

- **Vor jeder Operation wartet der Dienst auf den Schreiber**
  (`AuditHandle::sync`). Sonst hinge der Kopf, den Oberfläche und
  Kommandozeile sehen, daran, wie weit der Schreib-Thread gerade ist.
- **Eine Prüfung schreibt keinen Record `audit.verified`.** Die Art steht in
  HUM-050, aber ein Record je Prüfung verschöbe den Kopf mit jeder Frage, und
  der Hash nach der Prüfung der Oberfläche wäre nie der, den die Kommandozeile
  danach nennt. Die Art bleibt im Register und ohne Schreiber.
- **Der Rückfall auf die Datei greift nur, wenn kein Daemon antwortet**:
  `Unavailable`, `Unauthenticated`, `Unimplemented` (ein Daemon vor diesem
  Issue). Ein Daemon, der ablehnt (`IPC_006` ohne Audit-Log, `AUDIT_006`,
  `RECORDER_00x`), bekommt keine schwächere Prüfung als Ersatz.
- **CSV hat zwölf Spalten statt acht.** HUM-070 schrieb die acht Felder des
  Records (`seq,ts,session,kind,data,prev,hash,mac`); das Kriterium hier und
  HUM-051 verlangen die zwölf aus HUM-050. Weil Daemon und Kommandozeile
  denselben Code nehmen, gilt die neue Form für beide; `docs/cli.md` sagt es.
- **`--since`/`--until` gehen über den Daemon.** Der Vertrag schließt beide
  Grenzen ein, die Kommandozeile schneidet halboffen; sie schickt deshalb als
  obere Grenze die letzte ganze Mikrosekunde vor `--until`. Das Log schreibt
  Mikrosekunden, das Ergebnis ist dasselbe.
- **`AUDIT_009` „Audit-Anfrage ungültig"** (neu im Register, gRPC
  `InvalidArgument`): keine Operation, ein Format außer `jsonl`/`csv`, ein
  relativer Zielpfad (der Daemon läuft in einem anderen Verzeichnis), eine
  verlangte Host-Schwärzung (`redact_hosts`, nach MVP; still ignoriert stünden
  die Hosts im Export), ein unlesbarer Zeitpunkt oder Cursor.
- **Der Kopf trägt über den Daemon keinen Zeitpunkt**: `AuditResponse` hat kein
  Feld dafür, und neue Felder sind Nicht-Ziel. `head.ts` steht unter `--json`
  nur in der Prüfung der Datei. Folgearbeit: HUM-162.
- **Jede Seite liest die ganze Datei.** Die Abfrage hält nur `limit + 1`
  Records im Speicher, liest aber jede Zeile; bei sehr langen Ketten wird eine
  Seite langsam. Ungemessen. Folgearbeit: HUM-163.
- **Gelesen wird nur bis zu dem Ende, das `AuditHandle::sync` meldet** (aus
  dem Review). Der Schreiber hängt weiter an, während eine Operation liest; eine
  halb geschriebene Zeile hinter dem Kopf hieß für die Prüfung
  `NonCanonicalLine`, Exit 4, und der Vorschlag hätte das laufende Log
  beiseitegelegt. `AuditVerifier::verify_until`, `query::query`/`tail` und
  `export::export` nehmen deshalb die gemeldete Nummer und hören dort auf;
  Anker hinter ihr zählen nicht. Belegt mit
  `a_line_being_written_behind_the_head_is_no_break` und Mutation.
- **Kein Export in ein privates `/tmp`** (aus dem Review).
  `packaging/systemd/humanitld.service` setzt `PrivateTmp=yes`; ein Export
  nach `/tmp` oder `/var/tmp` landete in `/tmp/systemd-private-*`. Der Daemon
  lehnt ihn mit `AUDIT_008` ab, wenn `/proc/self/mountinfo` dort eine Wurzel
  `systemd-private-*` zeigt; ein gewöhnliches `tmpfs` auf `/tmp` bleibt
  erlaubt. Die Kommandozeile prüft nach einem Export über den Daemon, ob sie
  die Datei sieht, sonst `AUDIT_008`.
- **Der Daemon legt keine Verzeichnisse an und folgt keinem Verweis im Weg
  zum Ziel** (aus dem Review). Ein Agent in der Sandbox hat dieselbe
  Nutzerkennung und könnte im Projektverzeichnis einen Verweis pflanzen. Das
  Zielverzeichnis muss es geben und seiner aufgelösten Form gleichen, sonst
  `AUDIT_008`. Zwischen Prüfung und Schreiben bleibt ein Fenster, benannt im
  Doc-Kommentar von `refuse_linked_parent`. Die Kommandozeile legt das
  Verzeichnis weiter selbst an, wie seit HUM-070: Sie läuft als der Mensch.

---

## HUM-157 · `audit.retention_days` hat keinen Leser
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-050, HUM-051, HUM-156 · Blockiert: keine

### Kontext
HUM-050 hat den Schlüssel `audit.retention_days` angelegt, Vorgabe `0` („für immer"), und HUM-051 hat nur verlangt, dass die Kette bei `0` unberührt bleibt. Das tut sie. Einen Wert größer als `0` liest aber niemand: Wer `audit.retention_days = 365` setzt, bekommt keine Löschung und keine Warnung. Das Register der Leser (`daemon/crates/config/tests/config_readers.rs`) führt den Schlüssel deshalb als `pending`. Aufgefallen am 2026-09-18 beim Abschluss von HUM-051.

Aus einer Hash-Kette zu löschen bricht sie absichtlich: Jeder Record trägt den Hash seines Vorgängers, und der erste verbliebene Record zeigt danach auf einen Hash, den es nicht mehr gibt. Eine Löschung ist darum nur ehrlich, wenn sie die Lücke selbst dokumentiert, und wenn `verify` sie danach als dokumentierte Lücke erkennt statt als Bruch.

### Ziel
Entweder löscht der Daemon Records der Kette, die älter sind als `audit.retention_days` Tage, und hinterlässt einen Record, der die Lücke dokumentiert, den `verify` als erlaubten Neuanfang anerkennt. Oder der Schlüssel wird gestrichen (`alias::RETIRED` mit Grund), und das Audit-Log wächst ohne Löschung, wie heute. Die Entscheidung fällt am Anfang dieses Issues und steht in `docs/SECURITY.md`.

### Nicht-Ziel
Löschen einzelner Records oder Löschen nach Inhalt. Die Aufbewahrung der Aufzeichnung (`recorder.retention_days`, HUM-051).

### Betroffene Pfade
- `daemon/crates/audit/src/` (Löschung, neuer Record, `verify`)
- `daemon/bin/humanitld/src/main.rs` (täglicher Lauf)
- `daemon/crates/config/src/model.rs`, `daemon/crates/config/tests/config_readers.rs`, `docs/CONFIG.md`
- `docs/SECURITY.md`, `docs/THREAT-MODEL.md`
- `app/l10n/*.arb`: der zweite Satz des Abschnitts „Aufbewahrung" im Audit-Screen (`auditRetentionChain`) sagt heute, der Schlüssel wirke nicht

### Spezifikation
Zu entscheiden: Wie ein Neuanfang aussieht, den `verify` von einem Bruch unterscheidet, und welche Anker dabei gelten. Wie `audit_anchors` gekürzt wird, ohne dass die Tabelle ihre Beweiskraft für die verbleibenden Records verliert. Ob die Löschung die Datei umschreibt oder rotiert.

### Schritte
1. Entscheidung Löschen oder Streichen, mit Begründung in `docs/SECURITY.md`.
2. Umsetzung oder Streichung samt Register und `docs/CONFIG.md`.
3. Satz im Audit-Screen anpassen.

### Tests
Bei Löschung: ein Log mit Records älter als die Frist, danach `verify` grün mit dokumentierter Lücke; eine Lücke ohne dokumentierenden Record bleibt ein Bruch. Bei Streichung: `CONFIG_005` als Warnung beim Laden.

### Akzeptanzkriterien
- [x] `audit.retention_days` ist im Register `effective` oder gestrichen, nie mehr `pending`. Entschieden 2026-09-23: löschen, nicht streichen (`backlog/CONVENTIONS.md` 4.40). `config_readers.rs` führt den Schlüssel als `effective`, `docs/CONFIG.md` ist neu erzeugt. Gebaut: täglicher Lauf im Daemon (`humanitld/src/audit_retention.rs`), `audit.pruned` samt Anker, `verify` erkennt den dokumentierten Anfang an. 13 Mutationen in `humanitl_audit` (Tests in `daemon/crates/audit/tests/retention.rs`), `humanitld`, `humanitl-ipc` und `humanitl-config`, jede rot gesehen: eine Lücke ohne dokumentierenden Record bleibt `SeqGap`, eine Kette, die nicht hält, wird nicht gekürzt.
- [x] `docs/SECURITY.md` sagt, was die Kette nach einer Löschung noch beweist: Abschnitt 8, „Aufbewahrung der Kette"; dazu K-14 in `docs/THREAT-MODEL.md`.
- [x] Der Abschnitt „Aufbewahrung" im Audit-Screen sagt dasselbe wie der Code: `auditRetentionChain` neu, Warnung `pruned` in der Statuskarte (`auditWarningPruned`). `audit_arb_test.dart` und `warnings_are_shown_in_amber` werden unter ihrer Mutation rot.
- [x] `make check` grün. Gemessen 2026-09-23 mit `STRICT=1 make check` im Arbeitsbaum dieses Issues, unter `bwrap --tmpfs /tmp`.

### Fallstricke
- Eine Löschung, die `verify` als Bruch meldet, macht jeden Audit-Screen dauerhaft rot.
- Die Anker in `audit_anchors` sind der Beleg gegen das Kürzen der Datei; wer sie mitlöscht, löscht den Beleg.

### Referenzen
HUM-050, HUM-051; `daemon/crates/audit/src/verify.rs`; `docs/SECURITY.md` („Was die Audit-Kette beweist").

---

## HUM-158 · Ein Export ohne freien Namen meldet `IPC_006`
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-051, HUM-070 · Blockiert: keine

### Kontext
Der Export im Audit-Screen (HUM-051) wählt im Ordner einen freien Dateinamen (`name`, `name-2` und so weiter, höchstens 1000 Versuche). Findet er keinen, meldet die Oberfläche `IPC_006` „Fähigkeit in diesem Daemon nicht verfügbar". Der Code passt nicht: Die Fähigkeit ist da, nur der Ordner ist voll. HUM-070 bringt `AUDIT_008` „Audit-Export nicht schreibbar", das zur Zeit von HUM-051 noch nicht auf `main` stand.

### Ziel
Ein Export ohne freien Namen meldet `AUDIT_008` mit `why` und dem Vorschlag, den Ordner anzusehen.

### Nicht-Ziel
Andere Fehler des Exports; die schreibt der Daemon (HUM-156).

### Betroffene Pfade
- `app/lib/features/audit/providers/audit_provider.dart` (`AuditExportNotifier`)
- `app/lib/core/domain/diagnostic_codes.dart`: Konstante für `AUDIT_008` anhängen

### Spezifikation
Der Befund im Zweig `path == null` von `AuditExportNotifier` trägt `AUDIT_008` statt `DiagnosticCodes.capabilityUnavailable`; `why` und `fix` bleiben.

### Schritte
1. Konstante anhängen, Zweig umstellen.
2. Test mit einem Ordner, in dem jeder Name belegt ist.

### Tests
`a_full_folder_reports_audit_008`: `auditPathTakenProvider` meldet jeden Pfad als belegt, der Export endet mit `AUDIT_008`, der Daemon wird nicht gefragt.

### Akzeptanzkriterien
- [x] Ein voller Ordner ergibt `AUDIT_008`, nicht `IPC_006`. Gemessen 2026-09-18, mitgebaut in HUM-156: `a_full_folder_reports_audit_008` (`app/test/features/audit/audit_screen_test.dart`) meldet jeden Pfad als belegt, der Export endet mit `AUDIT_008` samt `why` und `fix`, `auditExports` bleibt leer; mit `capabilityUnavailable` im Zweig wird der Test rot.
- [x] `make check` grün. Gemessen 2026-09-18 mit `STRICT=1 make check` im Arbeitsbaum von HUM-156.

### Fallstricke
- `AUDIT_008` muss im Register stehen, bevor die Oberfläche es benutzt (`backlog/CONVENTIONS.md` 4.6).

### Referenzen
HUM-051, HUM-070; `daemon/crates/core-types/src/diagnostics/codes.rs`.

---

## HUM-162 · Der Kopf der Kette hat über den Daemon keinen Zeitpunkt
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-156 · Blockiert: keine

### Kontext
HUM-070 zeigt den Kopf als `head: a3f9…c2e1 (seq 4213, 2026-09-02T10:42:01Z)`. Seit HUM-156 prüft `humanitl audit verify` über den Daemon, und `AuditResponse` trägt Hash und Nummer des Kopfes (`head_hash`, `head_seq`), aber keinen Zeitpunkt. Über den Daemon fehlt der Zeitpunkt deshalb in Text und JSON (`head.ts: null`); nur die Prüfung der Datei nennt ihn. HUM-156 durfte keine Felder anlegen (Nicht-Ziel).

### Ziel
`AuditResponse` trägt den Zeitpunkt des Kopfes im Format des Logs, `verify` und `head` füllen ihn, die Kommandozeile zeigt ihn wie in der Spezifikation von HUM-070, und die Karte des Audit-Screens kann ihn zeigen.

### Nicht-Ziel
Andere Felder am Kopf.

### Betroffene Pfade
- `proto/humanitl/v1/humanitl.proto`: `AuditResponse.head_ts` (nächste freie Nummer, `string`, dieselben Zeichen wie im Log); `PROTO_MINOR` +1
- `daemon/crates/audit/src/verify.rs` (`VerifyReport.head` mit Zeitpunkt), `daemon/crates/ipc/src/audit.rs`
- `daemon/bin/humanitl/src/cmd/audit.rs`

### Spezifikation
Als `string` und nicht als `Timestamp`, aus demselben Grund wie `AuditEntry.ts`: genau diese Zeichen stehen im Hash.

### Schritte
1. Feld, `make proto`.
2. Daemon füllt es für `verify` und `head`.
3. Kommandozeile liest es.

### Tests
`the_head_carries_its_time` (`daemon/crates/ipc/tests/audit_rpc.rs`), `audit_verify_asks_the_daemon_for_key_and_anchors` erweitert um `head.ts`.

### Akzeptanzkriterien
- [ ] `humanitl --json audit verify` gegen einen Daemon nennt `head.ts` gleich dem `ts` der letzten Zeile.
- [ ] `make check` grün.

### Fallstricke
- Ein Record, dessen `ts` nicht dem Format entspricht, ist in einer heilen Kette nicht möglich; der Kopf eines gebrochenen Logs trägt, was die Zeile trägt.

### Referenzen
HUM-070, HUM-156; `docs/PROTOCOL.md` 4 und 5.

---

## HUM-163 · Jede Seite der Audit-Tabelle liest die ganze Kette
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-156 · Blockiert: keine

### Kontext
HUM-156 beantwortet `Audit(Query)` und `Audit(Head)` aus der Datei: Jede Seite liest jede Zeile von `audit.jsonl`, parst sie als JSON und behält davon nur `limit + 1` Records. Der Speicher bleibt klein, die Zeit wächst mit der Kette. HUM-050 misst `verify` über 100 000 Records mit 3 s; eine Seite der Tabelle ist billiger als eine Prüfung, aber nicht gemessen, und die Tabelle blättert Seite um Seite. Bei einer Kette, die ein Jahr läuft, wird jedes Blättern so teuer wie ein Lesen der ganzen Datei.

### Ziel
Eine Seite der Tabelle kostet unabhängig von der Länge der Kette höchstens einen festen Anteil der Datei, gemessen mit einem Bench-Test über 100 000 Records.

### Nicht-Ziel
Ein Index in `SQLite`, der eine zweite Wahrheit neben der Kette wäre; die Datei bleibt die Quelle.

### Betroffene Pfade
- `daemon/crates/audit/src/query.rs`
- `daemon/crates/audit/tests/bench.rs`

### Spezifikation
Erst messen: Seite ohne Filter und mit Filter über 100 000 Records. Liegt eine Seite ohne Filter über 200 ms, von hinten lesen (die jüngsten Records stehen am Ende, eine Seite ohne Filter braucht nur das Ende; `writer.rs` liest schon so), und `entries` bei gesetztem Filter nur so weit zählen, wie die Oberfläche es braucht, oder ausdrücklich als Schätzung kennzeichnen.

### Schritte
1. Bench-Test (`#[ignore]`).
2. Je nach Messung: Lesen von hinten für Seiten ohne Filter.

### Tests
`a_page_over_100k_records_is_fast` (Bench), `query_pages_newest_first_with_a_cursor` bleibt grün.

### Akzeptanzkriterien
- [ ] Die Dauer einer Seite über 100 000 Records ist gemessen und steht hier.
- [ ] `make check` grün.

### Fallstricke
- Der Cursor ist eine Nummer, keine Position in der Datei; wer von hinten liest, darf sich auf aufsteigende Nummern nur verlassen, solange die Kette heil ist, und muss bei einem Bruch dasselbe liefern wie heute.

### Referenzen
HUM-050 (Bench), HUM-051 (Seiten von 200), HUM-156.

## HUM-159 · Die harte Sperre blockt, bevor jemand pseudonymisieren kann
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-049, HUM-156 · Blockiert: HUM-055

### Kontext
Aufgefallen bei HUM-049 (2026-09-18). Die Spezifikation von HUM-049 beschreibt die harte Sperre als Weigerung an der Freigabe: Die Anfrage mit einer IBAN wird gehalten, die Oberfläche zeigt „Senden nicht möglich", `Decide(Allow)` endet mit `HOLD_004`, und der Mensch kann pseudonymisieren. Gebaut ist seit HUM-023 etwas Strengeres: Mit `hold.hard_block_checksum_secrets = true` blockt `FlowHandler::analyze` eine solche Anfrage sofort als System (`BlockReason::Secret`), sie wird nie gehalten. HUM-049 hat diesen Weg behalten und ihm `HOLD_004` als Befund mitgegeben, dazu dieselbe Prüfung für eine bearbeitete Fassung. Was fehlt, ist der Weg über den Editor: Wer den Schalter setzt, kann eine Anfrage mit IBAN nicht mehr pseudonymisiert hinausschicken, der Agent bekommt nur `403`.

### Ziel
Mit dem Schalter wird eine Anfrage mit bestätigtem Geheimnis gehalten, solange keine Regel sie freigibt oder blockt. `Decide(Allow)` auf sie endet über gRPC mit `HOLD_004`, der Flow bleibt gehalten, `AllowEdited` ohne das Geheimnis geht durch. Eine Regel `allow` oder die Durchreiche zum Sprachmodell geben eine solche Anfrage nie frei: Sie blockt weiter sofort. Die Oberfläche liest den Schalter und zeigt „Senden nicht möglich" mit dem `why` von `HOLD_004` als Tooltip; die Pause zeigt „Trotzdem senden" dann nicht.

### Nicht-Ziel
Die Allowlist aus HUM-049 (nach dem MVP, BACKLOG.md Abschnitt 9, Punkt 15).

### Betroffene Pfade
- `daemon/crates/proxy/src/handler.rs` (`analyze`), `daemon/crates/proxy/src/pipeline.rs` (Regel-Freigabe bei bestätigtem Geheimnis)
- `daemon/crates/ipc/src/server.rs` (`decide_one`: Prüfung vor `Allow`, Befund statt `IPC_003`); erst nach HUM-156, das dieselbe Datei umbaut
- `app/lib/core/ipc/`: `GetConfig` als Dart-Aufruf oder ein Feld am Flow, das die Sperre ansagt; `action_bar.dart`, `findings_pause.dart`
- `app/lib/core/ipc/fake_daemon_client.dart`: `HOLD_004`, wenn ein Schalter gesetzt ist

### Spezifikation
`humanitl_proxy::findings::check_allow` (HUM-049) ist die eine Prüfung; `decide_one` ruft sie mit den Funden aus der Registry. Ob die Oberfläche den Schalter über `GetConfig` liest oder der Daemon am Flow ansagt, dass Senden gesperrt ist, entscheidet dieses Issue; das zweite braucht kein Wissen über die Konfiguration im Client und ist deshalb vorzuziehen (ADR-018).

### Schritte
1. Halten statt blocken, Regel-Freigabe bleibt Block; Tests im Proxy.
2. `decide_one` mit `HOLD_004`; Test über den gRPC-Client.
3. Oberfläche und Fake.

### Tests
`allow_with_checksum_secret_refused_over_grpc` (`HOLD_004`, Flow bleibt gehalten), `allow_edited_without_the_secret_goes_out`, `rule_allow_still_blocks_a_checksum_secret`, Widget `hard_block_hides_send_anyway`.

### Akzeptanzkriterien
- [ ] Mit Schalter und gültiger IBAN: „Senden nicht möglich", `Decide(Allow)` über gRPC endet mit `HOLD_004`, der Flow bleibt gehalten.
- [ ] Pseudonymisiert geht dieselbe Anfrage hinaus.
- [ ] Eine Regel `allow` lässt eine solche Anfrage nie hinaus.

### Fallstricke
- Eine Regel oder die Durchreiche darf die Sperre nie umgehen; heute schützt die sofortige Sperre auch diesen Weg, und wer auf Halten umstellt, muss ihn eigens schließen.
- `docs/SECURITY.md` nennt den Schalter; die Aussage „geht nicht ungeändert hinaus" muss danach weiter stimmen.

### Referenzen
HUM-049 (Spezifikation und Stand), HUM-023, HUM-156.

---

## HUM-160 · „Trotzdem senden" hinterlässt keine Spur
Sprint: 4 · Größe: M · Abhängigkeiten: HUM-049, HUM-050 · Blockiert: HUM-055

### Kontext
Aufgefallen bei HUM-049 (2026-09-18). „Trotzdem senden" in der Pause schickt heute ein gewöhnliches `Decide(Allow)`. Dass der Mensch die offenen Funde gesehen und bewusst gesendet hat, steht nirgends: nicht im Proto (`acknowledged_findings = 8` ist nur in der Spezifikation von HUM-049 notiert), nicht in `findings.resolved` (bleibt `NULL`), nicht im `Decided`-Ereignis und nicht im Audit. Das Kriterium „History zeigt `unresolved_findings = 1`" aus HUM-049 ist deshalb offen.

Das eigentliche Loch ist `AllowEdited`: Der Daemon scannt die bearbeitete Fassung ein zweites Mal (`edit::remaining_findings`) und kennt damit die Funde, mit denen sie hinausgeht, schreibt sie aber nirgends hin; im Audit steht nur, dass bearbeitet freigegeben wurde. Wer später prüft, ob eine Anfrage mit offenen Funden den Rechner verlassen hat, kann es für eine bearbeitete nicht sagen.

### Ziel
`DecideRequest.acknowledged_findings` (Feld 8) trägt die Indizes der bestätigten Funde. Der Daemon schreibt sie als `resolved = 'acknowledged'`, das `Decided`-Ereignis trägt `unresolved_findings`, das Audit `flow.decided` trägt `unresolved_findings` und `acknowledged`, und die History zeigt die Zahl. Bei `AllowEdited` ist `unresolved_findings` die Zahl aus dem zweiten Scan der bearbeiteten Fassung, nicht die der gehaltenen.

### Nicht-Ziel
`ignore_always` und `allowlisted` (nach dem MVP).

### Betroffene Pfade
- `proto/humanitl/v1/humanitl.proto` (`PROTO_MINOR` anheben, `docs/PROTOCOL.md`)
- `daemon/crates/ipc/src/validate.rs`, `daemon/crates/ipc/src/server.rs`
- `daemon/crates/recorder/src/writer.rs`, `daemon/crates/recorder/src/query.rs`
- `daemon/crates/audit/src/kinds.rs` (`unresolved_findings` ist dort schon vorgesehen)
- `app/lib/features/intercept/providers/decision.dart`, `app/lib/features/history/`

### Spezifikation
Wie HUM-049, Abschnitt „Daemon", ohne Allowlist. Die Indizes beziehen sich auf die Reihenfolge im `Analyzed`-Ereignis; ein Index außerhalb wird mit `IPC_002` abgelehnt.

### Schritte
1. Proto, Validierung, Handler.
2. Recorder und Audit.
3. Oberfläche: die Pause schickt die Indizes aller offenen Funde, die History zeigt die Zahl.

### Tests
`send_anyway_acknowledges_all` (Widget), `acknowledged_findings_are_recorded` (Recorder), `decided_carries_unresolved_findings` (Proxy).

### Akzeptanzkriterien
- [x] „Trotzdem senden" leitet weiter; History zeigt `unresolved_findings = 1` für eine Anfrage mit einer E-Mail, die über das Halten gesendet wurde, und `0` nach der Pause. Gemessen: `decided_carries_unresolved_findings` (`daemon/crates/proxy/tests/findings.rs`, echter Proxy mit Aufzeichnung: Halten `1`, Pause `0` in `Decided`, Registry und `flows.unresolved_findings`, Status 200); Oberfläche `send_anyway_acknowledges_all`, `over the valve the history counts 1 unresolved`, `after the pause the history counts 0 unresolved` (`app/test/features/intercept/findings_pause_test.dart`), `history_shows_unresolved_findings` (`app/test/features/history/history_detail_test.dart`), `the row takes the count of open findings from the decision` (`history_page_test.dart`). `STRICT=1 make check` grün am 2026-09-19.
- [x] Das Audit trägt beide Zahlen. Gemessen: `flow_decided_carries_unresolved_and_acknowledged` (`daemon/bin/humanitld/src/audit_sink.rs`): Pause `0`/`1`, Halten `1`/`0`, Block ohne beide.
- [x] Eine bearbeitet freigegebene Anfrage, in der eine E-Mail stehen blieb, trägt im Audit und in der History `unresolved_findings = 1`. Gemessen: `decided_carries_unresolved_findings`, Fall `Release::Edited` (zweiter Scan: stehengelassene Adresse `1`, ersetzte `0`, in `Decided` und `flows.unresolved_findings`); das Audit schreibt `unresolved_findings` aus demselben `Decided` (`FlowDecided::with_findings`, Sink-Test oben).

### Fallstricke
- Eine Bestätigung hebt `HOLD_004` nie auf (HUM-049, `check_allow`).

### Referenzen
HUM-049, HUM-050, HUM-089.

---

## HUM-161 · Der Editor sendet verbliebene Funde ohne Rückfrage
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-047, HUM-049 · Blockiert: keine

### Kontext
Aufgefallen bei HUM-049 (2026-09-18). Die Tabelle der Knopfzustände in HUM-049 verlangt für „Editierte Version senden" dieselbe Findings-Logik auf den verbliebenen Funden des Entwurfs. Gebaut ist die Pause nur in der Aktionsleiste; der Editor sendet einen Entwurf mit offenen Funden ohne Rückfrage. Nur die harte Sperre greift dort, im Daemon.

### Ziel
Hat der Entwurf offene Funde, heißt der Knopf „Editierte Version senden mit n Findings", und ein Klick öffnet dieselbe Pause über den Funden des Entwurfs, nicht über denen der gehaltenen Fassung.

### Nicht-Ziel
Ein zweiter Scan im Client; die Funde des Entwurfs führt der Editor schon (`Draft`).

### Betroffene Pfade
- `app/lib/features/editor/editor_screen.dart`, `app/lib/features/intercept/widgets/findings_pause.dart` (nach `core/ui`, wenn beide Features sie brauchen)

### Spezifikation
Die Pause bekommt die offenen Funde des Entwurfs; „Pseudonymisieren" heißt dort „Alle ersetzen".

### Schritte
1. Pause nach `core/ui` heben, damit kein Feature das andere importiert.
2. Editor: Zustand und Pause.

### Tests
`editor_send_with_open_findings_opens_pause`, `editor_send_without_findings_sends`.

### Akzeptanzkriterien
- [ ] Ein Entwurf mit einer stehengelassenen E-Mail sendet nicht auf den ersten Klick.

### Fallstricke
- Die Indizes des Entwurfs sind andere als die des `Analyzed`-Ereignisses (HUM-049, Fallstricke).

### Referenzen
HUM-047, HUM-049.

## HUM-164 · Ein Client weckt den Daemon über den Socket nicht
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-053 · Blockiert: keine

### Kontext
Seit HUM-053 hält `humanitld.socket` den gRPC-Socket, und der Daemon übernimmt ihn (`LISTEN_FDS`). Der eigentliche Nutzen der Socket-Aktivierung, ein Dienst, der erst beim ersten Client startet, greift trotzdem nicht: `humanitl_ipc::client::connect` liest zuerst das Token aus `$XDG_RUNTIME_DIR/humanitl/token` und öffnet den Socket erst danach, und das Token schreibt der laufende Daemon. Ohne laufenden Daemon gibt es kein Token, ohne Token keine Verbindung, ohne Verbindung keinen Start. `humanitl daemon install` aktiviert deshalb Socket **und** Dienst, und der Dienst startet mit jeder Sitzung. Die Oberfläche liest das Token ebenso vorab (`app/lib/core/ipc/`).

### Ziel
Ein Client, der kein Token findet, öffnet den Socket trotzdem einmal, wartet kurz auf das Token und versucht es dann erneut. Danach genügt `enable --now humanitld.socket`, und der Dienst startet beim ersten Client.

### Nicht-Ziel
Ein Token über den Socket auszuliefern; das Token bleibt eine Datei mit `0600`.

### Betroffene Pfade
- `daemon/crates/ipc/src/client.rs`
- `app/lib/core/ipc/` (Verbindungsaufbau)
- `daemon/bin/humanitl/src/cmd/unit.rs` (`SystemUnits::names`), `docs/INSTALL.md`

### Akzeptanzkriterien
- [ ] Unter `systemd-socket-activate` ohne laufenden Daemon endet `humanitl daemon status` mit Exit 0.
- [ ] `daemon install` aktiviert beim Paket nur noch den Socket; `docs/INSTALL.md` sagt es.
- [ ] `make check` grün.

### Fallstricke
- Ein Client, der auf das Token wartet, braucht eine Frist; sonst hängt `daemon status` an einem Socket, hinter dem kein Dienst mehr startet.

---

## HUM-165 · Der Daemon aus dem AppImage findet Katalog und Sandbox-Profil nicht
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-053, HUM-070 · Blockiert: keine

### Kontext
`humanitl daemon install` aus einem AppImage kopiert nur `humanitld` und `humanitl-shim` nach `~/.local/lib/humanitl/<version>.<stempel>/` (HUM-070). Der Daemon sucht den Domain-Katalog aber relativ zu seinem eigenen Pfad (`catalog_dir` in `daemon/bin/humanitld/src/main.rs`: `<exe>/../../share/humanitl/catalog`) und das Profil `default` ebenso (`tree_dirs` in `daemon/crates/ipc/src/sandbox.rs`: `<vorfahre>/profiles/sandbox`). Im Bild liegen beide unter `usr/lib/humanitl/` (HUM-053, `packaging/appimage/build-appimage.sh`), in der Kopie fehlen sie. Der kopierte Daemon läuft dann mit leerem Katalog und ohne Sandbox-Profil. Aufgefallen beim Bau des AppImage in HUM-053, nicht gemessen.

### Ziel
Die Kopie ist vollständig: `share/humanitl/catalog/` und `profiles/sandbox/default.toml` liegen neben `bin/`, im selben Aufbau wie im Archiv, und der kopierte Daemon findet beide.

### Betroffene Pfade
- `daemon/bin/humanitl/src/cmd/daemon.rs` (`stage`, `STAGED_BINARIES`)
- `daemon/bin/humanitl/tests/cli.rs` (`daemon_install_appimage_copies_binaries`)

### Akzeptanzkriterien
- [ ] Nach `--cli daemon install` aus dem AppImage meldet der Daemon beim Start einen Katalog mit Einträgen und startet eine Sandbox mit dem Profil `default`.
- [ ] `make check` grün.

---

## HUM-166 · `humanitl_ipc::serve` und `systemd::serve` sind zwei Fassungen desselben Dienstes
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-053, HUM-156 · Blockiert: keine

### Kontext
HUM-053 brauchte einen gRPC-Dienst auf einem übergebenen Socket, der die Datei am Ende liegen lässt und nach dem Binden `READY=1` meldet. `humanitl_ipc::serve` bindet selbst und hat dafür keine Naht, und `daemon/crates/ipc/src/server.rs` war zur selben Zeit Arbeitsgebiet von HUM-156. `daemon/bin/humanitld/src/systemd.rs` enthält deshalb eine zweite Fassung von Token, Interceptor, Signal und Frist (`drain`). Zwei Fassungen laufen auseinander.

### Ziel
`humanitl_ipc` bekommt `serve_on(listener, owns_socket, on_ready)`, `serve` ruft es nach dem Binden auf, und `systemd::serve` schrumpft auf die Wahl des Listeners und die Meldung an systemd.

### Betroffene Pfade
- `daemon/crates/ipc/src/server.rs`, `daemon/crates/ipc/src/lib.rs`
- `daemon/bin/humanitld/src/systemd.rs`

### Akzeptanzkriterien
- [ ] Token schreiben, Interceptor, Frist und Aufräumen stehen an genau einer Stelle.
- [ ] Die Tests aus `daemon/bin/humanitld/tests/socket_activation.rs` bleiben grün.
- [ ] `make check` grün.

---

## HUM-167 · Eine alte Nutzer-Unit verdeckt die Units des Pakets
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-053 · Blockiert: keine

### Kontext
Wer Humanitl erst aus dem Archiv oder dem AppImage installiert hat, hat `~/.config/systemd/user/humanitld.service` mit der Marke von `daemon install`. Installiert er danach das Paket, verdeckt diese Datei die Unit unter `/usr/lib/systemd/user/` (`systemd.unit(5)`). `daemon install` erkennt seit HUM-053 die Units des Pakets und aktiviert sie, sieht aber nicht nach, ob eine eigene ältere Kopie darüber liegt; gestartet wird dann der alte Daemon mit der ungehärteten Unit. Dazu kommt: Die Tests von `daemon install` in `daemon/bin/humanitl/tests/cli.rs` nehmen an, dass `/usr/lib/systemd/user/humanitld.service` fehlt; auf einem Rechner mit installiertem Paket liefen sie den Weg des Pakets.

### Ziel
Im Weg des Pakets meldet `daemon install` eine eigene Kopie (erste Zeile ist die Marke) unter `~/.config/systemd/user/`, kündigt an, sie beiseitezulegen, und tut das mit derselben Rücknahme wie beim Schreiben. Eine fremde Kopie ohne Marke bleibt liegen und ergibt einen Befund mit `CopyCommand`. Das Verzeichnis der Paket-Units lässt sich für Tests setzen, ohne den Befehl für Menschen zu ändern.

### Betroffene Pfade
- `daemon/bin/humanitl/src/cmd/daemon.rs` (`install_packaged`), `daemon/bin/humanitl/src/cmd/unit.rs`
- `daemon/bin/humanitl/tests/cli.rs`

### Akzeptanzkriterien
- [ ] Mit Paket und eigener alter Kopie startet nach `daemon install` die Unit des Pakets.
- [ ] Eine fremde Kopie wird nie angefasst.
- [ ] `make check` grün.

---

## HUM-168 · Ein Rückfall für Impeller fehlt im Release-Bau
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-053 · Blockiert: keine

### Kontext
HUM-053 verlangt, den Rückfall `--no-enable-impeller` zu dokumentieren. Der Linux-Runner (`app/linux/runner/my_application.cc`) reicht seine Argumente als Dart-Einstiegsargumente weiter, nicht an die Engine, und die Umgebungsvariablen der Engine (`FLUTTER_ENGINE_SWITCHES`) gelten nach ihrem Quelltext nur außerhalb von Release-Bauten. `docs/INSTALL.md` sagt deshalb, dass es heute keinen Rückfall gibt, statt einen Schalter zu nennen, der nichts tut. `DOCTOR_010` erkennt den bekannten schwarzen Bildschirm (NVIDIA unter Wayland) und verweist auf eine Dokumentation, die diesen Weg noch nicht hat. Nicht gemessen in HUM-053.

### Ziel
Gemessen, ob der Release-Bau Impeller abschalten kann, und wenn ja wie; der Weg steht in `docs/INSTALL.md` und im Fix von `DOCTOR_010`. Wenn nein, reicht der Runner einen eigenen Schalter an die Engine weiter (`FlDartProject` bzw. die Engine-Argumente).

### Betroffene Pfade
- `app/linux/runner/`
- `docs/INSTALL.md`, `daemon/crates/sandbox/src/doctor/` (Fix von `DOCTOR_010`)

### Akzeptanzkriterien
- [ ] Unter Xvfb meldet der Release-Bau mit dem dokumentierten Weg ein anderes Rendering-Backend als „Impeller" (dieselbe Zeile, die `packaging/deb/check-install.sh` liest).
- [ ] `make check` grün.

---

## HUM-171 · Tests lassen ihre Verzeichnisse in `/tmp` liegen
Sprint: 4 · Größe: S · Abhängigkeiten: — · Blockiert: —

### Kontext
Am 2026-09-19 wurde `tools/verify-commit.sh` über den Merge von HUM-053 rot,
obwohl der Diff nichts damit zu tun hatte: `report::tests::the_socket_walk_finds_a_socket_within_its_bounds`
im Shim legt einen Socket direkt unter `/tmp` an und erwartet, dass der
Suchlauf ihn findet. Der Suchlauf bricht nach `SOCKET_WALK_MAX_ENTRIES` (2000)
Einträgen ab, und in `/tmp` lagen über 4300. Gut 1700 davon waren
`humanitl-ui-state*` aus `app/test/harness/ui_state.dart`
(`Directory.systemTemp.createTempSync`, nie gelöscht) und gut 500
`coupling-*` aus `tools/tests/check_coupling_test.py` (`tempfile.mkdtemp`,
nie gelöscht). Jeder Lauf der Suite lässt also Verzeichnisse zurück, und ab
einer gewissen Menge wird ein Test rot, der mit ihnen nichts zu tun hat.

### Ziel
Die beiden Helfer räumen ihre Verzeichnisse selbst weg, und der Test des
Suchlaufs hängt nicht mehr davon ab, wie voll `/tmp` gerade ist.

### Akzeptanzkriterien
- [ ] Nach `flutter test` und `python3 tools/tests/check_coupling_test.py` liegt in `/tmp` kein neues `humanitl-ui-state*` und kein neues `coupling-*`.
- [ ] Der Test des Socket-Suchlaufs ist grün, auch wenn `/tmp` mehr als 2000 Einträge hat (gemessen mit einem Baum aus 2500 leeren Verzeichnissen, zum Beispiel unter `bwrap --tmpfs /tmp`).
- [ ] Mutationsprobe: ohne das Aufräumen bleibt ein Verzeichnis liegen, und ein Test sagt es.
- [ ] `make check` grün.

### Fallstricke
- Der Suchlauf selbst soll seine Grenze behalten; sie schützt die Sandbox vor einem riesigen Baum. Zu ändern ist der Test, nicht die Grenze.

### Referenzen
HUM-053 (der Lauf, an dem es auffiel); `daemon/bin/humanitl-shim/src/report.rs`, `app/test/harness/ui_state.dart`, `tools/tests/check_coupling_test.py`.

---

## HUM-185 · Der Bildschirm-Test gegen den echten Daemon fällt in CI zufällig aus
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-144 · Blockiert: —

### Kontext
Der CI-Schritt „The app on a screen against a real daemon (HUM-144)" im Job
`e2e-xvfb` (`make flutter-test-integration` unter Xvfb) war zweimal rot, ohne
dass der Commit etwas daran geändert hätte: am 2026-09-18 über `a27962e`
(HUM-047, Lauf 35383298554) und am 2026-09-19 über `f9dff18` (HUM-053, Lauf
35412854835). Beide Male liefen dieselben Tests lokal unter Xvfb grün
(`queue_freeze` und `shell_test`), und die Läufe danach waren in CI wieder
grün. Das Log des Jobs ist ohne Admin-Rechte nicht lesbar; welcher der beiden
Tests fiel und woran, ist deshalb nicht bekannt. Der Kandidat ist
`queue_freeze` (`no_row_moves_under_the_pointer_while_fifteen_flows_arrive`),
weil er gegen die Zeit misst.

### Ziel
Der Schritt ist in CI stabil, oder er sagt bei einem Ausfall genug, dass man
die Ursache aus dem Artefakt lesen kann.

### Akzeptanzkriterien
- [ ] Das Artefakt des Jobs enthält bei einem Ausfall die Ausgabe des Tests und das Log des Daemons.
- [ ] Die Ursache ist benannt und behoben, oder der Test wartet auf Zustände statt auf Zeit.
- [ ] Zwanzig Läufe des Schritts hintereinander lokal unter Last (`stress -c 6` oder ein paralleler Bau) sind grün.
- [ ] `make check` grün.

### Referenzen
HUM-144; `app/integration_test/`, `.github/workflows/ci.yml` (Job `e2e-xvfb`).

---

## HUM-169 · `flows watch`: der Ereignisstrom auf der Kommandozeile
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-078 · Blockiert: keine

### Kontext
HUM-078 ordnet jeder RPC ein Unterkommando zu. `Subscribe` hat heute nur `humanitl run --ask terminal` als Client auf der Kommandozeile: die Moderation liest den Strom, zeigt aber nur gehaltene Anfragen. Wer den Strom beobachten will, ohne selbst eine Sitzung zu starten, hat keinen Weg; die Fallstricke von HUM-078 verlangen `flows watch` ausdrücklich.

### Ziel
`humanitl flows watch [--passthrough] [--since ID] [--json]` schreibt jedes `FlowEvent` als eine Zeile, bis Ctrl+C oder der Daemon den Strom beendet.

### Nicht-Ziel
Kein Entscheiden aus dem Strom heraus; dafür gibt es `flows decide` und `--ask terminal`.

### Betroffene Pfade
- `daemon/bin/humanitl/src/cli.rs` (`FlowsCmd::Watch`)
- `daemon/bin/humanitl/src/cmd/flows.rs`
- `daemon/bin/humanitl/src/parity.rs` (Zeile `("Humanitl.Subscribe", "flows watch")`)
- `docs/reference/parity.md` (neu erzeugt)

### Spezifikation
Textform: Zeitstempel, Art des Ereignisses, Flow-Id, eine kurze Angabe je Art (Host und Pfad bei `Received`, Entscheidung bei `Decided`, Code bei `Diagnostic`). Unter `--json` ein Objekt je Zeile mit `at`, `kind`, `flow_id` und den Feldern der Art; `summary_json` aus `cmd/flows.rs` für `Received`. `Lagged` wird als Befund gemeldet, nicht verschwiegen.

### Schritte
1. Unterkommando und Ausgabe, Tests gegen den Fake-Daemon.
2. `PARITY` ergänzen, `cargo xtask docs`.

### Tests
- `flows_watch_prints_one_line_per_event` gegen den Fake-Daemon.
- `flows_watch_json_is_one_object_per_line`.

### Akzeptanzkriterien
- [ ] `flows watch` zeigt jedes Ereignis des Stroms.
- [ ] `docs/reference/parity.md` nennt `flows watch` bei `Humanitl.Subscribe`.

### Fallstricke
- Ctrl+C muss den Strom sauber schließen; Exit 0, nicht 130, wenn der Nutzer das Beobachten beendet.

### Referenzen
ADR-018; HUM-078; `backlog/CONVENTIONS.md` 4.35.

---

## HUM-170 · `GetConfig` und `GetSessionSummary` haben keinen Ort in der Oberfläche
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-078 · Blockiert: keine

### Kontext
`cargo xtask docs` warnt seit HUM-078 bei zwei RPCs, die die Oberfläche nicht aufruft: `GetConfig` (die aufgelöste Konfiguration mit Herkunft je Feld) und `GetSessionSummary` (was ein Sandbox-Lauf im Projektverzeichnis hinterlassen hat, HUM-043). ADR-018 gibt der Oberfläche dafür höchstens einen Sprint Rückstand auf die Kommandozeile.

### Ziel
Beide RPCs haben einen Ort in der Oberfläche und eine Zeile in `app/lib/core/parity.dart`; `cargo xtask docs` warnt nicht mehr.

### Nicht-Ziel
Kein neuer Einstellungsbildschirm über das hinaus, was die Spezifikation des Settings-Bildschirms ohnehin vorsieht.

### Betroffene Pfade
- `app/lib/core/ipc/daemon_client.dart` und die beiden Clients (`getConfig`, `getSessionSummary`)
- der Settings-Bildschirm (Konfiguration mit Herkunft) und der Sandbox-Bildschirm (Zusammenfassung nach dem Lauf)
- `app/lib/core/parity.dart`, `docs/reference/parity.md`

### Spezifikation
`getConfig` liefert die Werte samt `Origin`, die der Settings-Bildschirm neben jedem Feld zeigt. `getSessionSummary` wird nach dem Ende eines Laufs im Sandbox-Bildschirm gezeigt, mit denselben Angaben wie `humanitl sessions summary`.

### Schritte
1. Methoden im `DaemonClient`, Fake und gRPC.
2. Anzeige in den beiden Bildschirmen, ARB-Schlüssel `en` und `de`.
3. Registry ergänzen, `cargo xtask docs`.

### Tests
Widget-Tests für beide Anzeigen gegen den Fake-Client.

### Akzeptanzkriterien
- [ ] `cargo xtask docs --check` läuft ohne Warnung.
- [ ] `docs/reference/parity.md` hat keinen Abschnitt „UI-Lücken" mehr mit Einträgen.

### Fallstricke
- `app/lib/core/ipc/fake_daemon_client.dart` ist eine gemeinsam genutzte Datei (CLAUDE.md): nur anhängen.

### Referenzen
ADR-018; HUM-043; HUM-078; `backlog/CONVENTIONS.md` 4.35.

---

## HUM-190 · `AGENT_004` zeigt einen PATH, den der Bildschirm zurückhält
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-139, HUM-137 · Blockiert: —

### Kontext
Beim Bau von HUM-137 aufgefallen. `AGENT_005` nennt den `PATH` der Sandbox nur, wenn auch die Umgebungstabelle ihn zeigt (`sandbox_path_of` in `daemon/crates/ipc/src/sandbox.rs`, Regel aus `backlog/CONVENTIONS.md` 4.17): Steht er in `sandbox.env` oder in einem eigenen Profil, ersetzt ihn `<withheld>`. `AGENT_004` aus der Vorprüfung (HUM-139, `daemon/crates/sandbox/src/agent/opencode.rs`, `AgentContext::sandbox_path_display`) schreibt denselben Wert ohne diese Prüfung in `why` und in den Vorschlag `ChangeSetting` auf `sandbox.env.PATH`. Zwei Befunde reden über dieselbe Zeile und behandeln sie verschieden; der Bildschirm zeigt einen Wert in der Karte, den er zwei Reiter weiter als zurückgehalten ausweist.

### Ziel
Beide Befunde folgen einer Regel. Entweder gilt `PATH` als Wert, der unabhängig von seiner Herkunft gezeigt werden darf (dann steht die Ausnahme in 4.17 und in `VISIBLE_ENV`, und `sandbox_path_of` fällt weg), oder `AGENT_004` hält ihn zurück wie `AGENT_005`.

### Akzeptanzkriterien
- [ ] Die Entscheidung steht in `backlog/CONVENTIONS.md` 4.17.
- [ ] Ein Test belegt für `AGENT_004` und `AGENT_005` dasselbe Verhalten bei einem `PATH` aus `sandbox.env`, mit Mutationsprobe.
- [ ] `make check` grün.

### Referenzen
HUM-137; HUM-139; `backlog/CONVENTIONS.md` 4.17 und 4.39.

---

## HUM-197 · Im schmalen History-Detail bleibt dem Body weiter kaum Platz
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-153 · Blockiert: nichts; der Body ist erreichbar, nur nicht auf einen Blick

### Kontext
HUM-153 stellt den Rumpf neben Kopf, Tabs und Kopfzeilen, sobald das Detail
mindestens doppelt so breit ist wie das Textmaß von `mono12` (90 Zeichen, rund
648 px, also ab rund 1296 px). Darunter bleibt alles untereinander, wie vorher.
Gemessen wurde das Ziel nur bei 1400 × 900. Ein Fenster von 1280 px Breite mit
der Icon-Rail daneben liegt unter der Schwelle, und dort bekommt der Rumpf bei
900 px Höhe weiter nur knapp hundert Pixel: Titel und Umschalter, kein Inhalt.
Dasselbe gilt bei `TextScaler.linear(2.0)` in jeder Fensterbreite, weil das
Textmaß mit der Schrift wächst.

### Ziel
Wer im Verlauf eine Anfrage öffnet, sieht auch bei 1280 × 800 den Anfang ihres
Bodys ohne zu scrollen.

### Nicht-Ziel
Eine andere `BodyView`. Die Aufteilung zwischen Tabelle und Detail als Ganzes
neu zu entwerfen.

### Akzeptanzkriterien
- [ ] Ein Golden bei 1280 × 800 (mit Icon-Rail) zeigt den Titel des Bodys und mindestens fünf Zeilen des JSON-Baums.
- [ ] Die Anordnung bei 1400 × 900 aus HUM-153 bleibt unverändert (`history_detail_body_tree_*`).

### Fallstricke
- Die Kopfzeilen einzuklappen spart Platz, versteckt aber `content-encoding`
  und `content-type`, nach denen der Rumpf ausgepackt wird. Wer das tut, zeigt
  beide weiter an.

### Referenzen
HUM-153; `app/lib/features/history/history_detail.dart`, `docs/UX.md` 3.2.

---

## HUM-198 · Die History des Fakes nennt eine Anfragegröße, die ihr Rumpf nicht hat
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-032 · Blockiert: —

### Kontext
Gefunden bei HUM-153. Der Recorder schreibt `request_size` aus der Größe des
Anfrage-Rumpfs (`daemon/crates/recorder/src/writer.rs`, „erst hier stimmen
`FlowSummary::request_size` und die Sortierung nach Größe"). Der Fake im
History-Szenario setzt die Zeile dagegen auf `_requestSize` (`180 + (index % 17) * 64`)
und legt einen Rumpf von rund siebzig Bytes daneben
(`app/lib/core/ipc/fake_daemon_client.dart`, `_SeededFlow`). Im Detail steht
dadurch im Kopf eine andere Anfragegröße als im Titel des Rumpfs darunter,
bei Zeile 10 des Szenarios 756 B gegen 75 B, ein Widerspruch, den `backlog/CONVENTIONS.md` 4.13 verbietet und den jedes
History-Golden festhält.

### Ziel
Anfragegröße der Zeile und Größe des Rumpfs im History-Szenario sind dieselbe
Zahl, so wie beim echten Daemon.

### Akzeptanzkriterien
- [ ] Ein Test prüft für jede Zeile des Szenarios, dass `Flow.requestSize` gleich `FlowDetail.request.body.size` ist (bei einer bearbeiteten Anfrage gleich der Größe der bearbeiteten Fassung).
- [ ] Die Sortierung nach Größe unterscheidet weiterhin Zeilen (die Rümpfe bekommen dafür wechselnde Längen, nicht die Zeile eine erfundene Zahl).
- [ ] Die betroffenen History-Goldens sind neu erzeugt, und jede geänderte PNG ist im Commit begründet.

### Fallstricke
- `app/lib/core/ipc/fake_daemon_client.dart` ist eine gemeinsam genutzte Datei (CLAUDE.md): nur den eigenen Abschnitt ändern.

### Referenzen
HUM-032, HUM-153; `backlog/CONVENTIONS.md` 4.13.

---

## HUM-194 · Das Audit-Log hat keine Obergrenze in Bytes
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-157 · Blockiert: —

### Kontext
Seit HUM-157 löscht `audit.retention_days` den Anfang der Kette nach Alter. Eine
Frist in Tagen begrenzt aber keinen vollen Tag: Ein Agent, der viele Anfragen
stellt, schreibt je Flow mehrere Records, und die Datei wächst bis zum nächsten
täglichen Lauf ohne Grenze. Neben jede Aufbewahrung gehört eine Grenze in Bytes
samt Reihenfolge des Löschens und dem Verhalten bei fast voller Platte.

### Ziel
Ein Schlüssel `audit.max_bytes` (Vorgabe 0, keine Grenze), den derselbe Lauf wie
`audit.retention_days` liest: Ist die Datei größer, schneidet er vorn so viel,
dass sie darunter liegt, mit demselben `audit.pruned` als Beleg.

### Akzeptanzkriterien
- [ ] Ein Log über der Grenze ist nach dem Lauf darunter, und `verify` hält mit der Warnung `pruned`.
- [ ] Eine Kette, die nicht hält, wird auch hier nicht gekürzt (`AUDIT_001`).
- [ ] `make check` grün.

### Referenzen
HUM-157; `daemon/crates/audit/src/retention.rs` (`find_cut`).

---

## HUM-195 · `AuditWarning` nennt im Vertrag nur zwei Arten
Sprint: 4 · Größe: S · Abhängigkeiten: HUM-157, HUM-160 · Blockiert: —

### Kontext
HUM-157 hat die Warnung `pruned` hinzugefügt. Der Kommentar an
`AuditWarning` in `proto/humanitl/v1/humanitl.proto` nennt nur `no_hmac_key`
und `unanchored_tail`, und dass `records` bei `pruned` die Nummer des letzten
gelöschten Records trägt, steht nur in `backlog/CONVENTIONS.md` 4.40. Der
Vertrag wurde in HUM-157 nicht angefasst, weil HUM-160 ihn gleichzeitig auf
1.13 hebt.

### Akzeptanzkriterien
- [ ] Der Kommentar nennt alle drei Arten und die Bedeutung von `records` je Art; die erzeugten Dateien sind nachgezogen.
- [ ] `make check` grün.

### Referenzen
HUM-157; `daemon/crates/ipc/src/audit.rs` (`verify_response`).

