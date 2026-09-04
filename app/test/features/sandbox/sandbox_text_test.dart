// Was der Bildschirm sagt, ohne einen Baum zu bauen: die Laufzeit als Text,
// die Farbe eines Zustands, und dass beide Sprachdateien dieselben Schlüssel
// tragen.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/sandbox/sandbox_text.dart';

Map<String, Object?> _arb(String name) =>
    jsonDecode(File('l10n/$name').readAsStringSync()) as Map<String, Object?>;

void main() {
  test('the uptime is exact and never rounded', () {
    expect(sandboxUptimeText(Duration.zero), '0:00');
    expect(sandboxUptimeText(const Duration(seconds: 9)), '0:09');
    expect(sandboxUptimeText(const Duration(minutes: 4, seconds: 7)), '4:07');
    expect(sandboxUptimeText(const Duration(minutes: 75)), '1:15:00');
    expect(
      sandboxUptimeText(const Duration(hours: 12, minutes: 3, seconds: 4)),
      '12:03:04',
    );
    // Eine Uhr, die zurücksteht, zeigt null und keine negative Zeit.
    expect(sandboxUptimeText(const Duration(seconds: -5)), '0:00');
  });

  test('only a writable bind counts as writable', () {
    expect(MountMode.rw.isWritable, isTrue);
    for (final MountMode mode in MountMode.values) {
      if (mode != MountMode.rw) {
        expect(mode.isWritable, isFalse, reason: mode.name);
      }
    }
  });

  test('a withheld value is not an empty one', () {
    // Der Name klingt nach nichts, und genau darum geht es: die Vorgabe ist
    // „zurückgehalten", nicht „sichtbar, außer der Name klingt verdächtig".
    const EnvEntry hidden = EnvEntry(key: 'DATABASE_URL', withheld: true);
    const EnvEntry empty = EnvEntry(key: 'NO_PROXY');
    expect(hidden.isMasked, isTrue);
    expect(hidden.isEmpty, isFalse);
    expect(empty.isMasked, isFalse);
    expect(empty.isEmpty, isTrue);
  });

  test('a stopped sandbox with a finished agent is not "exited"', () {
    const SandboxStatus stopped = SandboxStatus();
    expect(stopped.agentExited, isFalse, reason: 'nothing was ever running');
    const SandboxStatus running = SandboxStatus(
      state: SandboxState.running,
      agentRunning: false,
    );
    expect(running.agentExited, isTrue);
  });

  test('only a blocking finding forbids a start', () {
    const SandboxStatus warned = SandboxStatus(
      diagnostics: <Diagnostic>[
        Diagnostic(code: 'SANDBOX_005', severity: Severity.warning),
      ],
    );
    expect(warned.blocking, isNull);
    const SandboxStatus blocked = SandboxStatus(
      diagnostics: <Diagnostic>[
        Diagnostic(code: 'SANDBOX_001', severity: Severity.blocking),
      ],
    );
    expect(blocked.blocking?.code, 'SANDBOX_001');
  });

  test('only the paths of the person are the exception to the sentence', () {
    const SandboxStatus status = SandboxStatus(
      mounts: <MountEntry>[
        MountEntry(dst: '/usr', src: '/usr', mode: MountMode.ro),
        MountEntry(
          dst: '/work',
          src: '/home/u/p',
          mode: MountMode.rw,
          origin: ValueOrigin.session,
        ),
        MountEntry(
          dst: '/home/u/.cache/pip',
          src: '/home/u/.cache/pip',
          mode: MountMode.ro,
          origin: ValueOrigin.user,
        ),
      ],
    );
    expect(status.extraHostPaths.map((MountEntry m) => m.dst), <String>[
      '/home/u/.cache/pip',
    ]);
    expect(status.workMount?.src, '/home/u/p');
  });

  test('both languages carry the same sandbox keys', () {
    Set<String> keys(Map<String, Object?> arb) =>
        arb.keys.where((String key) => key.startsWith('sandbox')).toSet();
    final Set<String> en = keys(_arb('app_en.arb'));
    expect(en, isNotEmpty);
    expect(keys(_arb('app_de.arb')), en);
  });

  test('the German of this screen says Einhängungen, not Mounts', () {
    final Map<String, Object?> de = _arb('app_de.arb');
    expect(de['sandboxTabMounts'], 'Einhängungen');
    // Der Satz, um den es auf diesem Bildschirm geht, nennt /work und sagt
    // ausdrücklich, dass sonst nichts hineinreicht.
    final String sentence = de['sandboxMountsSentence']! as String;
    expect(sentence, contains('/work'));
    expect(sentence, contains('Sonst nichts'));
  });
}
