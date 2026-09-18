// Der Audit-Screen gegen einen echten Daemon (HUM-156, offene Kriterien von
// HUM-051).
//
// Nicht Teil des normalen Laufs, aus demselben Grund wie
// `test/features/sandbox/daemon_live_test.dart`: Der Test startet einen echten
// `humanitld` und ruft die gebaute Kommandozeile `humanitl` auf. Er läuft nur
// mit `make flutter-test-daemon`, also mit `HUMANITL_DAEMON_TESTS=1`.
//
// **Warum es ihn gibt.** Drei Kriterien von HUM-051 hingen an einem Daemon, der
// `Audit` beantwortet: ein Ergebnis nach höchstens zwei Sekunden, „gebrochen
// ab Sequenz n" nach einer Manipulation der Datei, und derselbe Kopf-Hash wie
// `humanitl audit verify --json`. Die Widget-Tests daneben messen den
// Bildschirm gegen eine Attrappe; hier laufen die echten Provider, die der
// Bildschirm liest, gegen den echten Dienst, und die Kommandozeile fragt
// denselben Daemon.
//
// **Was er nicht misst.** Pixel und Tasten: dass `Ctrl+5` den Bildschirm
// öffnet und dass die Karte den Bruch in Worte fasst, messen
// `navigationKeys` und `status_broken_shows_seq_and_reason`.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/launch_options.dart';
import 'package:humanitl/features/audit/providers/audit_provider.dart';

import '../sandbox/daemon_live_test.dart' show LiveDaemon, liveAllowed;

/// Die Frist aus HUM-051: Die Status-Karte zeigt nach höchstens zwei
/// Sekunden ein Ergebnis.
const Duration resultBudget = Duration(seconds: 2);

/// Das Verzeichnis des Repositories, von `app/` aus gesehen.
String _repoRoot() => Directory.current.path.endsWith('/app')
    ? Directory.current.parent.path
    : Directory.current.path;

/// Was `humanitl --json audit verify` gegen [daemon] sagt, samt Exit-Code.
Future<(int, Map<String, Object?>)> _cliVerify(LiveDaemon daemon) async {
  final ProcessResult result = await Process.run(
    '${_repoRoot()}/daemon/target/debug/humanitl',
    <String>['--json', 'audit', 'verify'],
    environment: daemon.environment,
    stdoutEncoding: utf8,
  );
  final Object? decoded = jsonDecode((result.stdout as String).trim());
  return (result.exitCode, decoded! as Map<String, Object?>);
}

/// Das Log des Daemons unter seinem Datenverzeichnis.
File _log(LiveDaemon daemon) =>
    File('${daemon.root.path}/data/humanitl/audit/audit.jsonl');

/// Liest Kopf und Prüfung so, wie der Bildschirm sie beim Öffnen liest:
/// beide zugleich, in einer neuen Generation.
Future<(AuditHead, AuditReport, Duration)> _open(
  ProviderContainer container,
  int generation,
) async {
  final Stopwatch watch = Stopwatch()..start();
  final (AuditHead head, AuditReport report) = await (
    container.read(auditHeadProvider(generation).future),
    container.read(auditVerifyProvider(generation).future),
  ).wait;
  watch.stop();
  return (head, report, watch.elapsed);
}

