/// The dry run: what this one rule would have matched among the requests the
/// session recorded.
///
/// It counts, it never decides. "Would have matched 3 of the last 42" is a
/// count of recorded requests, not a promise about the next one and not a
/// decision: another rule above this one can still win, and the panel says so
/// rather than letting the number imply otherwise (CONVENTIONS 4.13).
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/editor.dart';
import '../rule_text.dart';
import '../severity.dart';

/// How many matches the panel lists before it stops.
///
/// The count above the list is the answer; the rows are the evidence for it,
/// and three of them are as much evidence as thirty. It is also the number of
/// skeleton rows [dryRunSkeletonRows] draws, so that the answer replaces its
/// own skeleton without moving anything (`docs/UX.md` 2.11).
const int dryRunRowLimit = 3;

/// How many skeleton rows stand while the answer is on its way.
const int dryRunSkeletonRows = dryRunRowLimit;

/// Width of the method column of a match row: `OPTIONS` in `mono11` plus the
/// gutter, so that the hosts under one another start on one axis.
const double dryRunMethodColumn = 52;

/// The dry run panel of the editor.
class DryRunPanel extends ConsumerWidget {
  /// Creates the panel for [draft].
  const DryRunPanel({required this.draft, super.key});

  /// The rule as the form currently has it.
  final Rule draft;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool readable = rulePassesPreCheck(draft);
    final AsyncValue<DryRun> answer = ref.watch(
      ruleDryRunProvider(dryRunKey(draft)),
    );
    // Kein Wert bedeutet: noch nichts gezählt. Solange darf hier keine Zahl
    // stehen -- „0 von 0" läse sich als geprüft und harmlos, und genau dieses
    // Panel soll eine zu weite Regel aufhalten. Was der Daemon nicht weiß,
    // steht als unbekannt da, nie als Null (`backlog/CONVENTIONS.md` 4.13).
    final DryRun? result = answer.value;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          l10n.rulesDryRunTitle,
          style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
        ),
        SizedBox(height: tokens.spacing.x1),
        if (!readable)
          Text(
            l10n.rulesDryRunWaiting,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          )
        else if (answer.error case final Object error)
          // Ein Probelauf, der scheitert, sagt das an der Stelle, an der die
          // Antwort stünde -- mit dem Code und den Worten des Daemons. Sonst
          // stünde dort für immer „es wird gezählt", und ein Zählsatz ohne
          // Zählung ist die stillste Art, etwas zu behaupten
          // (`docs/UX.md` 4.4).
          //
          // Gefragt ist `error` und nicht der Zustand `AsyncError`: riverpod 3
          // versucht einen gescheiterten Provider von sich aus erneut und
          // meldet währenddessen `AsyncLoading` mit dem Fehler darin. Wer auf
          // den Zustand prüfte, bekäme wieder den Zählsatz zu sehen.
          _DryRunFailed(error: error)
        else
          // Das Panel wartet in sich: der Editor bleibt benutzbar, und nichts
          // außerhalb dieses Kastens bewegt sich (`docs/UX.md` 2.11). Der
          // Satz über den Zeilen steht in jedem der drei Zustände an
          // derselben Stelle, das Gerüst bleibt also stehen, während die
          // Antwort kommt (CONVENTIONS 4.13, Vorhersagbarkeit).
          HWait(
            loading: result == null,
            skeleton: const _Counting(skeleton: true),
            child: result == null
                ? const _Counting(skeleton: false)
                : _Answer(result: result),
          ),
      ],
    );
  }
}

/// Was an der Stelle der Antwort steht, solange keine da ist: der Satz, dass
/// gezählt wird, und der Platz, den die Zeilen einnehmen werden.
class _Counting extends StatelessWidget {
  const _Counting({required this.skeleton});

