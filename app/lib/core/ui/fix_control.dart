/// The control for a [FixAction]: a button that carries the action out where
/// this client can (installing the user service, copying a command, a link or
/// an `export` line), a chip naming the action for what HUM-069 wires up
/// later.
///
/// Lives in `core/ui` because several screens show diagnostics -- the setup
/// screen, the sandbox, the tray notice, the action bar and the diagnostic
/// strip of the intercept screen -- and no feature imports another one
/// (ARCHITECTURE 5). A `Diagnostic` that carries a `FixAction` and shows no
/// action is a defect (`docs/UX.md` 4.4), so the control has to be reachable
/// from wherever a diagnostic is drawn.
///
/// The line stays on the honest side of that rule in both directions. It never
/// offers a button for something the client cannot do -- writing an environment
/// variable into the configuration needs `SetConfig`, which answers
/// `unimplemented` until HUM-069, so copying the `export` line is the part that
/// works today. And where it can act, it acts: `InstallService` runs
/// `humanitl daemon install` instead of putting it on the clipboard, because
/// the acceptance criterion of HUM-044 is measured -- the row turns green
/// within four seconds of the click -- and a clipboard does not turn anything
/// green.
library;

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../domain/domain.dart';
import '../../l10n/l10n.dart';
import 'shell_command.dart';
import 'ui.dart';

/// Der Befehl, den [FixAction.installService] ausführt, als eine Zeile.
///
/// Er steht hier und nicht in vier Sätzen verstreut: `humanitl daemon install`
/// ist der eine Weg, die Nutzer-Unit anzulegen, und was er tut, steht in
/// `docs/cli.md`. Angezeigt wird diese Zeile; ausgeführt wird sie nie als
/// Zeichenkette, sondern als [installServiceBinary] mit
/// [installServiceArguments].
const String installServiceCommand = 'humanitl daemon install';

/// Name der Kommandozeile neben der laufenden Anwendung.
const String installServiceBinary = 'humanitl';

/// Die Argumente von [installServiceCommand], einzeln.
///
/// Eine Liste und keine Zeile: `Process.run` bekommt sie ohne Shell, so dass
/// nichts an ihnen erst noch ausgewertet wird.
const List<String> installServiceArguments = <String>['daemon', 'install'];

/// Was die Anwendung startet, wenn sie [FixAction.installService] ausführt.
///
/// Null, wenn neben der laufenden Anwendung keine Kommandozeile liegt, die
/// diesen Namen trägt und nicht die Anwendung selbst ist.
///
/// **Nie aus `PATH`, nie aus einem Konfigurationswert, nie über eine Shell**
/// (HUM-044, Schritt 7): Was hier läuft, schreibt eine systemd-Unit, und der
/// einzige Pfad, dem diese Anwendung dafür traut, ist ihr eigener Nachbar.
///
/// **Zwei Orte, in dieser Reihenfolge.** Das Paket legt beides in denselben
/// Baum, aber nicht auf dieselbe Ebene (HUM-053, `backlog/sprint-4.md`): Das
/// Flutter-Binary liegt als `/usr/lib/humanitl/humanitl`, die Kommandozeile
/// eine Ebene tiefer als `/usr/lib/humanitl/bin/humanitl`, und `/usr/bin/humanitl`
/// ist nur ein Symlink darauf. Deshalb wird zuerst `bin/` gesucht und erst
/// danach das Verzeichnis der Anwendung selbst, wo ein Bündel ohne diese
/// Trennung sie ablegen würde.
///
/// Beide Male gilt dieselbe Bedingung: Der Fund darf nicht die laufende
/// Anwendung sein. Sie trägt heute denselben Namen wie die Kommandozeile
/// (`app/linux/CMakeLists.txt`, `BINARY_NAME`), und ein Aufruf von sich selbst
/// öffnete ein zweites Fenster, statt etwas zu installieren.
///
/// [runningExecutable] und [exists] sind für Tests; ohne sie gelten der
/// laufende Prozess und das Dateisystem.
({String? path, String? refusal}) installServiceCandidate({
  String? runningExecutable,
  bool Function(String path)? exists,
  int Function(String path)? modeOf,
}) {
  final String running = runningExecutable ?? Platform.resolvedExecutable;
  final int slash = running.lastIndexOf('/');
  if (slash <= 0) {
    return (
      path: null,
      refusal: 'this application has no directory to look in',
    );
  }
  final String directory = running.substring(0, slash);
  final bool Function(String) present =
      exists ?? ((String path) => File(path).existsSync());
  final int Function(String) mode =
      modeOf ?? ((String path) => File(path).statSync().mode);
  String? refusal;
  for (final String candidate in <String>[
    '$directory/bin/$installServiceBinary',
    '$directory/$installServiceBinary',
  ]) {
    if (candidate == running || !present(candidate)) {
      continue;
    }
    switch (_ownedAllTheWayUp(candidate, mode)) {
      case _Owned.yes:
        return (path: candidate, refusal: null);
      case _Owned.writableByOthers:
        refusal ??=
            '$candidate, or a directory above it, may be written by '
            'somebody other than you';
      case _Owned.unmeasurable:
        refusal ??= 'the permissions of $candidate could not be read';
    }
  }
  return (
    path: null,
    refusal:
        refusal ??
        'no $installServiceBinary lies in $directory/bin or beside this '
            'application',
  );
}