void main() {
  if (!liveAllowed) {
    test('audit_daemon_live_tests_are_opt_in', () {
      // Kein `skip:` an einem echten Test: Ein übersprungener Test steht als
      // solcher im Bericht und erklärt sich selbst.
    }, skip: 'set HUMANITL_DAEMON_TESTS=1 (make flutter-test-daemon)');
    return;
  }

  LiveDaemon? daemon;
  ProviderContainer? container;
  // Die Provider halten ihren Wert nur, solange jemand zuhört; der Bildschirm
  // hört zu, hier tun es diese Abos.
  final List<ProviderSubscription<Object?>> listening =
      <ProviderSubscription<Object?>>[];

  setUpAll(() async {
    final File cli = File('${_repoRoot()}/daemon/target/debug/humanitl');
    if (!cli.existsSync()) {
      fail('daemon/target/debug/humanitl is missing; run cargo build first');
    }
    daemon = await LiveDaemon.start();
    if (daemon == null) {
      fail('daemon/target/debug/humanitld is missing; run cargo build first');
    }
    container = ProviderContainer(
      overrides: <Override>[
        launchOptionsProvider.overrideWithValue(
          LaunchOptions(socketPath: daemon!.socket),
        ),
      ],
    );
  });

  tearDownAll(() async {
    for (final ProviderSubscription<Object?> subscription in listening) {
      subscription.close();
    }
    try {
      container?.dispose();
    } finally {
      await daemon?.stop();
    }
  });

  test(
    'a_real_daemon_answers_within_two_seconds_with_the_head_of_the_cli',
    () async {
      // Ein Daemon schreibt beim Start einige Records; gewartet wird, bis der
      // Kopf zweimal hintereinander derselbe ist, damit der Vergleich mit der
      // Kommandozeile nicht an einem Record hängt, der dazwischen kam.
      AuditHead? previous;
      int generation = 0;
      late AuditHead head;
      late AuditReport report;
      late Duration took;
      for (;;) {
        generation++;
        listening
          ..add(container!.listen(auditHeadProvider(generation), (_, _) {}))
          ..add(container!.listen(auditVerifyProvider(generation), (_, _) {}));
        (head, report, took) = await _open(container!, generation);
        if (previous != null && previous.hash == head.hash) {
          break;
        }
        previous = head;
        expect(generation, lessThan(20), reason: 'the head never settled');
        await Future<void>.delayed(const Duration(milliseconds: 300));
      }

      expect(report.ok, isTrue, reason: '${report.diagnostic}');
      expect(head.hash, hasLength(64));
      expect(head.anchors, isNotNull, reason: 'the daemon reports its anchors');
      expect(
        report.warnings.where((AuditWarning w) => w.kind == 'no_hmac_key'),
        isEmpty,
        reason: 'the daemon checks with its key',
      );
      expect(took, greaterThan(Duration.zero));
      expect(
        took,
        lessThan(resultBudget),
        reason: 'the status card has a result inside the budget of HUM-051',
      );
      // ignore: avoid_print
      print(
        'audit live: head and verify answered in ${took.inMilliseconds} ms',
      );

      final (int exit, Map<String, Object?> cli) = await _cliVerify(daemon!);
      expect(exit, 0, reason: '$cli');
      expect(cli['mode'], 'full');
      expect(cli['checked_by'], 'daemon');
      final Map<String, Object?> cliHead = cli['head']! as Map<String, Object?>;
      expect(
        cliHead['hash'],
        head.hash,
        reason:
            'the head hash of the screen is the one the command line prints',
      );
      expect(cliHead['seq'], head.seq);
    },
  );

  test(
    'a_changed_line_is_broken_at_its_seq_in_the_screen_and_the_cli',
    () async {
      // Ein Zeichen im ersten Record, an Ort und Stelle: Die Länge der Datei
      // bleibt, und der Schreiber des Daemons hängt weiter hinten an.
      final File log = _log(daemon!);
      final String first = (await log.readAsLines()).first;
      const String marker = '"version":"';
      final int at = first.indexOf(marker);
      expect(at, greaterThanOrEqualTo(0), reason: first);
      final int index = at + marker.length;
      final int offset = utf8.encode(first.substring(0, index)).length;
      final int old = first.codeUnitAt(index);
      // `dd` mit `conv=notrunc` schreibt an der Stelle, ohne zu kürzen und ohne
      // `O_APPEND`; Darts `FileMode` hat keinen Modus, der beides zusagt.
      final Process dd = await Process.start('dd', <String>[
        'of=${log.path}',
        'bs=1',
        'seek=$offset',
        'conv=notrunc',
        'status=none',
      ]);
      dd.stdin.add(<int>[if (old == 0x39) 0x38 else old + 1]);
      await dd.stdin.close();
      expect(await dd.exitCode, 0);
      expect(
        (await log.readAsLines()).first,
        isNot(first),
        reason: 'the first record really changed',
      );

      const int generation = 1000;
      listening
        ..add(container!.listen(auditHeadProvider(generation), (_, _) {}))
        ..add(container!.listen(auditVerifyProvider(generation), (_, _) {}));
      final (_, AuditReport report, Duration took) = await _open(
        container!,
        generation,
      );
      expect(report.ok, isFalse);
      expect(report.firstBadSeq, 1);
      expect(report.reason, AuditBreakReason.hashMismatch);
      expect(report.diagnostic?.code, 'AUDIT_001');
      expect(took, lessThan(resultBudget));

      final (int exit, Map<String, Object?> cli) = await _cliVerify(daemon!);
      expect(exit, 4, reason: '$cli');
      expect(cli['first_bad_seq'], report.firstBadSeq);
      expect(cli['reason'], 'hash_mismatch');
      expect(cli['checked_by'], 'daemon');
    },
  );
}
