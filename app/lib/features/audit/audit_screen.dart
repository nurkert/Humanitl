/// Der Audit-Bildschirm: Zustand der Kette, die Records darunter, die
/// Aufbewahrung am Fuß (HUM-051).
///
/// Fünfter Eintrag der Icon-Rail, `Ctrl+5` (BACKLOG.md 5). Er ist der Ort, an
/// dem jemand ohne Kommandozeile prüfen und exportieren kann, was der Daemon
/// getan hat; die Kette selbst gehört dem Daemon, und dieser Bildschirm ist ein
/// dünner Client darauf (ADR-018).
///
/// Der Bildschirm besitzt drei Dinge und nicht mehr: die Aufteilung zwischen
/// Karte, Tabelle und Aufbewahrung, was ein Klick auf eine Zeile tut, und die
/// Meldung des letzten Exports. Alles andere steht in den drei Widgets darunter
/// und in den Providern.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/ipc/daemon_client.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/audit_provider.dart';
import 'widgets/audit_table.dart';
import 'widgets/chain_status_card.dart';
import 'widgets/retention_section.dart';

/// Breite des Sheets, das einen Record zeigt.
const double auditSheetWidth = 560;

/// Welchen Anteil der Höhe Statuskarte und Exportmeldung höchstens nehmen.
///
/// Darüber scrollen sie; die Tabelle behält den Rest. Gemessen am kleinsten
/// Fenster mit dem schlimmsten Fall — gebrochene Kette, zwei Warnungen,
/// gescheiterter Export — in `audit_screen_test.dart`.
const double auditTopShareMax = 0.35;

/// Der Audit-Bildschirm.
class AuditScreen extends ConsumerStatefulWidget {
  /// Legt den Bildschirm an.
  const AuditScreen({super.key});

  @override
  ConsumerState<AuditScreen> createState() => _AuditScreenState();
}

class _AuditScreenState extends ConsumerState<AuditScreen> {
  AuditRecordRow? _sheetRow;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AuditRecordRow? row = _sheetRow;
    return ColoredBox(
      color: tokens.colors.bg0,
      child: Stack(
        children: <Widget>[
          LayoutBuilder(
            builder: (BuildContext context, BoxConstraints constraints) =>
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    // Karte und Exportmeldung wachsen mit dem, was sie zu
                    // sagen haben: eine gebrochene Kette, zwei Warnungen, ein
                    // gescheiterter Export. Ohne Deckel schöben sie im
                    // schlimmsten Fall die Tabelle auf null Höhe, und das ist
                    // genau der Fall, in dem jemand die Records sehen will.
                    // Der Deckel ist ein Anteil der Höhe dieses Bildschirms,
                    // der Rest darüber scrollt (HUM-051, Befund M4).
                    ConstrainedBox(
                      constraints: BoxConstraints(
                        maxHeight: constraints.maxHeight * auditTopShareMax,
                      ),
                      child: const SingleChildScrollView(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            ChainStatusCard(),
                            _ExportNotice(),
                          ],
                        ),
                      ),
                    ),
                    const AuditFilterBar(),
                    const HHairline(),
                    Expanded(
                      child: AuditTable(
                        onOpen: (AuditRecordRow tapped) =>
                            setState(() => _sheetRow = tapped),
                      ),
                    ),
                    const HHairline(),
                    const RetentionSection(),
                  ],
                ),
          ),
          if (row != null)
            Positioned(
              top: 0,
              right: 0,
              bottom: 0,
              child: AuditRecordSheet(
                row: row,
                onClose: () => setState(() => _sheetRow = null),
              ),
            ),
        ],
      ),
    );
  }
}

