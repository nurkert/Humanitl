/// Die Karte über der Tabelle: hält die Kette, was sie trägt, und die zwei
/// Handlungen daneben (HUM-051).
///
/// Sie sagt drei Dinge, und jedes davon hat einen Absender:
///
/// 1. **Ob die Kette hält.** Das Ergebnis kommt aus `Audit(verify)` im Daemon;
///    dieser Bildschirm prüft nichts selbst. Solange die Prüfung läuft, steht
///    dort „wird geprüft" und nicht „in Ordnung" — ein grüner Punkt, den
///    niemand gemessen hat, ist die eine Lüge, gegen die dieser Bildschirm
///    gebaut ist (`backlog/CONVENTIONS.md` 4.13).
/// 2. **Was sie trägt.** Zahl der Records, Zahl der Anker, Alter des letzten
///    Ankers — alles aus `Audit(head)`.
/// 3. **Was sie nicht beweist.** Die Warnungen der Prüfung stehen als eigene
///    Zeile in Bernstein, nicht versteckt hinter einem Haken: Ohne Schlüssel
///    sind die MACs ungeprüft, und Records hinter dem letzten Anker könnten
///    fehlen, ohne dass es auffiele (`docs/SECURITY.md`, „Was die Audit-Kette
///    beweist").
///
/// Der Haken aus der Spezifikationsskizze ist ein Punkt geworden: `HGlyph`
/// kennt keinen Haken, und eine neue Form gehört nach `packages/ui` und nicht
/// in ein Feature. Der gebrochene Fall behält `shield-x`, den es dort gibt.
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/middle_ellipsis.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/audit_provider.dart';

/// Durchmesser des Zustandspunkts.
const double auditStatusDotSize = 8;

/// Wie viele Zeichen des Head-Hashes in der Karte stehen.
///
/// In der Mitte gekürzt, nicht am Ende: Anfang und Ende sind der Teil, den ein
/// Mensch gegen `humanitl audit verify --json | jq .head` hält. Der ganze Hash
/// steht im Screenreader-Namen und liegt hinter dem Kopierknopf; gekürzt wird
/// nur, was auf dem Schirm steht.
const int auditHeadHashChars = 17;

