/// Die Filterleiste und die Tabelle der Records (HUM-051).
///
/// Virtualisiert über eine feste Zeilenhöhe, wie die History-Tabelle: Das
/// Blättern durch zehntausend Zeilen kostet so viel wie das durch zwanzig
/// (`docs/UX.md` 7). Geladen wird nie die ganze Kette, sondern Seite um Seite
/// über den Cursor des Daemons; sie wächst ohne Grenze, und eine Tabelle, die
/// alles hielte, hörte mit der Zeit auf zu funktionieren.
///
/// Nichts hier bricht um. Zu breit wird waagerecht gescrollt, denn ein Umbruch
/// verschiebt jede Zeile unter der, die jemand gerade vergleicht
/// (`docs/UX.md` 3.2).
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/focus_ring.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/audit_provider.dart';

/// Die Breiten der fünf Spalten.
abstract final class AuditColumns {
  /// `seq`, rechtsbündig.
  static const double seq = 72;

  /// Der Zeitpunkt in Monospace.
  static const double time = 190;

  /// Die Art.
  static const double kind = 170;

  /// Die Sitzung, gekürzt.
  static const double session = 96;

  /// Die Zusammenfassung nimmt den Rest, mindestens so viel.
  static const double summaryMin = 260;

  /// Die Summe der festen Spalten plus die Mindestbreite der letzten.
  static double get total => seq + time + kind + session + summaryMin;
}

/// Bei welchem Anteil der Scrollstrecke die nächste Seite geholt wird.
const double auditLoadMoreAt = 0.8;

/// Die Arten, nach denen die Filterleiste anbietet zu filtern.
///
/// Präfixe, keine vollständigen Namen: `flow.` trifft jede Art eines Flusses.
/// Die Liste ist kurz mit Absicht — sie ist eine Abkürzung für den häufigen
/// Fall, nicht das Verzeichnis aller Arten (`humanitl_audit::RecordKind`).
const List<String> auditKindPrefixes = <String>[
  'flow.',
  'flow.decided',
  'rule.',
  'config.changed',
  'session.',
  'audit.',
];

/// Die Filterleiste über der Tabelle.
class AuditFilterBar extends ConsumerWidget {
  /// Legt die Leiste an.
  const AuditFilterBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AuditFilter filter = ref.watch(auditFilterProvider);
    final AuditRecordsState page = ref.watch(auditRecordsProvider(filter));
    // Die Leiste trägt die Zeitfelder und hält darum ihre Fehlermarken am
    // Leben; fällt die Leiste, fallen die Marken mit (Review 3).
    ref.watch(auditRangeInvalidProvider);
    return Padding(
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x3,
        vertical: tokens.spacing.x2,
      ),
      child: Row(
        children: <Widget>[
          Text(
            l10n.auditFilterLabel,
            style: tokens.typography.ui12.tinted(tokens.colors.fg2),
          ),
          SizedBox(width: tokens.spacing.x2),
          _KindFilter(filter: filter),
          SizedBox(width: tokens.spacing.x2),
          _SessionFilter(filter: filter),
          SizedBox(width: tokens.spacing.x2),
          _RangeField(
            fieldKey: const Key('audit-filter-from'),
            label: l10n.auditFilterFrom,
            value: filter.from,
            onChanged: (DateTime? at) =>
                ref.read(auditFilterProvider.notifier).setFrom(at),
          ),
          SizedBox(width: tokens.spacing.x2),
          _RangeField(
            fieldKey: const Key('audit-filter-to'),
            label: l10n.auditFilterTo,
            value: filter.to,
            onChanged: (DateTime? at) =>
                ref.read(auditFilterProvider.notifier).setTo(at),
          ),
          const Spacer(),
          Text(
            key: const Key('audit-total'),
            l10n.auditTotal(page.total),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ],
      ),
    );
  }
}

/// Die Auswahl der Art.
class _KindFilter extends ConsumerWidget {
  const _KindFilter({required this.filter});

  final AuditFilter filter;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    return _Chooser<String>(
      chooserKey: const Key('audit-filter-kind'),
      value: filter.kindPrefix,
      options: <String>['', ...auditKindPrefixes],
      label: (String option) =>
          option.isEmpty ? l10n.auditFilterKindAll : option,
      onChanged: (String option) =>
          ref.read(auditFilterProvider.notifier).setKindPrefix(option),
    );
  }
}