  /// Ob die Zeilen schon als Skelett stehen. [HWait] entscheidet das: unter
  /// seiner Schwelle bleibt der Platz leer, damit nichts flackert.
  final bool skeleton;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final double rows = dryRunSkeletonRows * tokens.sizes.rowHistory;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          l10n.rulesDryRunCounting,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(
          l10n.rulesDryRunOnlyThisRule,
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x2),
        if (skeleton)
          HSkeleton(
            rows: dryRunSkeletonRows,
            rowHeight: tokens.sizes.rowHistory,
          )
        else
          SizedBox(height: rows),
      ],
    );
  }
}

/// Warum kein Probelauf lief, mit den Worten des Daemons.
class _DryRunFailed extends StatelessWidget {
  const _DryRunFailed({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Object failure = error;
    final Diagnostic diagnostic = failure is DaemonException
        ? failure.diagnostic
        : Diagnostic(
            code: DiagnosticCodes.rulesRequestInvalid,
            severity: Severity.error,
            why: '$failure',
          );
    return HDiagnosticCard(
      code: diagnostic.code,
      severityLabel: ruleSeverityLabel(l10n, diagnostic.severity),
      color: ruleSeverityColor(tokens, diagnostic.severity),
      title: l10n.rulesDryRunFailedTitle,
      // Der Satz des Daemons, nie ein umgeschriebener (`docs/UX.md` 4.4).
      why: diagnostic.why,
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
    );
  }
}

/// Die Antwort: die Zahl, der Bezug und die Belege dafür.
class _Answer extends StatelessWidget {
  const _Answer({required this.result});

  final DryRun result;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          l10n.rulesDryRunResult(result.matches.length, result.scanned),
          style: tokens.typography.ui13.tinted(tokens.colors.fg0),
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(
          l10n.rulesDryRunOnlyThisRule,
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x2),
        _Matches(result: result),
      ],
    );
  }
}

class _Matches extends StatelessWidget {
  const _Matches({required this.result});

  final DryRun result;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    if (result.scanned == 0) {
      return Text(
        l10n.rulesDryRunNothingRecorded,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      );
    }
    if (result.matches.isEmpty) {
      return Text(
        l10n.rulesDryRunNoMatch,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      );
    }
    final List<Flow> shown = result.matches.take(dryRunRowLimit).toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final Flow flow in shown) _MatchRow(flow: flow),
        if (result.matches.length > shown.length)
          Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x1),
            child: Text(
              l10n.rulesDryRunMore(result.matches.length - shown.length),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
      ],
    );
  }
}

/// One recorded request the rule would have matched: when it happened, what
/// it was and where it went. No state colour: nothing was decided here.
class _MatchRow extends StatelessWidget {
  const _MatchRow({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // The density of a history row, as a minimum: a larger text scale makes
    // the row taller instead of cutting it off (`docs/UX.md` 3.2 and 6).
    return ConstrainedBox(
      constraints: BoxConstraints(minHeight: tokens.sizes.rowHistory),
      child: Row(
        children: <Widget>[
          Text(
            l10n.rulesDryRunTime(flow.receivedAt.toLocal()),
            // `fg1`, nie `fg2`: der Zeitpunkt ist der Beleg dafür, dass die
            // Regel diese Anfrage getroffen hätte, und `fg2` ist wirklich
            // deaktivierten Controls vorbehalten (`docs/UX.md` 6).
            style: tokens.typography.mono11.tinted(tokens.colors.fg1),
          ),
          SizedBox(width: tokens.spacing.x2),
          SizedBox(
            width: dryRunMethodColumn,
            child: Text(
              flow.methodLabel,
              style: tokens.typography.mono11.tinted(tokens.colors.fg1),
            ),
          ),
          Text(
            flow.host,
            style: tokens.typography.mono12.tinted(tokens.colors.fg0),
          ),
          SizedBox(width: tokens.spacing.x2),
          Expanded(
            child: Text(
              flow.path,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            ),
          ),
        ],
      ),
    );
  }
}
