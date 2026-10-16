// Goldens der Statuskarte des Audit-Screens (HUM-051, HUM-054): die Kette
// hält, und die Kette ist bei Record 150 gebrochen, mit dem Befund des
// Daemons darunter. Je in den vier Varianten aus
// `support/golden_variants.dart`.
//
// Nur die Karte und nicht der ganze Bildschirm: Die Tabelle darunter zeigt
// Zeitpunkte in der Ortszeit des Rechners, und ein Golden, das auf einem
// Rechner in Berlin anders aussieht als im Runner, bewacht nichts. Aus
// demselben Grund steht der Fake ohne Anker da: Das Alter des letzten Ankers
// misst die Karte an der Wanduhr (`auditAgeLabel`).
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/audit/widgets/chain_status_card.dart';

import 'support/golden_variants.dart';

/// Ein Fake, dessen Kette keinen Anker trägt; [broken] bricht sie bei 150.
FakeDaemonClient auditClient({bool broken = false}) {
  final FakeDaemonClient client = FakeDaemonClient()..auditAnchorEvery = 0;
  if (broken) {
    client
      ..auditChainBroken = true
      ..auditBreakSeq = 150
      ..auditBreakReason = AuditBreakReason.hashMismatch;
  }
  return client;
}

/// Die Karte über [client], oben im Bild.
Widget statusCard(GoldenVariant variant, FakeDaemonClient client) =>
    goldenFrame(
      variant: variant,
      background: variant.tokens.colors.bg0,
      overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      child: const Align(
        alignment: Alignment.topCenter,
        child: ChainStatusCard(),
      ),
    );

/// Lässt Kopf und Prüfung ankommen und die Wartefrist von `HWait` ablaufen.
Future<void> settleCard(WidgetTester tester) async {
  for (int i = 0; i < 10; i++) {
    await tester.pump(const Duration(milliseconds: 16));
  }
  await tester.pump(const Duration(milliseconds: 600));
  await tester.pumpAndSettle();
}

void main() {
  goldenVariants(
    'audit_status_ok',
    constraints: const BoxConstraints.tightFor(width: 900, height: 160),
    pumpBeforeTest: settleCard,
    builder: (GoldenVariant variant) => statusCard(variant, auditClient()),
  );

  goldenVariants(
    'audit_status_broken',
    constraints: const BoxConstraints.tightFor(width: 900, height: 360),
    pumpBeforeTest: settleCard,
    builder: (GoldenVariant variant) =>
        statusCard(variant, auditClient(broken: true)),
  );
}