/// Die Auswahl der Sitzung.
///
/// Die Sitzungen kommen aus den geladenen Records und aus nirgendwo sonst: Der
/// Vertrag kennt keine Liste der Sitzungen des Audit-Logs, und eine erfundene
/// wäre eine Behauptung über Zeilen, die niemand gelesen hat.
class _SessionFilter extends ConsumerWidget {
  const _SessionFilter({required this.filter});

  final AuditFilter filter;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final AuditRecordsState page = ref.watch(auditRecordsProvider(filter));
    final List<String> sessions = <String>{
      if (filter.session.isNotEmpty) filter.session,
      for (final AuditRecordRow row in page.rows)
        if (row.session != auditNoSession) row.session,
    }.toList()..sort();
    return _Chooser<String>(
      chooserKey: const Key('audit-filter-session'),
      value: filter.session,
      options: <String>['', ...sessions],
      label: (String option) => option.isEmpty
          ? l10n.auditFilterSessionAll
          : auditShortSession(option),
      onChanged: (String option) =>
          ref.read(auditFilterProvider.notifier).setSession(option),
    );
  }
}

/// Eine Auswahl, die bei jedem Druck zum nächsten Wert weitergeht.
///
/// Kein aufklappendes Menü: Die Bibliothek bringt eines mit, aber es gehört
/// gewickelt nach `packages/ui`, und das ist ein eigener Schritt. Bis dahin
/// ist dies ein Knopf mit sichtbarem Zustand — jede Maus-Geste hat damit eine
/// Taste, was `docs/UX.md` 5.1 verlangt, und nichts davon täuscht ein Menü vor.
class _Chooser<T> extends StatelessWidget {
  const _Chooser({
    required this.chooserKey,
    required this.value,
    required this.options,
    required this.label,
    required this.onChanged,
  });

  final Key chooserKey;
  final T value;
  final List<T> options;
  final String Function(T option) label;
  final void Function(T option) onChanged;

  @override
  Widget build(BuildContext context) {
    final int index = options.indexOf(value);
    final T next = options.isEmpty
        ? value
        : options[(index + 1) % options.length];
    return HButton(
      key: chooserKey,
      variant: HButtonVariant.secondary,
      onPressed: options.length <= 1 ? null : () => onChanged(next),
      child: Text(label(value)),
    );
  }
}

/// Ein Feld für eine Zeitgrenze.
///
/// Die Uhrzeit steht in Monospace, weil sie Teil des Belegs ist (`docs/UX.md`
/// 9, Punkt 29), und sie steht in **UTC**, wie im Log: angezeigt mit `Z`,
/// gelesen als UTC, auch wenn jemand das `Z` weglässt. Eine Grenze, die beim
/// Anzeigen UTC und beim Lesen Ortszeit wäre, verschöbe sich bei jedem Hin und
/// Her um den Abstand der Zeitzone.
///
/// Was sich nicht lesen lässt, ändert den Filter **nicht**. Das Feld zeigt
/// dann, dass es nicht gilt, und Tabelle wie Export behalten die Grenze, die
/// vorher galt. Eine Grenze still wegzunehmen, während ihr Text noch dasteht,
/// hieße, über „alle Zeit" zu exportieren, während der Mensch einen Zeitraum
/// sieht (`backlog/CONVENTIONS.md` 4.13).
class _RangeField extends ConsumerStatefulWidget {
  const _RangeField({
    required this.fieldKey,
    required this.label,
    required this.value,
    required this.onChanged,
  });

  final Key fieldKey;
  final String label;
  final DateTime? value;
  final void Function(DateTime? at) onChanged;

  @override
  ConsumerState<_RangeField> createState() => _RangeFieldState();
}

class _RangeFieldState extends ConsumerState<_RangeField> {
  late final TextEditingController _controller = TextEditingController(
    text: _shown(widget.value),
  );

  /// Verlässt der Fokus das Feld, gilt, was darin steht — wie nach Enter.
  ///
  /// Sonst stünde ein getippter, nie bestätigter Zeitraum im Feld, während
  /// Tabelle und Export über alle Zeit gingen (Review 2, Befund 2).
  late final FocusNode _focus = FocusNode(debugLabel: '${widget.fieldKey}')
    ..addListener(_focusChanged);

