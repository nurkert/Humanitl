/// Das Protokoll, in dem die Anwendung ihr eigenes Ende aufschreibt
/// (HUM-136).
///
/// # Warum es diese Datei gibt
///
/// Am 2026-09-07 war das Fenster nach Stunden fort, der Daemon lief weiter,
/// der Agent lief weiter, und niemand konnte sagen, warum. In den Protokollen
/// stand die Zeile, die ein sauberes Schließen genauso erzeugt wie ein
/// Absturz. Ein Werkzeug, das einen Arbeitstag lang offen steht, muss über
/// sein eigenes Ende Auskunft geben; sonst hat der Mensch nichts zu melden
/// und nichts zu versuchen.
///
/// # Was darin steht, und was nicht
///
/// Genau vier Arten von Zeilen, und keine davon entsteht im Sekundentakt:
/// [kindStart] beim Start, [kindStop] beim geordneten Ende, [kindException]
/// für jede Ausnahme, die einer der drei Handler fängt
/// (`error_handlers.dart`), und [kindSignal] für ein Ende von außen. Die
/// Signal-Zeile schreibt nicht Dart, sondern der Linux-Runner
/// (`linux/runner/exit_log.cc`): Wenn `SIGTERM` kommt oder der native Teil
/// abstürzt, ist die Dart-Seite schon fort oder wird es im selben Augenblick.
/// Aus demselben Grund schreibt der Runner auch [kindStop], wenn die
/// GTK-Schleife endet -- er ist das Letzte, was in einem geordneten Ende noch
/// läuft.
///
/// Ein Protokoll, das bei jedem Bild schreibt, ist ein Protokoll, das die
/// Platte füllt. Deshalb gibt es zwei Grenzen: [maxBytes] für die Datei mit
/// genau einer Rotation nach `app.log.1`, und die Zusammenfassung gleicher
/// Ausnahmen zu einer Zeile mit `repeated=`. Mehr als `2 * maxBytes` darf das
/// Protokoll auf der Platte des Nutzers nie kosten.
///
/// # Warum nicht in `/tmp`
///
/// Ein Protokoll, das der nächste Neustart wegräumt, beantwortet die Frage
/// nie. Die Datei liegt unter `$XDG_STATE_HOME` -- dem Verzeichnis, das die
/// Spezifikation für genau diese Art Wert nennt: Zustand, der einen Neustart
/// überleben soll. Sie verlässt den Rechner des Nutzers nicht; es gibt keinen
/// Absturzbericht, der irgendwohin geht.
///
/// # Wer sie lesen darf
///
/// Der Text einer Ausnahme kann tragen, was sonst nirgends steht: ein Stück
/// Rumpf, eine Kopfzeile, ein Token. Deshalb zwei Maßnahmen, und die zweite
/// ersetzt die erste nicht. Erstens der Ort: `0600` in einem Verzeichnis
/// `0700` wie jede Datei mit Host-Zustand (`docs/SECURITY.md` 4 und 8,
/// `BACKLOG.md` 286) -- Schutz gegenüber anderen Konten auf derselben
/// Maschine, nicht gegenüber dem eigenen. Dart kennt keinen Systemaufruf
/// dafür, also stellt [_restrict] die Rechte über `chmod` nach; im fertigen
/// Programm ist das ein Nachtrag ohne Wirkung, weil der Runner Verzeichnis
/// und Datei schon mit den richtigen Rechten anlegt
/// (`linux/runner/exit_log.cc`). Zweitens die Redaktion: Was in die Zeile
/// darf, geht durch `redactForLog` (`redaction.dart`), und dort steht auch,
/// was sie nicht abfängt.
library;

import 'dart:convert' show utf8;
import 'dart:io';

import 'redaction.dart';

/// Wohin die Anwendung schreibt, was über ihren eigenen Lauf zu sagen ist.
///
/// Jede Methode ist eine Antwort und nie eine Ausnahme: Ein Protokoll, das
/// beim Schreiben wirft, verliert genau den Fehler, wegen dem es gerufen
/// wurde. Schlägt das Schreiben fehl, bleibt der Lauf davon unberührt.
class AppLog {
  /// Ein Protokoll unter [path].
  AppLog(this.path);

