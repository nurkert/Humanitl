/// Der Abschnitt „Aufbewahrung" unter der Tabelle (HUM-051).
///
/// Zwei Sätze nach der Skizze der Spezifikation, und beide sagen dasselbe wie
/// der Code: Die **Aufzeichnung** (Anfragen, Antworten, Bodies) wird nach 180
/// Tagen gelöscht, sofern `recorder.retention_days` nichts anderes sagt, und
/// `0` heißt nie (`humanitl_recorder::Retention`, Konfiguration 0 bis 3650).
/// Die **Audit-Kette** wird nie gelöscht: Der Aufräumlauf fasst weder
/// `audit.jsonl` noch `audit_anchors` an, und `audit.retention_days` hat in
/// dieser Fassung keinen Leser (HUM-157 entscheidet ihn).
///
/// **Warum die 180 als Vorgabe dasteht und nicht als Wert.** Der Client kennt
/// heute nur `SetConfig`, nicht `GetConfig` (`app/lib/core/ipc/daemon_client.dart`,
/// HUM-069); welche Zahl auf dieser Maschine gilt, kann er nicht lesen. Der
/// Satz nennt deshalb die Vorgabe zusammen mit dem Schlüssel, der sie ändert,
/// und der Knopf darunter kopiert den Befehl, der die geltenden Werte ausgibt
/// (`backlog/CONVENTIONS.md` 4.13). Sobald es den Einstellungs-Bildschirm
/// gibt, tritt der gelesene Wert an diese Stelle.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// Der Schlüssel, der die Aufbewahrung der Aufzeichnung steuert.
const String recorderRetentionKey = 'recorder.retention_days';

/// Der Schlüssel, der die Aufbewahrung der Audit-Kette steuert.
const String auditRetentionKey = 'audit.retention_days';

/// Der Abschnitt.
class RetentionSection extends ConsumerWidget {
  /// Legt den Abschnitt an.
  const RetentionSection({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          border: Border.all(color: tokens.colors.line),
          borderRadius: BorderRadius.circular(HRadius.card),
        ),
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x3),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                l10n.auditRetentionTitle,
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
              ),
              SizedBox(height: tokens.spacing.x2),
              Text(
                key: const Key('audit-retention-recordings'),
                l10n.auditRetentionRecordings(recorderRetentionKey),
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
              SizedBox(height: tokens.spacing.x1),
              Text(
                key: const Key('audit-retention-chain'),
                l10n.auditRetentionChain(auditRetentionKey),
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
              SizedBox(height: tokens.spacing.x3),
              // Der Weg zu den beiden Werten, solange der
              // Einstellungs-Bildschirm fehlt: ein Befehl, der sie ausgibt.
              // `FixControl` ist derselbe Knopf, den jeder andere Vorschlag
              // bekommt, damit „kopieren" überall dasselbe heißt.
              FixControl(
                fix: const FixAction.copyCommand(
                  command:
                      'humanitl config get $recorderRetentionKey && '
                      'humanitl config get $auditRetentionKey',
                ),
                copyKey: const Key('audit-retention-settings'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