  bool _invalid = false;

  static String _shown(DateTime? value) =>
      value == null ? '' : auditFormatUtc(value);

  @override
  void didUpdateWidget(_RangeField old) {
    super.didUpdateWidget(old);
    // Wer den Filter von außen ändert oder leert, ändert auch, was hier steht;
    // ein alter Text neben einem neuen Filter wäre eine zweite Wahrheit.
    if (widget.value != old.value) {
      final String shown = _shown(widget.value);
      if (_controller.text != shown) {
        _controller.text = shown;
      }
      if (_invalid) {
        _invalid = false;
        _report(invalid: false, deferred: true);
      }
    }
  }

  @override
  void dispose() {
    _focus
      ..removeListener(_focusChanged)
      ..dispose();
    _controller.dispose();
    super.dispose();
  }

  void _focusChanged() {
    if (!_focus.hasFocus) {
      _submit(_controller.text);
    }
  }

  /// Meldet, ob dieses Feld gerade einen unlesbaren Text hält; ein Export
  /// fängt dann nicht an.
  ///
  /// Mit [deferred] erst nach dem Aufbau: Ein Provider darf nicht geändert
  /// werden, während ein Widget baut, und `didUpdateWidget` läuft im Aufbau.
  void _report({required bool invalid, bool deferred = false}) {
    final String id = '${widget.fieldKey}';
    void mark() =>
        ref.read(auditRangeInvalidProvider.notifier).mark(id, invalid: invalid);
    if (!deferred) {
      mark();
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted) {
        mark();
      }
    });
  }

  void _submit(String text) {
    final String trimmed = text.trim();
    if (trimmed.isEmpty) {
      setState(() => _invalid = false);
      _report(invalid: false);
      if (widget.value != null) {
        widget.onChanged(null);
      }
      return;
    }
    final DateTime? at = auditParseUtc(trimmed);
    if (at == null) {
      setState(() => _invalid = true);
      _report(invalid: true);
      return;
    }
    setState(() => _invalid = false);
    _report(invalid: false);
    _controller.text = auditFormatUtc(at);
    if (at != widget.value) {
      widget.onChanged(at);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SizedBox(
      width: auditRangeFieldWidth,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          HTextField(
            key: widget.fieldKey,
            controller: _controller,
            focusNode: _focus,
            semanticsLabel: widget.label,
            hint: widget.label,
            onSubmitted: _submit,
          ),
          if (_invalid)
            Text(
              key: ValueKey<String>('${widget.fieldKey}-invalid'),
              l10n.auditFilterInvalid,
              style: tokens.typography.ui11.tinted(tokens.state.blocked),
            ),
        ],
      ),
    );
  }
}

/// [at] in UTC als `YYYY-MM-DD HH:MM:SSZ`, die Form des Zeitraumfelds.
String auditFormatUtc(DateTime at) {
  final DateTime utc = at.toUtc();
  String two(int value) => value.toString().padLeft(2, '0');
  return '${utc.year.toString().padLeft(4, '0')}-${two(utc.month)}-'
      '${two(utc.day)} ${two(utc.hour)}:${two(utc.minute)}:${two(utc.second)}Z';
}

/// Eine Zonenangabe am Ende einer Uhrzeit: `Z`, `z`, `+hh`, `+hh:mm`, `+hhmm`.
///
/// Sie muss hinter einer Uhrzeit stehen. Ohne diese Bedingung läse sich das
/// `-11` am Ende von `2026-09-11` als Versatz von elf Stunden.
final RegExp _auditZone = RegExp(
  r'[T ]\d{2}(?::?\d{2}){0,2}(?:[.,]\d+)?\s*(?:[zZ]|[+-]\d{2}(?::?\d{2})?)$',
);