/// Die Statuskarte.
class ChainStatusCard extends ConsumerWidget {
  /// Legt die Karte an.
  const ChainStatusCard({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final int run = ref.watch(auditRunProvider);
    final AsyncValue<AuditHead> head = ref.watch(auditHeadProvider(run));
    final AsyncValue<AuditReport> report = ref.watch(auditVerifyProvider(run));
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
              _StatusLine(report: report, head: head),
              SizedBox(height: tokens.spacing.x2),
              _HeadLine(head: head),
              if (report.value case final AuditReport value) ...<Widget>[
                for (final AuditWarning warning in value.warnings)
                  Padding(
                    padding: EdgeInsets.only(top: tokens.spacing.x2),
                    child: _WarningLine(warning: warning),
                  ),
                if (value.diagnostic case final Diagnostic diagnostic)
                  Padding(
                    padding: EdgeInsets.only(top: tokens.spacing.x3),
                    child: HDiagnosticCard(
                      key: const Key('audit-broken-diagnostic'),
                      code: diagnostic.code,
                      severityLabel: l10n.auditSeverityError,
                      color: tokens.state.blocked,
                      title: l10n.auditBrokenTitle,
                      why: diagnostic.why,
                      docsUrl: diagnostic.docsUrl,
                      width: double.infinity,
                      fix: FixControl(
                        fix: diagnostic.fix,
                        copyKey: const Key('audit-broken-fix'),
                      ),
                    ),
                  ),
              ],
              if (report case AsyncError(:final Object error))
                Padding(
                  padding: EdgeInsets.only(top: tokens.spacing.x3),
                  child: _FailureCard(
                    keyValue: const Key('audit-verify-failure'),
                    title: l10n.auditVerifyFailedTitle,
                    error: error,
                  ),
                ),
              if (head case AsyncError(:final Object error))
                Padding(
                  padding: EdgeInsets.only(top: tokens.spacing.x3),
                  child: _FailureCard(
                    keyValue: const Key('audit-head-failure'),
                    title: l10n.auditHeadFailedTitle,
                    error: error,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Die erste Zeile: Punkt, Zustand, Zahlen.
class _StatusLine extends StatelessWidget {
  const _StatusLine({required this.report, required this.head});

  final AsyncValue<AuditReport> report;
  final AsyncValue<AuditHead> head;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AuditReport? value = report.value;
    final bool broken = value != null && !value.ok;
    final Color color = switch (value) {
      null => tokens.colors.fg2,
      AuditReport(ok: true) => tokens.state.allowed,
      _ => tokens.state.blocked,
    };
    final String text = switch (value) {
      null when report.hasError => l10n.auditStatusUnknown,
      null => l10n.auditStatusChecking,
      AuditReport(ok: true) => l10n.auditStatusOk,
      final AuditReport failed => l10n.auditStatusBroken(
        failed.firstBadSeq,
        auditBreakReasonLabel(l10n, failed.reason),
      ),
    };
    final AuditHead? counts = head.value;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        if (broken)
          HGlyphIcon(
            HGlyph.shieldX,
            size: HType.ui13.fontSize!,
            color: color,
            semanticsLabel: text,
          )
        else
          _StatusDot(color: color, semanticsLabel: text),
        SizedBox(width: tokens.spacing.x2),
        Flexible(
          child: Text(
            key: const Key('audit-status-text'),
            text,
            style: tokens.typography.ui13.medium.tinted(color),
            overflow: TextOverflow.ellipsis,
          ),
        ),
        if (counts != null) ...<Widget>[
          const _Separator(),
          Text(
            key: const Key('audit-record-count'),
            l10n.auditRecords(counts.records),
            style: tokens.typography.ui13.tinted(tokens.colors.fg1),
          ),
          const _Separator(),
          // Nicht gemeldet ist nicht null: Ein Daemon, der die Anker nicht
          // nennt, hat keine gezählt, und „keine Anker" wäre eine Aussage über
          // die Kette, die niemand gemessen hat (HUM-051).
          if (counts.anchors case final int anchors) ...<Widget>[
            Text(
              key: const Key('audit-anchor-count'),
              l10n.auditAnchors(anchors),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
            const _Separator(),
            Text(
              counts.lastAnchorAt == null
                  ? l10n.auditNoAnchorYet
                  : l10n.auditLastAnchor(
                      auditAgeLabel(l10n, counts.lastAnchorAt!),
                    ),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          ] else
            Text(
              key: const Key('audit-anchor-count'),
              l10n.auditAnchorsUnknown,
              style: tokens.typography.ui13.tinted(tokens.colors.fg2),
            ),
        ],
      ],
    );
  }
}

/// Die zweite Zeile: der Head-Hash, der Kopierknopf und die zwei Handlungen.
class _HeadLine extends ConsumerStatefulWidget {
  const _HeadLine({required this.head});

  final AsyncValue<AuditHead> head;

  @override
  ConsumerState<_HeadLine> createState() => _HeadLineState();
}

class _HeadLineState extends ConsumerState<_HeadLine> {
  bool _copied = false;

  @override
  void didUpdateWidget(_HeadLine old) {
    super.didUpdateWidget(old);
    // „Kopiert" gilt dem Hash, der kopiert wurde. Ein neuer Kopf ist noch
    // nicht in der Zwischenablage (Review 3).
    if (widget.head.value?.hash != old.head.value?.hash) {
      _copied = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String hash = widget.head.value?.hash ?? '';
    final int run = ref.watch(auditRunProvider);
    final bool checking = ref.watch(auditVerifyProvider(run)).isLoading;
    return Row(
      children: <Widget>[
        Text(
          l10n.auditHeadLabel,
          style: tokens.typography.ui12.tinted(tokens.colors.fg2),
        ),
        SizedBox(width: tokens.spacing.x2),
        // Der Hash ist ein Beleg, also Monospace.
        Semantics(
          label: hash.isEmpty ? null : hash,
          child: Text(
            key: const Key('audit-head-hash'),
            hash.isEmpty ? '\u2014' : middleEllipsis(hash, auditHeadHashChars),
            style: tokens.typography.mono12.tinted(tokens.colors.fg0),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        HButton(
          key: const Key('audit-copy-head'),
          variant: HButtonVariant.ghost,
          onPressed: hash.isEmpty ? null : () => unawaited(_copy(hash)),
          child: Text(_copied ? l10n.auditHeadCopied : l10n.auditCopyHead),
        ),
        const Spacer(),
        HButton(
          key: const Key('audit-verify-now'),
          variant: HButtonVariant.secondary,
          onPressed: checking
              ? null
              : () => ref.read(auditRunProvider.notifier).again(),
          child: Text(checking ? l10n.auditVerifyRunning : l10n.auditVerifyNow),
        ),
        SizedBox(width: tokens.spacing.x2),
        const AuditExportMenu(),
      ],
    );
  }

  Future<void> _copy(String hash) async {
    await Clipboard.setData(ClipboardData(text: hash));
    if (!mounted) {
      return;
    }
    setState(() => _copied = true);
  }
}

/// Das Export-Menü: die zwei Dokumente, dann der Dialog des Schreibtischs.
class AuditExportMenu extends ConsumerStatefulWidget {
  /// Legt das Menü an.
  const AuditExportMenu({super.key});

  @override
  ConsumerState<AuditExportMenu> createState() => _AuditExportMenuState();
}

class _AuditExportMenuState extends ConsumerState<AuditExportMenu> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AuditExportState job = ref.watch(auditExportProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.end,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        HButton(
          key: const Key('audit-export-open'),
          variant: HButtonVariant.ghost,
          onPressed: job.running ? null : () => setState(() => _open = !_open),
          child: Text(l10n.auditExport),
        ),
        if (_open)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                HButton(
                  key: const Key('audit-export-jsonl'),
                  variant: HButtonVariant.secondary,
                  onPressed: () => _run(AuditExportFormat.jsonl),
                  child: Text(l10n.auditExportJsonl),
                ),
                SizedBox(height: tokens.spacing.x1),
                HButton(
                  key: const Key('audit-export-csv'),
                  variant: HButtonVariant.secondary,
                  onPressed: () => _run(AuditExportFormat.csv),
                  child: Text(l10n.auditExportCsv),
                ),
              ],
            ),
          ),
      ],
    );
  }

  void _run(AuditExportFormat format) {
    setState(() => _open = false);
    final String dialogTitle = context.l10n.auditExportDialogTitle;
    unawaited(_commitThenExport(format, dialogTitle));
  }

  /// Übernimmt, was noch unbestätigt in einem Zeitfeld steht, und exportiert
  /// erst dann.
  ///
  /// Wer ein Datum tippt und ohne Enter auf „Export" klickt, meint den
  /// Zeitraum, den er sieht. Der Fokus verlässt deshalb zuerst das Feld; das
  /// Feld übernimmt dabei seinen Text wie nach Enter (`_RangeField`). Die
  /// Fokusänderung wendet Flutter in einer Mikroaufgabe an, also wird eine
  /// Runde gewartet. Steht danach in einem Feld etwas, das keine Zeit ist,
  /// beginnt kein Export: Die Zeile unter dem Feld sagt schon, warum
  /// (Review 2, Befund 2).
  Future<void> _commitThenExport(
    AuditExportFormat format,
    String dialogTitle,
  ) async {
    FocusManager.instance.primaryFocus?.unfocus();
    await Future<void>.delayed(Duration.zero);
    if (!mounted) {
      return;
    }
    if (ref.read(auditRangeInvalidProvider).isNotEmpty) {
      return;
    }
    await ref
        .read(auditExportProvider.notifier)
        .run(
          format: format,
          filter: ref.read(auditFilterProvider),
          dialogTitle: dialogTitle,
        );
  }
}

/// Eine Warnung der Prüfung, in Bernstein.
class _WarningLine extends StatelessWidget {
  const _WarningLine({required this.warning});

