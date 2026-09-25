/// Der Entwurf einer bearbeiteten Anfrage (HUM-047).
///
/// Ein [Draft] ist das, was im Editor steht, und nichts sonst: Werte, keine
/// Widgets, kein Provider, kein IO. Alles, was daran geändert wird, geht durch
/// die reinen Funktionen in `draft_ops.dart`; der Provider hält nur den
/// jeweils letzten Stand.
///
/// # Zwei Offset-Räume, und warum hier der zweite gilt
///
/// Der Daemon zählt Bytes. `Finding.span` ist ein `Range<usize>` über den
/// dekodierten Bytes der Anfrage (`backlog/CONVENTIONS.md` 3.2). Dart zählt
/// UTF-16-Code-Units: `'ä'.length` ist 1, aber die UTF-8-Darstellung ist zwei
/// Bytes lang, und ein Emoji ist in Dart 2 Code-Units und in UTF-8 vier Bytes.
/// Ein Editor, der die Byte-Zahl des Daemons als Zeichenposition benutzte,
/// unterstriche bei jedem Umlaut vor einem Fund die falsche Stelle — und
/// ersetzte sie auch.
///
/// Deshalb wird **einmal beim Laden** umgerechnet ([FindingView.start] und
/// [FindingView.end] sind Code-Unit-Offsets im Text ihres Ortes) und beim
/// Senden gar nicht mehr zurückgerechnet: Was hinausgeht, ist der fertige
/// Text, und die Bytes zählt der Daemon selbst.
library;

import 'dart:convert';

import 'package:freezed_annotation/freezed_annotation.dart';

import '../../../core/body/body_kind.dart';
import '../../../core/domain/domain.dart';

export '../../../core/body/body_kind.dart' show BodyKind;

part 'draft.freezed.dart';

/// Die Kopfzeilen, die der Mensch nicht ändern darf.
///
/// Dieselben Namen wie im Daemon: `DAEMON_OWNED_HEADERS` in
/// `daemon/crates/proxy/src/edit.rs`, die der Daemon nach einer Bearbeitung
/// selbst setzt, und `HOP_BY_HOP` in `daemon/crates/proxy/src/upstream.rs`,
/// die er vor dem Weiterleiten streicht. Eine freie Zeile mit einem dieser
/// Namen käme nie beim Ziel an, ohne dass es jemand sähe. Ein Test in
/// `test/features/editor/draft_ops_test.dart` liest beide Listen aus dem
/// Rust-Quelltext und hält diese hier in beide Richtungen dagegen. Das Präfix
/// [lockedHeaderPrefix] sperrt darüber hinaus alle `proxy-*`, auch die, die der
/// Daemon durchließe: Solche Kopfzeilen sind für einen Proxy bestimmt, nicht
/// für das Ziel. Der Vergleich läuft **immer** über
/// Kleinbuchstaben: Kopfzeilennamen sind ohne Rücksicht auf Groß- und
/// Kleinschreibung zu lesen (RFC 9110 5.1), und ein `Content-Length` mit
/// großem C wäre sonst editierbar, ein `content-length` nicht.
///
/// Die Sperre im Feld ist Bequemlichkeit, keine Zusicherung: Die Zusicherung
/// gibt allein der Daemon (`apply_edit`). Ein Client, der nicht diese
/// Oberfläche ist, kommt an diesem Feld ohnehin vorbei.
const Set<String> lockedHeaderNames = <String>{
  'host',
  'content-length',
  'transfer-encoding',
  'content-encoding',
  'expect',
  'connection',
  'upgrade',
  'keep-alive',
  'te',
  'trailer',
};

/// Das Präfix, das eine ganze Familie gesperrter Kopfzeilen abdeckt.
const String lockedHeaderPrefix = 'proxy-';

/// Wahr, wenn eine Kopfzeile dem Daemon oder der Verbindung gehört.
bool isLockedHeader(String name) {
  final String lower = name.toLowerCase();
  return lockedHeaderNames.contains(lower) ||
      lower.startsWith(lockedHeaderPrefix);
}