  /// Das Protokoll an der Stelle, die XDG dafür nennt.
  ///
  /// `$XDG_STATE_HOME/humanitl/app.log`, und `~/.local/state/...`, wenn die
  /// Variable nicht gesetzt ist -- dieselbe Reihenfolge, die der Daemon für
  /// seine Verzeichnisse benutzt (`humanitl_config::Paths`) und die der
  /// Runner in C noch einmal nachbildet, weil er vor Dart läuft.
  factory AppLog.resolve({Map<String, String>? environment}) {
    final Map<String, String> env = environment ?? Platform.environment;
    final String? state = env['XDG_STATE_HOME'];
    final String base = state == null || state.isEmpty
        ? '${env['HOME'] ?? '.'}/.local/state'
        : state;
    return AppLog('$base/humanitl/$fileName');
  }

  /// Der Name der Datei.
  static const String fileName = 'app.log';

  /// Der Name der einen Rotation.
  static const String rotatedFileName = 'app.log.1';

  /// Die Obergrenze der Datei in Bytes (256 KiB).
  ///
  /// Mit der einen Rotation kostet das Protokoll nie mehr als das Doppelte.
  static const int maxBytes = 256 * 1024;

  /// Die Obergrenze einer einzelnen Zeile in Bytes.
  ///
  /// Der Text einer Ausnahme kann lang sein, und eine einzige solche Zeile
  /// füllte sonst das halbe Protokoll. Wird gekürzt, endet die Zeile auf
  /// [cutMarker]. Die Grenze ist keine Schutzmaßnahme -- was nicht in die
  /// Zeile gehört, entfernt `redactForLog`, bevor gekürzt wird.
  static const int maxLineBytes = 4096;

  /// Die Rechte der Datei: nur der eigene Nutzer, lesen und schreiben.
  static const String fileMode = '600';

  /// Die Rechte des Verzeichnisses: nur der eigene Nutzer.
  static const String directoryMode = '700';

  /// Was am Ende einer gekürzten Zeile steht.
  static const String cutMarker = ' [cut]';

  /// Die Art der Zeile beim Start.
  static const String kindStart = 'start';

  /// Die Art der Zeile beim geordneten Ende.
  static const String kindStop = 'stop';

  /// Die Art der Zeile einer gefangenen Ausnahme.
  static const String kindException = 'exception';

  /// Die Art der Zeile eines Endes von außen oder eines nativen Absturzes.
  ///
  /// Sie entsteht in `linux/runner/exit_log.cc` und nie hier; die Konstante
  /// steht hier, weil das Format an einer Stelle beschrieben sein muss.
  static const String kindSignal = 'signal';

  /// Wo die Datei liegt.
  final String path;

  /// Wo die eine Rotation liegt.
  String get rotatedPath => path.endsWith(fileName)
      ? '${path.substring(0, path.length - fileName.length)}$rotatedFileName'
      : '$path.1';

  String? _repeatedSource;
  String? _repeatedText;
  int _repeats = 0;

  /// Schreibt die Startzeile mit Zeitpunkt, Version und Prozesskennung.
  void started({required String version}) {
    _never(() {
      _flushRepeats();
      _append(
        _line(kindStart, <String, String>{
          'version': version.isEmpty ? 'unknown' : version,
        }),
      );
    });
  }

  /// Schreibt die Zeile des geordneten Endes.
  ///
  /// Unter Linux ruft das niemand: Dort schreibt der Runner diese Zeile, wenn
  /// die GTK-Schleife endet, weil die Dart-Seite zu diesem Zeitpunkt nicht
  /// mehr verlässlich läuft. Die Methode steht hier für jeden anderen Weg,
  /// auf dem die Anwendung geordnet endet, und für die Tests des Formats.
  void stopped() {
    _never(() {
      _flushRepeats();
      _append(_line(kindStop, const <String, String>{}));
    });
  }