  final AuditWarning warning;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String text = switch (warning.kind) {
      'no_hmac_key' => l10n.auditWarningNoHmacKey,
      'unanchored_tail' => l10n.auditWarningUnanchoredTail(warning.records),
      // `records` ist hier die Nummer des letzten gelöschten Records (HUM-157).
      'pruned' => l10n.auditWarningPruned(warning.records),
      _ => l10n.auditWarningUnknown(warning.kind),
    };
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        HGlyphIcon(
          HGlyph.triangleAlert,
          size: HType.ui12.fontSize!,
          color: tokens.state.held,
          semanticsLabel: text,
        ),
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          child: Text(
            text,
            style: tokens.typography.ui12.tinted(tokens.state.held),
          ),
        ),
      ],
    );
  }
}

/// Ein Befund, der eine der beiden Abfragen getroffen hat.
class _FailureCard extends StatelessWidget {
  const _FailureCard({
    required this.keyValue,
    required this.title,
    required this.error,
  });

  final Key keyValue;
  final String title;
  final Object error;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Diagnostic diagnostic = auditDiagnosticOf(error);
    return HDiagnosticCard(
      key: keyValue,
      code: diagnostic.code,
      severityLabel: context.l10n.auditSeverityError,
      color: tokens.state.error,
      title: title,
      why: diagnostic.why,
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
      fix: FixControl(fix: diagnostic.fix, copyKey: keyValue),
    );
  }
}