/// Wo im Entwurf ein Fund oder eine Ersetzung sitzt.
///
/// Der Spiegel von `FindingLocation` des Kerns, der den Kopfzeilennamen im
/// Enum trägt. Dart-Enums tragen keine Werte, also stehen hier drei Felder;
/// [headerName] ist nur bei [FindingLocation.header] gefüllt und immer
/// kleingeschrieben.
///
/// # Warum der Name allein nicht reicht
///
/// Eine Kopfzeile darf mehrfach vorkommen, und mehrere tun es regelmäßig:
/// `Set-Cookie`, `Via`, `Warning`, jede eigene `X-`-Kopfzeile. Ein Ort, der nur
/// den Namen nennt, meint dann alle auf einmal — und eine Ersetzung schriebe
/// denselben Wert in jede davon und zerstörte die übrigen. [headerIndex] zeigt
/// deshalb auf **genau einen** Eintrag in `Draft.headers`; `-1` heißt „der
/// erste mit diesem Namen" und ist der Rückfall für einen Ort, den niemand
/// aufgelöst hat.
@freezed
abstract class DraftLocation with _$DraftLocation {
  /// Baut einen Ort.
  const factory DraftLocation({
    required FindingLocation kind,
    @Default('') String headerName,
    @Default(-1) int headerIndex,
  }) = _DraftLocation;

  const DraftLocation._();

  /// Der Body.
  static const DraftLocation body = DraftLocation(kind: FindingLocation.body);

  /// Die Query, also alles hinter dem ersten `?` von `pathAndQuery`.
  static const DraftLocation query = DraftLocation(kind: FindingLocation.query);

  /// Der Wert der Kopfzeile [name]; [index] zeigt auf genau einen Eintrag.
  static DraftLocation header(String name, {int index = -1}) => DraftLocation(
    kind: FindingLocation.header,
    headerName: name.toLowerCase(),
    headerIndex: index,
  );

  /// Der Ort eines Fundes des Daemons.
  ///
  /// Der Daemon nennt nur den Namen: Er durchsucht jeden Wert einzeln, aber
  /// der `FindingLocation::Header(name)` auf der Leitung trägt keine Nummer.
  /// Aufgelöst wird die Stelle beim Laden des Entwurfs (`buildDraft`), nicht
  /// hier.
  static DraftLocation of(Finding finding) => switch (finding.location) {
    FindingLocation.body => body,
    FindingLocation.query => query,
    FindingLocation.header => header(finding.headerName),
  };

  /// Die Stelle in [headers], auf die dieser Ort zeigt, oder `-1`.
  int indexIn(List<HeaderEntry> headers) {
    if (kind != FindingLocation.header) {
      return -1;
    }
    if (headerIndex >= 0 &&
        headerIndex < headers.length &&
        headers[headerIndex].name.toLowerCase() == headerName) {
      return headerIndex;
    }
    return headers.indexWhere(
      (HeaderEntry entry) => entry.name.toLowerCase() == headerName,
    );
  }
}

/// Was mit einem Fund geschehen ist.
enum FindingStatus {
  /// Niemand hat ihn angefasst; er zählt als offen.
  open,

  /// Er wurde durch ein Pseudonym ersetzt.
  replaced,

  /// Der Mensch hat ihn stehen lassen; er zählt nicht mehr als offen.
  ///
  /// Nur eine ausdrückliche Entscheidung setzt diesen Stand (`ignoreFinding`
  /// in `draft_ops.dart`), nie eine Eingabe: Ein Fund, den das Tippen
  /// mehrdeutig gemacht hat, bleibt offen, und der Knopf „Senden" hält weiter
  /// an der Pause (HUM-161).
  ignored,

  /// Der Wert steht an seinem Ort nicht mehr; jemand hat ihn gelöscht oder
  /// überschrieben.
  ///
  /// Gesetzt nur, wenn der Wert in dem, was hinausginge (Rumpf, jede
  /// Kopfzeile, Pfad, Query), **nirgends** mehr vorkommt. Solange er noch
  /// irgendwo steht, ist der Fund offen, und steht er wieder da, wird er es
  /// wieder (HUM-161, `anchoring.dart`).
  removed,
}

/// Eine Kopfzeile im Entwurf.
@freezed
abstract class HeaderEntry with _$HeaderEntry {
  /// Baut einen Eintrag.
  const factory HeaderEntry({
    required String name,
    required String value,
    @Default(false) bool locked,
  }) = _HeaderEntry;

  const HeaderEntry._();

  /// Der Eintrag zu einer Kopfzeile der Anfrage, mit gesetzter Sperre.
  ///
  /// Der Wert wird als UTF-8 gelesen und nicht über `Header.text`, das die
  /// Bytes einzeln als Zeichen nimmt (also Latin-1). Der Editor schreibt beim
  /// Senden UTF-8 zurück (`buildEditedRequest`), und die Fundstellen des
  /// Daemons sind Byte-Versätze in UTF-8: Ein `ü` im Wert, als zwei Zeichen
  /// gelesen, verschöbe jede Markierung dahinter — und eine unveränderte
  /// Kopfzeile ginge mit anderen Bytes hinaus, als sie hereinkam.
  factory HeaderEntry.of(Header header) => HeaderEntry(
    name: header.name,
    value: utf8.decode(header.value, allowMalformed: true),
    locked: isLockedHeader(header.name),
  );
}