/// Wie [`_ownedAllTheWayUp`] ausging.
enum _Owned {
  /// Niemand außer dem Eigentümer kann etwas daran ändern.
  yes,

  /// Irgendwo auf dem Weg darf die Gruppe oder dürfen alle schreiben.
  writableByOthers,

  /// Ein Pfad ließ sich nicht messen.
  unmeasurable,
}

/// Ob [path] und **jedes** Verzeichnis darüber gegen fremde Hand geschützt ist.
///
/// Der Nachbar wird gleich ausgeführt, und was er tut, ist eine systemd-Unit in
/// das Verzeichnis des Nutzers zu schreiben. Dass er nicht aus `PATH` kommt, war
/// die halbe Zusage; die andere Hälfte ist, dass ihn niemand sonst hingelegt
/// haben kann.
///
/// **Die ganze Kette, nicht nur das Elternverzeichnis.** Wer `/opt/humanitl`
/// schreiben darf, ersetzt `bin` samt Inhalt und wählt die Rechte darin selbst;
/// eine Prüfung, die bei `/opt/humanitl/bin` aufhört, sieht davon nichts. Genau
/// dieser Fall stand als Begründung im Kommentar und war trotzdem offen.
///
/// **Das Sticky-Bit zählt als Schutz.** `/tmp` trägt `1777`: Jeder darf dort
/// anlegen, aber nur der Eigentümer eines Eintrags darf ihn löschen oder
/// umbenennen. Ein AppImage hängt unter `/tmp/.mount_*`, und ohne diese
/// Ausnahme verlöre es den Knopf, obwohl niemand seine Datei ersetzen kann.
///
/// **Der Eigentümer wird nicht geprüft**, und das ist eine bekannte Lücke:
/// `FileStat` aus `dart:io` gibt `mode` her, aber keine uid. Eine Datei, die
/// einem anderen Konto gehört und `0755` trägt, kommt hier durch. Sie
/// abzudecken hieße `stat(2)` über FFI.
_Owned _ownedAllTheWayUp(String path, int Function(String) mode) {
  const int groupWrite = 0x10; // 0o020
  const int otherWrite = 0x2; // 0o002
  const int sticky = 0x200; // 0o1000
  String at = path;
  while (true) {
    final int bits;
    try {
      bits = mode(at);
    } on Object {
      return _Owned.unmeasurable;
    }
    final bool othersMayWrite = bits & (groupWrite | otherWrite) != 0;
    if (othersMayWrite && bits & sticky == 0) {
      return _Owned.writableByOthers;
    }
    if (at == '/') {
      return _Owned.yes;
    }
    at = _directoryOf(at);
  }
}

/// Das Verzeichnis, in dem [path] liegt.
String _directoryOf(String path) {
  final int slash = path.lastIndexOf('/');
  return slash <= 0 ? '/' : path.substring(0, slash);
}