/// Liest eine Zeitgrenze als UTC; null, wenn der Text keine ist.
///
/// ISO 8601, mit `T` oder Leerzeichen zwischen Datum und Uhrzeit, auch nur ein
/// Datum. Ohne Zonenangabe gilt UTC und nicht die Ortszeit, dieselbe Zone, in
/// der das Feld den Wert anzeigt. Mit Zonenangabe (`Z`, `+02:00`, `-0400`)
/// wird umgerechnet.
///
/// **Nie über die Ortszeit.** Ein Text ohne Zone wird nicht als lokaler
/// Zeitpunkt gelesen und danach in Felder zerlegt, sondern bekommt vor dem
/// Lesen ein `Z`. Ein Text mit Zone geht unverändert an `DateTime.tryParse`,
/// das dafür einen UTC-Zeitpunkt liefert; das `toUtc` danach ändert dann
/// nichts und steht nur da, damit das Ergebnis in jedem Fall UTC ist.
///
/// Gemessen am 2026-09-18 mit `TZ=Europe/Berlin` und `TZ=America/New_York`:
/// `2026-09-18T10:00:00+02:00` ergibt in beiden `08:00Z`, `-04:00` ergibt
/// `14:00Z`. Die Vermutung aus Review 2, Befund 1, `tryParse` liefere bei
/// einem Versatz einen lokalen Zeitpunkt, trifft für Dart nicht zu; der Umbau
/// macht den Weg ohne Zone ausdrücklich, statt sich auf die Zerlegung eines
/// lokalen Zeitpunkts zu verlassen.
DateTime? auditParseUtc(String text) {
  final String trimmed = text.trim();
  if (trimmed.isEmpty) {
    return null;
  }
  if (_auditZone.hasMatch(trimmed)) {
    return DateTime.tryParse(trimmed)?.toUtc();
  }
  final String normalized = trimmed.replaceFirst(' ', 'T');
  final String withTime = normalized.contains('T')
      ? normalized
      : '${normalized}T00:00:00';
  final DateTime? parsed = DateTime.tryParse('${withTime}Z');
  return parsed == null || !parsed.isUtc ? null : parsed;
}

/// Wie breit ein Feld einer Zeitgrenze ist.
const double auditRangeFieldWidth = 168;

/// Die Tabelle der Records.
class AuditTable extends ConsumerStatefulWidget {
  /// Legt die Tabelle an. [onOpen] läuft beim Klick auf eine Zeile.
  const AuditTable({required this.onOpen, super.key});

  /// Was ein Klick auf eine Zeile tut.
  final void Function(AuditRecordRow row) onOpen;

  @override
  ConsumerState<AuditTable> createState() => AuditTableState();
}

/// Der Zustand von [AuditTable].
class AuditTableState extends ConsumerState<AuditTable> {
  final ScrollController _vertical = ScrollController();
  final ScrollController _horizontal = ScrollController();

  @override
  void initState() {
    super.initState();
    _vertical.addListener(_scrolled);
  }

  @override
  void dispose() {
    _vertical
      ..removeListener(_scrolled)
      ..dispose();
    _horizontal.dispose();
    super.dispose();
  }

  void _scrolled() {
    if (!_vertical.hasClients) {
      return;
    }
    final ScrollPosition position = _vertical.position;
    final double extent = position.maxScrollExtent;
    if (extent > 0 && position.pixels >= extent * auditLoadMoreAt) {
      final AuditFilter filter = ref.read(auditFilterProvider);
      unawaited(ref.read(auditRecordsProvider(filter).notifier).loadMore());
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AuditFilter filter = ref.watch(auditFilterProvider);
    final AuditRecordsState page = ref.watch(auditRecordsProvider(filter));
    if (page.failure case final Diagnostic failure) {
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: HDiagnosticCard(
          key: const Key('audit-load-failure'),
          code: failure.code,
          severityLabel: l10n.auditSeverityError,
          color: tokens.state.error,
          title: l10n.auditLoadFailedTitle,
          why: failure.why,
          docsUrl: failure.docsUrl,
          width: double.infinity,
          fix: HButton(
            variant: HButtonVariant.secondary,
            onPressed: () => unawaited(
              ref.read(auditRecordsProvider(filter).notifier).reload(),
            ),
            child: Text(l10n.auditReload),
          ),
        ),
      );
    }
    if (page.isEmpty) {
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Align(
          alignment: Alignment.topLeft,
          child: Text(
            key: const Key('audit-empty'),
            l10n.auditEmpty,
            style: tokens.typography.ui13.tinted(tokens.colors.fg1),
          ),
        ),
      );
    }
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double width = constraints.maxWidth > AuditColumns.total
            ? constraints.maxWidth
            : AuditColumns.total;
        return SingleChildScrollView(
          controller: _horizontal,
          scrollDirection: Axis.horizontal,
          child: SizedBox(
            width: width,
            height: constraints.maxHeight,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                _Header(width: width),
                const HHairline(),
                Expanded(
                  child: ListView.builder(
                    controller: _vertical,
                    itemExtent: HSize.rowHistory,
                    itemCount: page.rows.length,
                    itemBuilder: (BuildContext context, int index) {
                      final AuditRecordRow row = page.rows[index];
                      return _Row(
                        key: ValueKey<int>(row.seq),
                        row: row,
                        width: width,
                        onOpen: () => widget.onOpen(row),
                      );
                    },
                  ),
                ),
                if (page.windowFull)
                  Padding(
                    padding: EdgeInsets.all(tokens.spacing.x2),
                    child: Text(
                      key: const Key('audit-window-full'),
                      l10n.auditWindowFull(auditMaxRows),
                      style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                    ),
                  ),
              ],
            ),
          ),
        );
      },
    );
  }
}