/// Ein Fund im Entwurf: was der Daemon fand, und was daraus geworden ist.
@freezed
abstract class FindingView with _$FindingView {
  /// Baut die Sicht auf einen Fund.
  ///
  /// [index] ist die Stelle im `Analyzed`-Ereignis und bleibt stabil, auch
  /// wenn die Liste umsortiert wird: `acknowledged_findings` (HUM-049) zählt
  /// darüber. [start] und [end] sind Code-Unit-Offsets im Text von
  /// [location], nicht die Byte-Offsets des Fundes.
  ///
  /// [originalStart] und [originalEnd] sind dieselben Offsets im **Original**
  /// und wandern nie. Die linke Hälfte des Editors steht auf ihnen: Sie zeigt,
  /// was der Agent geschickt hat, und dort verschiebt sich nichts, gleich wie
  /// oft rechts ersetzt wird.
  ///
  /// [value] ist der Text des Fundes, wie er beim Laden an seiner Stelle
  /// stand. Er ist der Anker (HUM-161): Nach jeder Eingabe wird der Fund an
  /// ihm neu verankert, und ersetzt wird nur, wo er noch genau so steht. Ein
  /// Span auf veralteten Offsets ersetzte sonst fremden Text, meldete den Fund
  /// als ersetzt, und der Wert ginge ohne Pause hinaus. Er bleibt, wie
  /// [Replacement.original], im Entwurf und geht nie auf die Leitung.
  ///
  /// [unplaced] ist wahr, wenn sich der Bereich des Daemons nicht auf den
  /// Text seines Ortes umrechnen ließ, etwa weil der Rumpf nicht vorliegt
  /// oder der Bereich hinter dem Wert der Kopfzeile endet. Ein solcher Fund
  /// ist trotzdem da und geht mit hinaus: Er bleibt offen, zählt für den Knopf
  /// und die Pause und wird nie ersetzt, weil niemand weiß, wo er steht
  /// (HUM-161).
  ///
  /// [alternatives] sind die möglichen Werte eines Fundes, von dem offen ist,
  /// welche von mehreren gleichnamigen Kopfzeilen ihn trägt. Er gilt erst als
  /// entfernt, wenn keiner davon mehr hinausginge.
  const factory FindingView({
    required int index,
    required Finding finding,
    required DraftLocation location,
    required int start,
    required int end,
    @Default(FindingStatus.open) FindingStatus status,
    @Default('') String pseudonym,
    @Default('') String valueHash,
    @Default(0) int originalStart,
    @Default(0) int originalEnd,
    @Default('') String value,
    @Default(false) bool unplaced,
    @Default(<String>[]) List<String> alternatives,
  }) = _FindingView;

  const FindingView._();

  /// Wahr, solange niemand ihn ersetzt oder ignoriert hat.
  bool get isOpen => status == FindingStatus.open;

  /// Der `kind` des Fundes, wie der Daemon ihn schreibt.
  String get kind => finding.kind;
}

/// Eine angewandte Ersetzung, für den Diff-Glow und für das Mapping.
///
/// [start] und [end] zeigen auf das **Pseudonym** im aktuellen Text von
/// [location], nicht auf den Originalwert: Der ist weg, und was leuchten soll,
/// ist, was jetzt dasteht (BACKLOG.md 5, Signature-Element „Diff-Glow").
///
/// [original] steht hier und geht nie auf die Leitung. Er ist der eine Grund,
/// warum das Popover „Original ↔ Pseudonym" host-seitig etwas zu zeigen hat;
/// `ReplacementProto` trägt ihn ausdrücklich nicht (`backlog/sprint-4.md`,
/// HUM-047).
@freezed
abstract class Replacement with _$Replacement {
  /// Baut eine Ersetzung.
  const factory Replacement({
    required DraftLocation location,
    required int start,
    required int end,
    required String original,
    required String pseudonym,
    @Default('') String valueHash,
  }) = _Replacement;

  const Replacement._();