  /// Schreibt eine Ausnahme mit ihrer Herkunft.
  ///
  /// [source] nennt den Handler, der sie gefangen hat (`ErrorSource`), damit
  /// eine Zeile später beantwortet, ob der Fehler aus dem Baum, aus der
  /// Plattform oder aus der Zone um `runApp` kam. Der [stack] steht als eine
  /// Zeile dahinter; eine Ausnahme über mehrere Zeilen wäre für jede Auswertung
  /// eine zweite Ausnahme.
  ///
  /// Gleiche Ausnahmen hintereinander werden gezählt, nicht wiederholt: Ein
  /// Fehler, der bei jedem Bild erneut auftritt, schriebe sonst in Sekunden
  /// die Startzeile aus der Datei. Die Zählung erscheint als `repeated=`,
  /// sobald eine andere Zeile folgt.
  void exception(Object error, {StackTrace? stack, required String source}) {
    _never(() {
      // Der Vergleich läuft auf dem rohen Text: Ein Fehler bei jedem Bild
      // ginge sonst sechzigmal in der Sekunde durch vier reguläre Ausdrücke,
      // nur damit das Ergebnis verworfen wird. Redigiert wird auf dem
      // Schreibweg, in [_append].
      final String described = describeErrorForLog(error);
      final String text = _oneLine(
        stack == null ? described : '$described | $stack',
      );
      if (source == _repeatedSource && text == _repeatedText) {
        _repeats++;
        return;
      }
      _flushRepeats();
      _repeatedSource = source;
      _repeatedText = text;
      _append(_line(kindException, <String, String>{'source': source}, text));
    });
  }

  /// Führt [write] aus und lässt nichts daraus entkommen.
  ///
  /// Das ist kein Gürtel zum Hosenträger: `toString()` gehört dem Objekt, das
  /// geworfen wurde, und darf selbst werfen. Käme diese Ausnahme aus
  /// [exception] heraus, stünde sie in `zoneErrorHandler` unbehandelt da --
  /// und dann wäre das Protokoll genau das geworden, was es erklären soll:
  /// der Grund, warum das Fenster verschwindet.
  static void _never(void Function() write) {
    try {
      write();
    } on Object {
      // Kein zweiter Versuch und keine Meldung: Wer hier landet, hat schon
      // einen Fehler, und ein Protokoll, das darüber klagt, macht daraus zwei.
    }
  }

  /// Was in der Datei steht, oder eine leere Liste, wenn es sie nicht gibt.
  ///
  /// Nur für Tests und für die Frage „was stand da zuletzt"; die Anwendung
  /// liest ihr eigenes Protokoll nicht.
  List<String> readLines() {
    try {
      final File file = File(path);
      if (!file.existsSync()) {
        return const <String>[];
      }
      return file
          .readAsLinesSync()
          .where((String line) => line.isNotEmpty)
          .toList(growable: false);
    } on IOException {
      return const <String>[];
    }
  }

  /// Schreibt die Zusammenfassung der unterdrückten Wiederholungen.
  ///
  /// Sie entsteht erst, wenn eine andere Zeile folgt. Endet der Prozess
  /// vorher, fehlt die Zahl -- die erste Ausnahme steht dann trotzdem in der
  /// Datei, und das ist die Auskunft, um die es geht.
  void _flushRepeats() {
    final int repeats = _repeats;
    final String? source = _repeatedSource;
    final String? text = _repeatedText;
    _repeats = 0;
    _repeatedSource = null;
    _repeatedText = null;
    if (repeats == 0 || source == null || text == null) {
      return;
    }
    _append(
      _line(kindException, <String, String>{
        'source': source,
        'repeated': '$repeats',
      }, text),
    );
  }

  String _line(String kind, Map<String, String> fields, [String text = '']) {
    final StringBuffer out = StringBuffer(_timestamp())
      ..write(' ')
      ..write(kind)
      ..write(' pid=')
      ..write(pid);
    for (final MapEntry<String, String> field in fields.entries) {
      out
        ..write(' ')
        ..write(field.key)
        ..write('=')
        ..write(_oneLine(field.value));
    }
    if (text.isNotEmpty) {
      out
        ..write(' ')
        ..write(_oneLine(text));
    }
    return out.toString();
  }