/// Der Punkt vor dem Zustand.
class _StatusDot extends StatelessWidget {
  const _StatusDot({required this.color, required this.semanticsLabel});

  final Color color;
  final String semanticsLabel;

  @override
  Widget build(BuildContext context) => Semantics(
    label: semanticsLabel,
    child: SizedBox.square(
      dimension: auditStatusDotSize,
      child: DecoratedBox(
        decoration: BoxDecoration(color: color, shape: BoxShape.circle),
      ),
    ),
  );
}

/// Der Mittelpunkt zwischen zwei Zahlen der Kopfzeile.
class _Separator extends StatelessWidget {
  const _Separator();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      child: Text('·', style: tokens.typography.ui13.tinted(tokens.colors.fg2)),
    );
  }
}

/// Der Grund eines Bruchs, in der Sprache des Menschen.
String auditBreakReasonLabel(AppLocalizations l10n, AuditBreakReason reason) =>
    switch (reason) {
      AuditBreakReason.seqGap => l10n.auditReasonSeqGap,
      AuditBreakReason.prevMismatch => l10n.auditReasonPrevMismatch,
      AuditBreakReason.hashMismatch => l10n.auditReasonHashMismatch,
      AuditBreakReason.macMismatch => l10n.auditReasonMacMismatch,
      AuditBreakReason.nonCanonicalLine => l10n.auditReasonNonCanonicalLine,
      AuditBreakReason.anchorMismatch => l10n.auditReasonAnchorMismatch,
      AuditBreakReason.truncatedBelowAnchor =>
        l10n.auditReasonTruncatedBelowAnchor,
      AuditBreakReason.unknown => l10n.auditReasonUnknown,
    };

/// Wie lange [at] her ist, grob: gerade eben, Minuten, Stunden, Tage.
///
/// Grob mit Absicht. Der Anker ist ein Beleg, sein **Alter** ist eine
/// Einordnung; eine Sekundenzahl daneben wäre eine Genauigkeit, die niemand
/// braucht und die jede Sekunde neu gezeichnet werden müsste.
String auditAgeLabel(AppLocalizations l10n, DateTime at, {DateTime? now}) {
  final Duration age = (now ?? DateTime.now()).toUtc().difference(at.toUtc());
  if (age.inMinutes < 1) {
    return l10n.auditAgeJustNow;
  }
  if (age.inHours < 1) {
    return l10n.auditAgeMinutes(age.inMinutes);
  }
  if (age.inDays < 1) {
    return l10n.auditAgeHours(age.inHours);
  }
  return l10n.auditAgeDays(age.inDays);
}

/// Der Befund hinter einem Fehler einer Audit-Abfrage.
///
/// Derselbe Weg wie im Verbindungstor: Was eine [DaemonException] trägt, ist
/// der Satz des Daemons; alles andere wird zu `DAEMON_001` mit dem Fehler im
/// `why`. Eine zweite Übersetzung wäre eine zweite Antwort auf dieselbe Frage.
Diagnostic auditDiagnosticOf(Object error) =>
    DaemonConnection.diagnosticOf(error);