/// Führt `humanitl daemon install` aus und liefert null, wenn es geklappt hat.
///
/// Ein Fehlschlag ist ein `Diagnostic` wie jeder andere und kein stilles
/// Nichts (`docs/UX.md` 4.4): Der Befund trägt den Grund und als Abhilfe
/// denselben Befehl zum Kopieren, damit ihn jemand im Terminal fahren kann.
///
/// Der Satz einer Absage nennt, was wirklich gemessen wurde, und nicht
/// pauschal „liegt nicht daneben": Eine Datei, die daliegt und nur wegen ihrer
/// Rechte verworfen wurde, ist etwas anderes als eine, die fehlt, und wer sie
/// vor sich sieht, während der Bildschirm ihr Fehlen behauptet, sucht an der
/// falschen Stelle (CONVENTIONS 4.13).
///
/// [resolve] und [run] sind für Tests; ohne sie gelten
/// [installServiceCandidate] und `Process.run` ohne Shell.
Future<Diagnostic?> runInstallService({
  ({String? path, String? refusal}) Function()? resolve,
  Future<ProcessResult> Function(String executable, List<String> arguments)?
  run,
}) async {
  final ({String? path, String? refusal}) found =
      (resolve ?? installServiceCandidate)();
  final String? executable = found.path;
  if (executable == null) {
    return _installFailed(
      found.refusal ?? 'no $installServiceBinary command line was found',
    );
  }
  try {
    final ProcessResult result = await (run ?? _runWithoutShell)(
      executable,
      installServiceArguments,
    );
    if (result.exitCode == 0) {
      return null;
    }
    final String detail = <String>[
      '$executable exited with ${result.exitCode}',
      if (_text(result.stderr) case final String message
          when message.isNotEmpty)
        message,
    ].join(': ');
    return _installFailed(detail);
  } on Object catch (error) {
    return _installFailed('$executable could not be started: $error');
  }
}

/// Der Prozessaufruf ohne Shell und ohne `PATH`-Suche.
Future<ProcessResult> _runWithoutShell(
  String executable,
  List<String> arguments,
) => Process.run(executable, arguments);

/// Die Ausgabe eines Prozesses als Zeile, ohne Rand.
String _text(Object? output) => output is String ? output.trim() : '';

/// Der Befund eines fehlgeschlagenen Versuchs.
///
/// Derselbe Code wie die Zeile, die ihn zeigt: Der Dienst läuft weiterhin
/// nicht, und das ist `DAEMON_001`. Ein eigener Code stünde nicht im Register
/// (`daemon/crates/core-types/src/diagnostics/codes.rs`), und diese Anwendung
/// erfindet keinen.
Diagnostic _installFailed(String why) => Diagnostic(
  code: DiagnosticCodes.daemonUnreachable,
  severity: Severity.error,
  why: why,
  fix: const FixAction.copyCommand(command: installServiceCommand),
);

/// The control for a [FixAction].
class FixControl extends StatefulWidget {
  /// Creates the control for [fix]; renders nothing for null.
  const FixControl({
    required this.fix,
    this.copyKey,
    this.installService,
    super.key,
  });

  /// The proposed fix.
  final FixAction? fix;

  /// Was der Knopf von [FixAction.installService] tut.
  ///
  /// Null bedeutet [runInstallService]; Tests setzen ihre eigene Fassung ein,
  /// damit kein Widget-Test einen Prozess startet.
  final Future<Diagnostic?> Function()? installService;

  /// Key of the copy button, for a screen that draws more than one of these.
  ///
  /// Two cards in the same strip would otherwise both answer to
  /// `setup-fix-copy`, and a test that taps that key would not know which one
  /// it hit. Null keeps the shared key the single-card screens use.
  final Key? copyKey;

  @override
  State<FixControl> createState() => _FixControlState();
}

class _FixControlState extends State<FixControl> {
  bool _copied = false;
  bool _installing = false;
  Diagnostic? _installFailure;

  /// Legt die Unit an und startet sie.
  ///
  /// Ein zweiter Klick, während der erste läuft, tut nichts: Der Knopf ist
  /// solange aus, und `humanitl daemon install` zweimal nebeneinander schriebe
  /// dieselbe Datei zweimal.
  Future<void> _install() async {
    if (_installing) {
      return;
    }
    setState(() {
      _installing = true;
      _installFailure = null;
    });
    final Diagnostic? failure =
        await (widget.installService ?? runInstallService)();
    if (!mounted) {
      return;
    }
    setState(() {
      _installing = false;
      _installFailure = failure;
    });
  }