  static String _timestamp() => DateTime.now().toUtc().toIso8601String();

  /// Macht aus beliebigem Text eine Zeile.
  ///
  /// Jedes Steuerzeichen wird ein Leerzeichen: Ein Zeilenumbruch im Text
  /// einer Ausnahme machte aus einem Eintrag zwei, und die zweite Zeile trüge
  /// keinen Zeitpunkt.
  static String _oneLine(String text) {
    final StringBuffer out = StringBuffer();
    bool lastWasSpace = false;
    for (final int unit in text.runes) {
      final bool isControl = unit < 0x20 || unit == 0x7f;
      if (isControl || unit == 0x20) {
        if (!lastWasSpace) {
          out.write(' ');
          lastWasSpace = true;
        }
        continue;
      }
      out.writeCharCode(unit);
      lastWasSpace = false;
    }
    return out.toString().trim();
  }

  static String _clamp(String line) {
    final List<int> bytes = utf8.encode(line);
    if (bytes.length <= maxLineBytes) {
      return line;
    }
    // Sechzehn Bytes Reserve: Der Schnitt kann mitten in eine UTF-8-Folge
    // fallen, und das Ersatzzeichen, das `allowMalformed` dafür einsetzt, ist
    // breiter als das, was es ersetzt.
    final int limit = maxLineBytes - cutMarker.length - 16;
    final String head = utf8.decode(
      bytes.sublist(0, limit),
      allowMalformed: true,
    );
    return '$head$cutMarker';
  }

  /// Der einzige Weg auf die Platte, und deshalb die Stelle, an der redigiert
  /// wird: Was hier nicht durchkommt, steht in keiner Datei.
  void _append(String line) {
    final String clamped = _clamp(redactForLog(line));
    final List<int> bytes = utf8.encode('$clamped\n');
    try {
      final File file = File(path);
      final int size = _prepare(file);
      if (size + bytes.length > maxBytes) {
        _rotate(file);
        // **Die neue Datei bekommt ihre Rechte hier und nicht beim nächsten
        // Anhängen.** `_prepare` lief vor der Rotation, und danach ist die
        // Datei fort; die nächste Zeile legte sie über `FileMode.append` neu
        // an, mit der Maske des Prozesses -- `rw-rw-r--` oder `rw-r--r--`.
        // Der Runner repariert das nicht: Sein `chmod` läuft nur beim Start.
        // Rotiert also die letzte Zeile vor einem Absturz, läge das Protokoll
        // bis zum nächsten Start offen da.
        file.createSync();
        _restrict(file.path, fileMode);
      }
      file.writeAsBytesSync(bytes, mode: FileMode.append, flush: true);
    } on IOException {
      // Der Lauf ist wichtiger als sein Protokoll. Eine volle Platte, ein
      // nicht beschreibbares Verzeichnis: Beides kostet die Antwort auf die
      // Frage nach dem Ende, aber keines davon das Fenster.
      // `FileSystemException` kommt hier mit an; sie erbt von `IOException`.
    }
  }

  /// Sorgt dafür, dass Verzeichnis und Datei da sind und niemandem sonst
  /// gehören. Liefert die Größe der Datei.
  ///
  /// Die Rechte werden bei jedem Anhängen geprüft und nicht nur beim Anlegen:
  /// Eine Datei aus einer älteren Fassung des Programms liegt sonst für immer
  /// offen da, und der Stat dafür fällt ohnehin an, weil die Größe gebraucht
  /// wird.
  int _prepare(File file) {
    final FileStat stat = file.statSync();
    if (stat.type != FileSystemEntityType.notFound) {
      if (stat.mode & _otherBits != 0) {
        _restrict(file.path, fileMode);
      }
      return stat.size;
    }
    final Directory parent = file.parent;
    if (!parent.existsSync()) {
      parent.createSync(recursive: true);
    }
    if (parent.statSync().mode & _otherBits != 0) {
      _restrict(parent.path, directoryMode);
    }
    file.createSync();
    _restrict(file.path, fileMode);
    return 0;
  }