/// Die Kopfzeile der Tabelle.
class _Header extends StatelessWidget {
  const _Header({required this.width});

  final double width;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TextStyle style = tokens.typography.ui11.tinted(tokens.colors.fg2);
    return ColoredBox(
      color: tokens.colors.bg1,
      child: SizedBox(
        height: HSize.rowHistory,
        child: Row(
          children: <Widget>[
            _Cell(
              width: AuditColumns.seq,
              alignment: Alignment.centerRight,
              child: Text(l10n.auditColumnSeq, style: style),
            ),
            _Cell(
              width: AuditColumns.time,
              child: Text(l10n.auditColumnTime, style: style),
            ),
            _Cell(
              width: AuditColumns.kind,
              child: Text(l10n.auditColumnKind, style: style),
            ),
            _Cell(
              width: AuditColumns.session,
              child: Text(l10n.auditColumnSession, style: style),
            ),
            Expanded(
              child: _Cell(
                width: null,
                child: Text(l10n.auditColumnSummary, style: style),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Eine Zeile der Tabelle.
class _Row extends StatefulWidget {
  const _Row({
    required this.row,
    required this.width,
    required this.onOpen,
    super.key,
  });

  final AuditRecordRow row;
  final double width;
  final VoidCallback onOpen;

  @override
  State<_Row> createState() => _RowState();
}

class _RowState extends State<_Row> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AuditRecordRow row = widget.row;
    return FocusableActionDetector(
      onShowHoverHighlight: (bool value) => setState(() => _hovered = value),
      onFocusChange: (bool value) => setState(() => _focused = value),
      actions: <Type, Action<Intent>>{
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (ActivateIntent intent) {
            widget.onOpen();
            return null;
          },
        ),
      },
      child: FocusRing(
        visible: _focused,
        radius: tokens.radii.badge,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onOpen,
          child: ColoredBox(
            color: _hovered ? tokens.colors.bg2 : tokens.colors.bg0,
            child: SizedBox(
              height: HSize.rowHistory,
              child: Row(
                children: <Widget>[
                  _Cell(
                    width: AuditColumns.seq,
                    alignment: Alignment.centerRight,
                    child: Text(
                      '${row.seq}',
                      style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                    ),
                  ),
                  _Cell(
                    width: AuditColumns.time,
                    child: Text(
                      auditRowTime(row),
                      style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                    ),
                  ),
                  _Cell(
                    width: AuditColumns.kind,
                    child: Text(
                      row.kind,
                      style: tokens.typography.ui12.tinted(tokens.colors.fg0),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  _Cell(
                    width: AuditColumns.session,
                    child: Text(
                      auditShortSession(row.session),
                      style: tokens.typography.mono12.tinted(tokens.colors.fg2),
                    ),
                  ),
                  Expanded(
                    child: _Cell(
                      width: null,
                      child: Text(
                        auditSummary(row),
                        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                        overflow: TextOverflow.ellipsis,
                        semanticsLabel: l10n.auditRowSemantics(
                          row.seq,
                          row.kind,
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Eine Zelle mit fester oder freier Breite.
class _Cell extends StatelessWidget {
  const _Cell({
    required this.width,
    required this.child,
    this.alignment = Alignment.centerLeft,
  });

  final double? width;
  final Widget child;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Widget padded = Padding(
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      child: Align(alignment: alignment, child: child),
    );
    final double? width = this.width;
    return width == null ? padded : SizedBox(width: width, child: padded);
  }
}

/// Die Sitzung eines Records, der zu keiner gehört
/// (`humanitl_audit::record::NO_SESSION`).
const String auditNoSession = '-';

/// Die Sitzung, auf acht Zeichen gekürzt; `-` bleibt `-`.
String auditShortSession(String session) =>
    session.length <= 8 ? session : session.substring(0, 8);

/// Der Zeitpunkt einer Zeile: Datum und Zeit in Ortszeit, Millisekunden dabei.
///
/// Der Zeitstempel des Records steht unverändert im Sheet; hier steht die Form,
/// die ein Mensch neben einer History-Zeile lesen kann.
String auditRowTime(AuditRecordRow row) {
  final DateTime? at = row.time;
  return at == null ? row.ts : auditFormatTimestamp(at.toLocal());
}

/// [at] als `YYYY-MM-DD HH:MM:SS.mmm`.
String auditFormatTimestamp(DateTime at) {
  String two(int value) => value.toString().padLeft(2, '0');
  return '${at.year.toString().padLeft(4, '0')}-${two(at.month)}-'
      '${two(at.day)} ${two(at.hour)}:${two(at.minute)}:${two(at.second)}'
      '.${at.millisecond.toString().padLeft(3, '0')}';
}

/// Die Zusammenfassung einer Zeile.
///
/// **Nur Felder, die im Record stehen.** Der Flow wird nicht nachgeladen: Das
/// wäre Payload, und die Spalte soll zeigen, was die Kette trägt, nicht was
/// die Aufzeichnung dazu weiß (HUM-051, Fallstricke).
///
/// Für `flow.decided` nennt die Spezifikation `<decision> · <method> · <host>`.
/// Methode und Host stehen in diesem Record **nicht**: `FlowDecided` trägt
/// Flow, Entscheidung, Regel und wer entschieden hat (`kinds.rs`), und
/// Methode und Host stehen in `flow.received`. Angezeigt wird deshalb, was da
/// ist — Methode und Host, sobald ein Record sie führt, sonst wer entschieden
/// hat. Erfunden wird nichts.
String auditSummary(AuditRecordRow row) {
  final Map<String, Object?> data = auditData(row);
  String? text(String key) {
    final Object? value = data[key];
    return value is String && value.isNotEmpty ? value : null;
  }

  switch (row.kind) {
    case 'flow.received':
      return _join(<String?>[text('method'), text('host')]);
    case 'flow.decided':
      return _join(<String?>[
        text('decision'),
        text('method'),
        text('host'),
        if (text('method') == null && text('host') == null) text('decided_by'),
      ]);
    case 'rule.added':
    case 'rule.updated':
    case 'rule.removed':
      return _join(<String?>[text('action'), text('match_host')]);
    case 'config.changed':
      final String? key = text('key');
      if (key == null) {
        return '';
      }
      // Ein geheimer Wert steht nicht im Log, und er steht auch hier nicht:
      // Die Punkte sagen, dass es einen gibt, nicht welcher (`kinds.rs`).
      final String value = data['secret'] == true
          ? '•••'
          : (text('value') ?? '');
      return value.isEmpty ? key : '$key = $value';
    case 'session.started':
      return _join(<String?>[text('agent'), text('profile')]);
    default:
      return '';
  }
}

/// `data` einer Zeile als Abbildung; ein unlesbares JSON ist leer.
Map<String, Object?> auditData(AuditRecordRow row) {
  try {
    final Object? parsed = jsonDecode(row.dataJson);
    return parsed is Map<String, Object?> ? parsed : const <String, Object?>{};
  } on FormatException {
    return const <String, Object?>{};
  }
}

String _join(List<String?> parts) => parts
    .whereType<String>()
    .where((String part) => part.isNotEmpty)
    .join(' · ');