  /// Der Originalwert, maskiert: die ersten und letzten zwei Zeichen bleiben.
  ///
  /// Die vorläufige Maske dieses Issues; HUM-048 legt die endgültige fest. Ein
  /// Wert mit höchstens vier Zeichen wird ganz zu Sternen — bei `a@b.de` blieben
  /// sonst zwei Drittel stehen.
  String get maskedOriginal {
    if (original.length <= 4) {
      return '*' * original.length;
    }
    final String head = original.substring(0, 2);
    final String tail = original.substring(original.length - 2);
    return '$head${'*' * (original.length - 4)}$tail';
  }
}

/// Der Entwurf einer bearbeiteten Anfrage.
///
/// [scheme] und [authority] sind gesperrt: Über sie hat der Mensch entschieden,
/// als er die Anfrage in der Warteschlange sah, und eine Bearbeitung, die sie
/// verschöbe, wäre ein Egress, den niemand freigegeben hat. Der Daemon lehnt
/// das mit `EDIT_001` ab; das gesperrte Feld erspart nur den Weg dorthin.
@freezed
abstract class Draft with _$Draft {
  /// Baut einen Entwurf.
  const factory Draft({
    required FlowId flowId,
    required String method,
    required Scheme scheme,
    required Authority authority,
    required String pathAndQuery,
    required BodyKind bodyKind,
    @Default('') String body,
    @Default(<HeaderEntry>[]) List<HeaderEntry> headers,
    @Default(<FindingView>[]) List<FindingView> findings,
    @Default(<Replacement>[]) List<Replacement> replacements,
    @Default(<String, String>{}) Map<String, String> pseudonyms,
    @Default(<String, int>{}) Map<String, int> counters,
    @Default(false) bool dirty,
  }) = _Draft;

  const Draft._();

  /// Der Pfad ohne Query.
  String get path {
    final int mark = pathAndQuery.indexOf('?');
    return mark < 0 ? pathAndQuery : pathAndQuery.substring(0, mark);
  }

  /// Die Query ohne das führende `?`, oder ein leerer String.
  String get query {
    final int mark = pathAndQuery.indexOf('?');
    return mark < 0 ? '' : pathAndQuery.substring(mark + 1);
  }

  /// Die vollständige URL, wie `EditedRequest.url` sie verlangt.
  String get url =>
      '${scheme.name}://${authority.display(scheme)}$pathAndQuery';

  /// Wahr, wenn der Body im Editor nicht bearbeitet werden kann.
  ///
  /// Binärdaten, ein Rumpf über der Vorschaugrenze und ein Rumpf, den der
  /// Daemon nicht auspacken konnte, haben keinen Text, den ein Mensch ändern
  /// könnte. Der Editor sagt das und schaltet das Senden ab, statt einen
  /// halben Rumpf anzubieten (`backlog/sprint-4.md`, HUM-047 Nicht-Ziel).
  bool get bodyIsEditable =>
      bodyKind != BodyKind.binary && bodyKind != BodyKind.tooLarge;

  /// Der Text an [location] im aktuellen Entwurf.
  ///
  /// Bei einer Kopfzeile genau der eine Eintrag, auf den [location] zeigt; ein
  /// leerer Text, wenn es ihn nicht mehr gibt.
  String textAt(DraftLocation location) => switch (location.kind) {
    FindingLocation.body => body,
    FindingLocation.query => query,
    FindingLocation.header => _headerValue(location),
  };

  String _headerValue(DraftLocation location) {
    final int at = location.indexIn(headers);
    return at < 0 ? '' : headers[at].value;
  }

  /// Derselbe Entwurf mit [text] an [location].
  ///
  /// **Genau eine Kopfzeile wird geschrieben.** Über den Namen zu gehen
  /// schriebe denselben Wert in jede gleichnamige und löschte damit die
  /// übrigen; `Set-Cookie` und `Via` kommen mehrfach vor, und eine Ersetzung in
  /// der einen darf die andere nicht anfassen.
  Draft withTextAt(DraftLocation location, String text) =>
      switch (location.kind) {
        FindingLocation.body => copyWith(body: text),
        FindingLocation.query => copyWith(
          pathAndQuery: text.isEmpty ? path : '$path?$text',
        ),
        FindingLocation.header => _withHeaderValue(location, text),
      };

  Draft _withHeaderValue(DraftLocation location, String text) {
    final int at = location.indexIn(headers);
    if (at < 0) {
      return this;
    }
    final List<HeaderEntry> next = List<HeaderEntry>.of(headers);
    next[at] = next[at].copyWith(value: text);
    return copyWith(headers: next);
  }
}