  Future<void> _copy(String text) async {
    await Clipboard.setData(ClipboardData(text: text));
    if (!mounted) {
      return;
    }
    setState(() => _copied = true);
    // Kein Literal: jede Dauer kommt aus `HMotion` (`docs/UX.md` 2.1).
    await Future<void>.delayed(HMotion.copyFeedback);
    if (mounted) {
      setState(() => _copied = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TextStyle mono = tokens.typography.mono12.tinted(tokens.colors.fg1);
    return switch (widget.fix) {
      null => const SizedBox.shrink(),
      FixActionCopyCommand(:final command) => _copyRow(
        tokens,
        label: _copied ? l10n.setupFixCopied : l10n.setupFixCopyCommand,
        text: command,
        style: mono,
      ),
      FixActionOpenUrl(:final url) => _copyRow(
        tokens,
        label: _copied ? l10n.setupFixCopied : l10n.setupFixCopyLink,
        text: url,
        // Der Akzent ist eine Fläche und misst hell 3,42:1 bis 3,74:1; ein
        // Wort darauf braucht 4,5:1 (`docs/UX.md` 6).
        style: tokens.typography.mono12.tinted(tokens.colors.accentText),
      ),
      // Das Abzeichen benennt, was zu tun ist; die Zeile darunter tut den
      // einen Teil davon, den dieser Client heute wirklich ausführen kann.
      // Ein Knopf, der in die Konfiguration schriebe, gehört hier nicht hin:
      // `SetConfig` antwortet bis HUM-069 `unimplemented`, und ein Control,
      // das etwas verspricht, was nicht geschieht, ist schlimmer als keines
      // (`docs/UX.md` 4.4, `backlog/CONVENTIONS.md` 4.13).
      //
      // Der Befehl wird nie interpoliert, sondern gebaut: `shell_command.dart`
      // quotiert den Wert und verweigert die Zeile, wo sie nicht beweisbar
      // genau eine Zuweisung wäre. Dann steht dort der Grund und kein Knopf.
      FixActionSetEnv(:final key, :final value) => _setEnv(
        tokens,
        l10n,
        key: key,
        value: value,
        style: mono,
      ),
      FixActionChangeSetting(:final key) => HBadge(
        text: l10n.setupFixChangeSetting(key),
      ),
      FixActionInstallService() => _installService(tokens, l10n, style: mono),
      FixActionAddRule() => HBadge(text: l10n.setupFixAddRule),
      FixActionRemountReadOnly() => HBadge(text: l10n.setupFixRemountReadOnly),
    };
  }

  /// Der Knopf und daneben, was er kopiert.
  ///
  /// [reflow] wählt zwischen zwei Anordnungen. Ohne ihn steht beides in einer
  /// Zeile und der Text wird gekürzt, wenn er nicht passt; das ist die Form,
  /// die der Setup-Bildschirm, die Sandbox und die Mitteilung des Trays auf
  /// ihren breiten Karten haben. Mit ihm rutscht der Text unter den Knopf,
  /// statt zu überlaufen, und wird nie gekürzt.
  ///
  /// Der Unterschied ist die Breite, die das Control bekommt: In der
  /// Warteschlange sind es ab `HSize.paneMinQueue` 280 px, und dort läuft eine
  /// Zeile aus Knopf und Befehl über den Rand (`docs/UX.md` 6). Was jemand
  /// kopieren soll, soll er auch lesen können, also bricht die schmale Form
  /// um, statt eine Ellipse zu setzen.
  Widget _copyRow(
    HTokens tokens, {
    required String label,
    required String text,
    required TextStyle style,
    bool reflow = false,
  }) {
    final Widget button = HButton(
      key: widget.copyKey ?? const Key('setup-fix-copy'),
      onPressed: () => _copy(text),
      child: Text(label),
    );
    if (reflow) {
      return Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: tokens.spacing.x3,
        runSpacing: tokens.spacing.x1,
        children: <Widget>[
          button,
          Text(text, style: style),
        ],
      );
    }
    return Row(
      children: <Widget>[
        button,
        SizedBox(width: tokens.spacing.x3),
        Expanded(
          child: Text(text, style: style, overflow: TextOverflow.ellipsis),
        ),
      ],
    );
  }

  /// Das Abzeichen für `InstallService`, der Knopf, der die Unit anlegt und
  /// startet, und daneben der Befehl, den er ausführt.
  ///
  /// **Der Knopf startet wirklich etwas.** Er fährt `humanitl daemon install`
  /// -- die Kommandozeile neben der laufenden Anwendung, aufgelöst von
  /// [installServiceCandidate], mit ihren Argumenten als Liste, ohne Shell,
  /// ohne `PATH` und ohne `sudo`. Danach färbt der Zwei-Sekunden-Takt der
  /// Verbindung die Zeile grün (HUM-044, Akzeptanzkriterium: binnen 4 s).
  ///
  /// Der Befehl steht sichtbar daneben, bevor er läuft: Was eine Unit auf
  /// diese Maschine schreibt, sagt vorher, was es tut (`docs/cli.md`,
  /// `humanitl daemon install`).
  ///
  /// Klappt es nicht, steht der Grund darunter, und die Abhilfe dieses Befunds
  /// ist derselbe Befehl zum Kopieren. Ein stiller Knopf, der nichts tut und
  /// nichts sagt, wäre schlimmer als keiner (`docs/UX.md` 4.4,
  /// `backlog/CONVENTIONS.md` 4.13).
  Widget _installService(
    HTokens tokens,
    AppLocalizations l10n, {
    required TextStyle style,
  }) {
    final Diagnostic? failure = _installFailure;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        HBadge(text: l10n.setupFixInstallService),
        SizedBox(height: tokens.spacing.x2),
        Wrap(
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: tokens.spacing.x3,
          runSpacing: tokens.spacing.x1,
          children: <Widget>[
            HButton(
              key: const Key('setup-fix-install'),
              onPressed: _installing ? null : _install,
              child: Text(
                _installing
                    ? l10n.setupFixInstallServiceRunning
                    : l10n.setupFixInstallServiceRun,
              ),
            ),
            Text(installServiceCommand, style: style),
          ],
        ),
        if (failure != null) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Text(
            l10n.setupFixInstallServiceFailed(failure.why),
            key: const Key('setup-fix-install-failed'),
            style: tokens.typography.ui12.tinted(
              tokens.stateTextColor(HFlowState.error),
            ),
          ),
          SizedBox(height: tokens.spacing.x2),
          _copyRow(
            tokens,
            label: _copied ? l10n.setupFixCopied : l10n.setupFixCopyCommand,
            text: installServiceCommand,
            style: style,
            reflow: true,
          ),
        ],
      ],
    );
  }

  /// Das Abzeichen für `SetEnv` und darunter entweder die Kopierzeile oder
  /// der Grund, warum es keine gibt.
  ///
  /// Angezeigt und kopiert wird dieselbe Zeichenkette. Wer die eine säuberte
  /// und die andere roh ließe, hätte den Fehler nur verschoben.
  Widget _setEnv(
    HTokens tokens,
    AppLocalizations l10n, {
    required String key,
    required String value,
    required TextStyle style,
  }) {
    final String? command = exportCommand(key: key, value: value);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        HBadge(text: l10n.setupFixSetEnv(key)),
        SizedBox(height: tokens.spacing.x2),
        if (command == null)
          Text(
            switch (exportRefusal(key: key, value: value)) {
              ExportRefusal.value => l10n.setupFixSetEnvMultiline(key),
              // `null` steht hier nur, weil der Typ es zulässt: Ohne Grund
              // gäbe es einen Befehl, und dieser Zweig liefe nicht.
              ExportRefusal.key || null => l10n.setupFixSetEnvBadKey(key),
            },
            key: const Key('setup-fix-no-command'),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          )
        else
          _copyRow(
            tokens,
            label: _copied ? l10n.setupFixCopied : l10n.setupFixCopyExport,
            text: command,
            style: style,
            // Diese Zeile steht in der Warteschlange, also im schmalsten
            // Pane des Programms.
            reflow: true,
          ),
      ],
    );
  }
}