/// Die Zeile unter der Karte, die sagt, was der letzte Export getan hat.
///
/// Inline und nicht als Streifen, der wieder verschwindet: Ein Export ist ein
/// Beleg, und wohin er geschrieben wurde, gehört auf den Schirm, bis jemand es
/// gelesen hat (`docs/UX.md` 4.6).
class _ExportNotice extends ConsumerWidget {
  const _ExportNotice();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AuditExportState job = ref.watch(auditExportProvider);
    final Widget? body = switch (job.phase) {
      AuditExportPhase.idle => null,
      AuditExportPhase.running => Text(
        key: const Key('audit-export-running'),
        l10n.auditExportRunning,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      ),
      AuditExportPhase.cancelled => Text(
        key: const Key('audit-export-cancelled'),
        l10n.auditExportCancelled,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      ),
      AuditExportPhase.done => Text(
        key: const Key('audit-export-done'),
        l10n.auditExportDone(job.records, job.path),
        style: tokens.typography.ui12.tinted(tokens.state.allowed),
      ),
      AuditExportPhase.failed => HDiagnosticCard(
        key: const Key('audit-export-failure'),
        code: job.failure?.code ?? '',
        severityLabel: l10n.auditSeverityError,
        color: tokens.state.error,
        title: l10n.auditExportFailedTitle,
        why: job.failure?.why ?? '',
        docsUrl: job.failure?.docsUrl,
        width: double.infinity,
      ),
    };
    if (body == null) {
      return const SizedBox.shrink();
    }
    return Padding(
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(child: body),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('audit-export-dismiss'),
            variant: HButtonVariant.ghost,
            onPressed: job.running
                ? null
                : () => ref.read(auditExportProvider.notifier).dismiss(),
            child: Text(l10n.auditExportDismiss),
          ),
        ],
      ),
    );
  }
}

/// Das Sheet mit dem vollständigen Record als JSON.
///
/// Nur lesen, nichts bearbeiten: Ein Record der Kette lässt sich nicht ändern,
/// und ein Feld, das sich bearbeiten ließe, versprächse das Gegenteil (HUM-051,
/// Nicht-Ziel).
class AuditRecordSheet extends StatefulWidget {
  /// Legt das Sheet für [row] an.
  const AuditRecordSheet({required this.row, required this.onClose, super.key});

  /// Der Record.
  final AuditRecordRow row;

  /// Schließt das Sheet.
  final VoidCallback onClose;

  @override
  State<AuditRecordSheet> createState() => _AuditRecordSheetState();
}

class _AuditRecordSheetState extends State<AuditRecordSheet> {
  bool _copied = false;

  @override
  void didUpdateWidget(AuditRecordSheet old) {
    super.didUpdateWidget(old);
    // „Kopiert" gilt dem Record, der kopiert wurde. Zeigt das Sheet einen
    // anderen, ist von dem noch nichts in der Zwischenablage.
    if (widget.row.seq != old.row.seq) {
      _copied = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String pretty = auditPrettyRecord(widget.row);
    return HSheet(
      title: Text(l10n.auditRecordTitle(widget.row.seq)),
      closeSemanticsLabel: l10n.auditRecordClose,
      onClose: widget.onClose,
      width: auditSheetWidth,
      actions: <Widget>[
        HButton(
          key: const Key('audit-record-copy'),
          variant: HButtonVariant.ghost,
          onPressed: () => unawaited(_copy(pretty)),
          child: Text(_copied ? l10n.auditRecordCopied : l10n.auditRecordCopy),
        ),
      ],
      // Kein Umbruch und keine Auswahl-Schicht: Der Record ist Beleg, er
      // scrollt in beide Richtungen, und kopiert wird er über den Knopf im
      // Kopf des Sheets (`docs/UX.md` 3.2).
      child: SingleChildScrollView(
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Text(
            key: const Key('audit-record-json'),
            pretty,
            style: tokens.typography.mono12.tinted(tokens.colors.fg0),
          ),
        ),
      ),
    );
  }

  Future<void> _copy(String text) async {
    await Clipboard.setData(ClipboardData(text: text));
    if (!mounted) {
      return;
    }
    setState(() => _copied = true);
  }
}

/// Der Record als eingerücktes JSON.
///
/// Die Zeile der Datei wird umgebrochen, nicht verändert: Dieselben Felder,
/// dieselben Werte, nur mit Einzug, damit ein Mensch sie lesen kann. Lässt sich
/// die Zeile nicht lesen, steht sie da, wie sie ist — ein Record, den der
/// Bildschirm nicht versteht, wird angezeigt und nicht verschwiegen.
String auditPrettyRecord(AuditRecordRow row) {
  if (row.line.isEmpty) {
    return row.dataJson;
  }
  try {
    return const JsonEncoder.withIndent('  ').convert(jsonDecode(row.line));
  } on FormatException {
    return row.line;
  }
}