  /// Für welche Ziele es aufgegeben wurde, die Rechte zu setzen.
  ///
  /// Auf einem Dateisystem ohne Rechte -- FAT, exFAT, manches über FUSE --
  /// bleibt `chmod` folgenlos, und ohne diese Marke forkte jede einzelne Zeile
  /// einen Prozess, synchron und mitten in `FlutterError.onError`. Ein
  /// Versuch je Ziel genügt: Was beim ersten Mal nicht half, hilft beim
  /// tausendsten auch nicht.
  ///
  /// **Je Ziel und nicht einmal für alle.** Verzeichnis und Datei sind zwei
  /// Fragen, und die erste wird zuerst gestellt: Gehört das Verzeichnis einem
  /// anderen -- dem Systemverwalter etwa -- oder hängt eine ACL daran,
  /// scheitert sein `chmod`, und eine gemeinsame Marke nähme der Datei damit
  /// den Versuch weg, den sie bestanden hätte. Das Protokoll bliebe für den
  /// ganzen Lauf offen lesbar, und niemand erführe davon.
  final Map<String, bool> _hopeless = <String, bool>{};

  /// Alles, was nicht dem Eigentümer gehört: Gruppe und Rest.
  static const int _otherBits = 0x3f;

  /// Nimmt Gruppe und Rest die Rechte.
  ///
  /// Über `chmod` und nicht über `dart:io`, weil `dart:io` dafür nichts hat.
  /// Ein Fehlschlag bleibt still: Er kostet den Schutz gegenüber anderen
  /// Konten, und im fertigen Programm ist er nicht erreichbar, weil der
  /// Runner beides vor dem ersten Bild anlegt.
  ///
  /// Danach wird nachgesehen, ob es etwas genützt hat, und **nur ein `chmod`,
  /// das gelungen ist und trotzdem nichts geändert hat**, gilt als
  /// aussichtslos ([_hopeless]): Das ist die Kennzeichnung eines
  /// Dateisystems ohne Rechte. Ein Fehlschlag ist etwas anderes -- keine
  /// Berechtigung, das Ziel gerade nicht da, `chmod` selbst nicht startbar --
  /// und der darf vorübergehend sein, sonst kostete ein einziger schlechter
  /// Augenblick den Schutz für den ganzen Lauf.
  void _restrict(String target, String mode) {
    if (_hopeless[target] ?? false) {
      return;
    }
    final ProcessResult result;
    try {
      result = Process.runSync('chmod', <String>[mode, target]);
    } on ProcessException {
      // Kein `chmod` auf diesem Rechner. Vorübergehend, nicht aussichtslos.
      return;
    } on IOException {
      // Nicht startbar. Ebenso.
      return;
    }
    if (result.exitCode != 0) {
      return;
    }
    // `statSync` wirft nicht: Was es nicht findet, hat den Modus 0, und der
    // trägt keine fremden Bits.
    if (FileStat.statSync(target).mode & _otherBits != 0) {
      _hopeless[target] = true;
    }
  }

  /// Schiebt die volle Datei auf [rotatedPath].
  ///
  /// Umbenennen und **nicht** vorher löschen: `rename` ersetzt das Ziel in
  /// einem Schritt. Zwischen einem Löschen und einem Umbenennen liegt ein
  /// Augenblick, in dem es keine Rotation gibt -- und der Runner rotiert
  /// dieselbe Datei, ohne von dieser Marke zu wissen. Löschte hier jemand
  /// zuerst, träfe er die Rotation, die der Runner gerade angelegt hat, und
  /// 256 KiB Vorgeschichte wären fort.
  void _rotate(File file) {
    try {
      if (file.existsSync()) {
        file.renameSync(rotatedPath);
      }
    } on IOException {
      // Konnte nicht rotiert werden: Dann wird angehängt, und die Obergrenze
      // gilt bis zum nächsten Versuch nicht. Das Protokoll zu verwerfen wäre
      // die teurere Antwort.
    }
  }
}
