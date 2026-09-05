/// The form: one rule, field by field, with the sentence it makes and the
/// requests it would have matched under it.
///
/// The form pre-checks only what it can judge on its own -- an empty host, a
/// wildcard that shares a label -- and says so while somebody types. Whether
/// the rule is legal is the daemon's answer, and when it refuses, its own
/// diagnostic stands under the form with the field and the line it names
/// (`docs/UX.md` 4.4).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/text/rule_sentence.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/editor.dart';
import '../providers/rules.dart';
import '../rule_text.dart';
import '../severity.dart';
import 'dry_run_panel.dart';
import 'rule_fields.dart';
import 'rule_row.dart';

/// Which lifetime the segmented control shows.
enum _Lifetime { never, session, at }

/// Width of the port field: five digits and the gutter. It stands beside the
/// scheme, which takes the rest of the line.
const double rulePortFieldWidth = 96;

/// The editor pane.
class RuleEditor extends ConsumerStatefulWidget {
  /// Creates the editor.
  const RuleEditor({super.key});

  @override
  ConsumerState<RuleEditor> createState() => _RuleEditorState();
}

class _RuleEditorState extends ConsumerState<RuleEditor> {
  final TextEditingController _host = TextEditingController();
  final TextEditingController _path = TextEditingController();
  final TextEditingController _port = TextEditingController();
  final TextEditingController _note = TextEditingController();
  final TextEditingController _endsAt = TextEditingController();
  int _generation = -1;
  String? _endsAtError;

  /// Whether the form has anything to complain about yet.
  ///
  /// A pristine empty host is not a mistake somebody made; it is a field
  /// nobody has filled in. The reason appears as soon as somebody types, or
  /// when they press `Save` -- which then says why instead of doing nothing
  /// (`docs/UX.md` 5.3).
  bool _showErrors = false;

  @override
  void dispose() {
    _host.dispose();
    _path.dispose();
    _port.dispose();
    _note.dispose();
    _endsAt.dispose();
    super.dispose();
  }

  /// Refills the controllers, and only when the editor was opened on
  /// something else. Refilling while somebody types would move the cursor.
  void _syncControllers(RuleEditorState editor) {
    if (_generation == editor.generation) {
      return;
    }
    _generation = editor.generation;
    final Rule? draft = editor.draft;
    _host.text = draft?.matcher.host ?? '';
    _path.text = draft?.matcher.path ?? '';
    _port.text = (draft?.matcher.port ?? 0) == 0
        ? ''
        : '${draft!.matcher.port}';
    _note.text = draft?.note ?? '';
    _endsAt.text = switch (draft?.expires) {
      RuleExpiryAt(:final DateTime at) => _isoMinutes(at.toLocal()),
      _ => '',
    };
    _endsAtError = null;
    _showErrors = false;
  }

  RuleEditorController get _editor => ref.read(ruleEditorProvider.notifier);

  void _setLifetime(_Lifetime lifetime, Rule draft) {
    // Der alte Fehlertext gehört zu einem Text, der gleich nicht mehr im Feld
    // steht. Bliebe er stehen, verweigerte `Save` mit einer Begründung, die
    // nichts mehr beschreibt (`docs/UX.md` 4.4).
    setState(() => _endsAtError = null);
    switch (lifetime) {
      case _Lifetime.never:
        _editor.setDraft(draft.copyWith(expires: const RuleExpiry.never()));
      case _Lifetime.session:
        _editor.setDraft(draft.copyWith(expires: const RuleExpiry.session()));
      case _Lifetime.at:
        final DateTime at = DateTime.now().add(const Duration(hours: 1));
        _endsAt.text = _isoMinutes(at);
        _editor.setDraft(draft.copyWith(expires: RuleExpiry.at(at: at)));
    }
  }

  /// Nimmt den getippten Endzeitpunkt an. [_endsAtError] trägt nur, was am
  /// Text liegt: eine Zeichenkette, die kein Zeitpunkt ist.
  void _setEndsAt(String text, Rule draft, AppLocalizations l10n) {
    _showErrors = true;
    final DateTime? parsed = DateTime.tryParse(text);
    setState(() {
      _endsAtError = parsed == null ? l10n.rulesEndsAtInvalid : null;
    });
    if (parsed != null) {
      // Der Entwurf nimmt auch den Zeitpunkt von gestern an, damit Vorschau
      // und Probelauf zeigen, was da steht; gespeichert wird er nicht, weil
      // `_endsAtProblem` daran hängt.
      _editor.setDraft(draft.copyWith(expires: RuleExpiry.at(at: parsed)));
    }
  }

  /// Was am Endzeitpunkt nicht stimmt: unlesbar, oder vorbei.
  ///
  /// Die zweite Prüfung liest die Uhr jedes Mal neu, beim Aufbau wie beim
  /// Druck auf `Save`. Ein Zeitpunkt, der beim Tippen eine Minute in der
  /// Zukunft lag, ist fünf Minuten später Vergangenheit, und ein vergangenes
  /// `at` lehnt auch `RulesStore::validated` nicht ab: die Regel wäre
  /// gespeichert und sofort abgelaufen. Bei einer `block`-Regel ist das die
  /// weitende Richtung -- jemand glaubt zu blockieren und blockiert nichts
  /// (`backlog/CONVENTIONS.md` 4.13).
  String? _endsAtProblem(Rule draft, AppLocalizations l10n) =>
      switch (draft.expires) {
        RuleExpiryAt(:final DateTime at) =>
          _endsAtError ??
              (at.isAfter(DateTime.now()) ? null : l10n.rulesEndsAtPast),
        RuleExpiryNever() || RuleExpirySession() => null,
      };

  /// Ob das Formular gerade selbst weiß, dass die Regel nicht gehen kann.
  ///
  /// Wird beim Druck erneut gerufen und nicht aus dem letzten Aufbau
  /// übernommen: zwischen Aufbau und Druck vergeht Zeit, und eine der drei
  /// Prüfungen hängt an der Uhr.
  bool _refused(Rule draft, AppLocalizations l10n) =>
      hostProblemText(draft.matcher.host, l10n) != null ||
      pathProblemText(draft.matcher.path, l10n) != null ||
      _endsAtProblem(draft, l10n) != null;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final RuleEditorState editor = ref.watch(ruleEditorProvider);
    _syncControllers(editor);
    final Rule? draft = editor.draft;
    if (draft == null) {
      return const _NoRuleOpen();
    }
    final bool enabled = !editor.readOnly;
    // What the form can judge on its own. Every one of them blocks `Save`,
    // not only the host: a path the engine cannot build and a time nobody can
    // read are just as unusable (`docs/UX.md` 4.4).
    final String? hostProblem = hostProblemText(draft.matcher.host, l10n);
    final String? pathProblem = pathProblemText(draft.matcher.path, l10n);
    final String? endsAtProblem = _endsAtProblem(draft, l10n);
    final String? hostError = _showErrors ? hostProblem : null;
    final String? pathError = _showErrors ? pathProblem : null;

    // The form scrolls, the two decisions do not: the same action always
    // stands in the same place, and `Save` is the filled control of the
    // screen while the editor is open (`docs/UX.md` 3.1, CONVENTIONS 4.13).
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(
          child: ListView(
            padding: EdgeInsets.all(tokens.spacing.x3),
            children: <Widget>[
              _Heading(editor: editor),
              SizedBox(height: tokens.spacing.x3),
              if (editor.readOnly) ...<Widget>[
                _BundledNotice(rule: draft),
                SizedBox(height: tokens.spacing.x3),
              ],
              RuleField(
                label: l10n.rulesFieldAction,
                child: HSegmented<RuleAction>(
                  enabled: enabled,
                  selected: draft.action,
                  onSelect: (RuleAction action) =>
                      _editor.setDraft(draft.copyWith(action: action)),
                  options: <HSegmentOption<RuleAction>>[
                    for (final RuleAction action in RuleAction.values)
                      HSegmentOption<RuleAction>(
                        value: action,
                        label: ruleActionWord(action, l10n),
                        leading: ruleActionGlyph(
                          action,
                          ruleActionTextColor(action, tokens),
                        ),
                      ),
                  ],
                ),
              ),
              RuleField(
                label: l10n.rulesFieldHost,
                error: hostError,
                hint: l10n.rulesHostHelp,
                child: HTextField(
                  key: const Key('rule-host'),
                  controller: _host,
                  enabled: enabled,
                  semanticsLabel: l10n.rulesFieldHost,
                  hint: l10n.rulesHostPlaceholder,
                  onChanged: (String value) {
                    // Somebody has answered for the field now, so the form
                    // may say what is wrong with the answer.
                    setState(() => _showErrors = true);
                    _editor.setMatcher(
                      draft.matcher.copyWith(host: value.trim()),
                    );
                  },
                ),
              ),
              RuleField(
                label: l10n.rulesFieldMethods,
                hint: l10n.rulesMethodsHelp,
                child: HChoiceChips<Method>(
                  enabled: enabled,
                  selected: draft.matcher.methods.toSet(),
                  onToggle: _editor.toggleMethod,
                  options: <HSegmentOption<Method>>[
                    for (final Method method in _offeredMethods)
                      HSegmentOption<Method>(
                        value: method,
                        label: method.token,
                      ),
                  ],
                ),
              ),
              RuleField(
                label: l10n.rulesFieldPath,
                error: pathError,
                child: HTextField(
                  key: const Key('rule-path'),
                  controller: _path,
                  enabled: enabled,
                  semanticsLabel: l10n.rulesFieldPath,
                  hint: l10n.rulesPathPlaceholder,
                  onChanged: (String value) {
                    setState(() => _showErrors = true);
                    _editor.setMatcher(
                      draft.matcher.copyWith(path: value.trim()),
                    );
                  },
                ),
              ),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Expanded(
                    child: RuleField(
                      label: l10n.rulesFieldScheme,
                      child: HSegmented<Scheme?>(
                        enabled: enabled,
                        selected: draft.matcher.scheme,
                        // `copyWith` reaches null here: freezed carries a
                        // sentinel for nullable fields, so "any scheme" is
                        // expressible without rebuilding the matcher by hand.
                        onSelect: (Scheme? scheme) => _editor.setMatcher(
                          draft.matcher.copyWith(scheme: scheme),
                        ),
                        options: <HSegmentOption<Scheme?>>[
                          HSegmentOption<Scheme?>(
                            value: null,
                            label: l10n.rulesSchemeAny,
                          ),
                          for (final Scheme scheme in Scheme.values)
                            HSegmentOption<Scheme?>(
                              value: scheme,
                              label: scheme.name,
                            ),
                        ],
                      ),
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x3),
                  SizedBox(
                    width: rulePortFieldWidth,
                    child: RuleField(
                      label: l10n.rulesFieldPort,
                      child: HTextField(
                        key: const Key('rule-port'),
                        controller: _port,
                        enabled: enabled,
                        digitsOnly: true,
                        semanticsLabel: l10n.rulesFieldPort,
                        hint: l10n.rulesPortPlaceholder,
                        onChanged: (String value) => _editor.setMatcher(
                          draft.matcher.copyWith(
                            port: int.tryParse(value) ?? 0,
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
              RuleField(
                label: l10n.rulesFieldUpgrade,
                child: HCheckbox(
                  enabled: enabled,
                  label: l10n.rulesUpgradeWebsocket,
                  hint: l10n.rulesUpgradeHelp,
                  value: draft.matcher.upgrade == Upgrade.websocket,
                  onChanged: (bool on) => _editor.setMatcher(
                    draft.matcher.copyWith(
                      upgrade: on ? Upgrade.websocket : null,
                    ),
                  ),
                ),
              ),
              RuleField(
                label: l10n.rulesFieldExpires,
                error: _showErrors ? endsAtProblem : null,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    HSegmented<_Lifetime>(
                      enabled: enabled,
                      selected: switch (draft.expires) {
                        RuleExpiryNever() => _Lifetime.never,
                        RuleExpirySession() => _Lifetime.session,
                        RuleExpiryAt() => _Lifetime.at,
                      },
                      onSelect: (_Lifetime lifetime) =>
                          _setLifetime(lifetime, draft),
                      options: <HSegmentOption<_Lifetime>>[
                        HSegmentOption<_Lifetime>(
                          value: _Lifetime.session,
                          label: l10n.rulesExpiresSession,
                        ),
                        HSegmentOption<_Lifetime>(
                          value: _Lifetime.at,
                          label: l10n.rulesExpiresAt,
                        ),
                        HSegmentOption<_Lifetime>(
                          value: _Lifetime.never,
                          label: l10n.rulesExpiresNever,
                        ),
                      ],
                    ),
                    if (draft.expires is RuleExpiryAt) ...<Widget>[
                      SizedBox(height: tokens.spacing.x2),
                      HTextField(
                        key: const Key('rule-ends-at'),
                        controller: _endsAt,
                        enabled: enabled,
                        semanticsLabel: l10n.rulesFieldExpires,
                        hint: l10n.rulesEndsAtPlaceholder,
                        onChanged: (String value) =>
                            _setEndsAt(value.trim(), draft, l10n),
                      ),
                    ],
                  ],
                ),
              ),
              RuleField(
                label: l10n.rulesFieldNote,
                hint: l10n.rulesNoteHelp,
                child: HTextField(
                  key: const Key('rule-note'),
                  controller: _note,
                  enabled: enabled,
                  mono: false,
                  semanticsLabel: l10n.rulesFieldNote,
                  onChanged: (String value) =>
                      _editor.setDraft(draft.copyWith(note: value)),
                ),
              ),
              // Hier stand die Stream-Checkbox (`rulesFieldStream`,
              // `rulesStream`, `rulesStreamHelp`). Sie ist das eine Control
              // dieses Bildschirms, das einen Rumpf über der Kappe ungelesen
              // hinauslässt, also die eine Weitung des offenen Datenpfads,
              // und die Spezifikation zeigt sie nur im Expert-Tier. Einen
              // Tier-Begriff gibt es in `app/lib` erst mit HUM-069; bis dahin
              // ist ein sichtbares Feld ohne diesen Schutz die falsche
              // Zwischenlösung, weil es die Weitung leichter macht als die
              // Einengung (CONVENTIONS 4.16). Das Feld der Regel bleibt
              // unangetastet: der Entwurf trägt weiter, was der Daemon
              // geliefert hat.
            ],
          ),
        ),
        const HHairline(),
        Padding(
          padding: EdgeInsets.all(tokens.spacing.x3),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              // The two things that say what will happen stand still while
              // the form scrolls: the sentence the rule makes, and what that
              // rule would have matched. Scrolling them out of sight would
              // make the check optional (`docs/UX.md` 1.1, Rules).
              _Preview(draft: draft),
              SizedBox(height: tokens.spacing.x3),
              DryRunPanel(draft: draft),
              SizedBox(height: tokens.spacing.x3),
              _Actions(
                editor: editor,
                draft: draft,
                refused: () => _refused(draft, l10n),
                onRefused: () => setState(() => _showErrors = true),
              ),
              if (editor.error case final Diagnostic error) ...<Widget>[
                SizedBox(height: tokens.spacing.x3),
                _SaveFailed(diagnostic: error),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

/// The verbs a rule is written with. `CONNECT` and `TRACE` are left out: no
/// agent sends them through this proxy, and a chip nobody uses is a chip
/// somebody has to read past.
const List<Method> _offeredMethods = <Method>[
  Method.get,
  Method.head,
  Method.post,
  Method.put,
  Method.patch,
  Method.delete,
  Method.options,
];

String _isoMinutes(DateTime at) =>
    at.toIso8601String().split('.').first.substring(0, 16);

/// The title of the pane.
class _Heading extends StatelessWidget {
  const _Heading({required this.editor});

  final RuleEditorState editor;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Text(
      editor.readOnly
          ? l10n.rulesEditorBundledTitle
          : editor.isNew
          ? l10n.rulesEditorNewTitle
          : l10n.rulesEditorTitle,
      style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
    );
  }
}

/// Why the form is grey, what state the rule is in, and what to do instead.
///
/// Der Zustand steht hier und nicht nur in der Zeile: der Editor ist die
/// größere Hälfte des Bildschirms, und eine ausgeschaltete Regel, die hier in
/// voller Stärke mit Aktion, Host und Frist steht, liest als wirksam. Das ist
/// dieselbe Behauptung, die dieses Feature in der Liste abstellt, eine Ebene
/// tiefer (CONVENTIONS 4.13). Die drei Kanäle sind deshalb dieselben wie in
/// der Zeile: Zeichen, Farbe und Wort.
class _BundledNotice extends ConsumerWidget {
  const _BundledNotice({required this.rule});

  final Rule rule;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg2,
        borderRadius: HRadius.controlRadius,
        border: Border.all(color: tokens.colors.line),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                HGlyphIcon(
                  rule.disabled ? HGlyph.close : HGlyph.lock,
                  size: 14,
                  color: rule.disabled
                      ? tokens.stateTextColor(HFlowState.timedOut)
                      : tokens.colors.fg1,
                ),
                SizedBox(width: tokens.spacing.x2),
                Text(
                  l10n.rulesBundledTitle,
                  style: tokens.typography.ui13.medium.tinted(
                    tokens.colors.fg0,
                  ),
                ),
              ],
            ),
            if (rule.disabled) ...<Widget>[
              SizedBox(height: tokens.spacing.x1),
              // Eine Feststellung, kein Angebot: der Satz darunter sagt, dass
              // man eine mitgelieferte Regel abschalten kann, und ohne diese
              // Zeile läse sich das, als wäre sie noch an.
              Text(
                l10n.rulesOriginBundledOff,
                style: tokens.typography.ui12.medium.tinted(
                  tokens.stateTextColor(HFlowState.timedOut),
                ),
              ),
            ],
            SizedBox(height: tokens.spacing.x1),
            Text(
              l10n.rulesBundledWhy,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ],
        ),
      ),
    );
  }
}

/// The rule as one sentence, in the largest type of the screen.
///
/// It stands above the buttons because it is what gets saved: whoever reads
/// it has read the rule, and whoever does not has still seen it
/// (`docs/UX.md` 4.6, "the rule as a sentence before Remember").
class _Preview extends StatelessWidget {
  const _Preview({required this.draft});

  final Rule draft;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          l10n.rulesPreviewTitle,
          style: tokens.typography.ui12.medium.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(
          ruleSentence(draft, l10n, now: DateTime.now()),
          style: tokens.typography.ui14.tinted(tokens.colors.fg0),
        ),
      ],
    );
  }
}

/// Save, cancel, make permanent -- and for a bundled rule the one thing that
/// can be done with it: put a rule of one's own in front.
class _Actions extends ConsumerWidget {
  const _Actions({
    required this.editor,
    required this.draft,
    required this.refused,
    required this.onRefused,
  });

  final RuleEditorState editor;
  final Rule draft;

  /// Fragt beim Druck, ob das Formular selbst schon weiß, dass die Regel
  /// nicht gehen kann. Ein Wert von vorhin täte es nicht: eine der Prüfungen
  /// hängt an der Uhr.
  final ValueGetter<bool> refused;

  /// Called instead of saving, so that the form says why.
  final VoidCallback onRefused;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final RuleEditorController controller = ref.read(
      ruleEditorProvider.notifier,
    );
    if (editor.readOnly) {
      return Row(
        children: <Widget>[
          HButton(
            key: const Key('rule-override'),
            variant: HButtonVariant.primary,
            size: HButtonSize.md,
            onPressed: () => controller.overrideBundled(draft),
            child: Text(l10n.rulesOverrideBundled),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            variant: HButtonVariant.ghost,
            onPressed: controller.close,
            child: Text(l10n.rulesClose),
          ),
        ],
      );
    }
    // „Dauerhaft machen" arbeitet auf der gespeicherten Regel, nie auf dem
    // Formular: der Daemon bekommt nur die Id, ändert also die Regel, die er
    // hat, und der Rückgängig-Streifen muss denselben Zustand zurückschreiben
    // können. Käme der Zustand aus dem Entwurf, machte ein Klick auf „Undo"
    // aus einer ungespeicherten Änderung eine gespeicherte -- eine
    // Schaltfläche mit der Aufschrift „Undo", die weitet, was den Rechner
    // verlässt (`docs/UX.md` 4.5, CONVENTIONS 4.13).
    final Rule? saved = ruleById(
      ref.watch(rulesProvider).value,
      editor.editing,
    );
    // Die gespeicherte Regel, sofern sie ein Ende hat: nur eine solche kann
    // dauerhaft werden, und der Daemon lehnt jede andere mit IPC_005 ab.
    final Rule? endsSomeday = saved != null && saved.expires is! RuleExpiryNever
        ? saved
        : null;
    final bool diverged = saved != null && saved != draft;
    return Wrap(
      spacing: tokens.spacing.x2,
      runSpacing: tokens.spacing.x2,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: <Widget>[
        HButton(
          key: const Key('rule-save'),
          variant: HButtonVariant.primary,
          size: HButtonSize.md,
          // Never a control that is dead without a reason: a press that
          // cannot be carried out uncovers the reasons instead of doing
          // nothing (`docs/UX.md` 5.3).
          onPressed: editor.saving
              ? null
              : () {
                  if (refused()) {
                    onRefused();
                    return;
                  }
                  unawaited(controller.save());
                },
          child: Text(l10n.rulesSave),
        ),
        HButton(
          key: const Key('rule-cancel'),
          variant: HButtonVariant.ghost,
          size: HButtonSize.md,
          onPressed: controller.close,
          child: Text(l10n.rulesCancel),
        ),
        if (endsSomeday case final Rule stored) ...<Widget>[
          HButton(
            key: const Key('rule-make-permanent'),
            variant: HButtonVariant.ghost,
            size: HButtonSize.md,
            // Gesperrt, solange Entwurf und gespeicherte Regel
            // auseinanderlaufen: der Knopf spräche sonst über eine andere
            // Regel als die, die im Formular steht.
            onPressed: diverged
                ? null
                : () async {
                    final Diagnostic? failed = await ref
                        .read(rulesProvider.notifier)
                        .makePermanent(stored);
                    if (failed != null) {
                      ref.read(rulesBannerProvider.notifier).showOne(failed);
                    } else {
                      controller.close();
                    }
                  },
            child: Text(l10n.rulesMakePermanent),
          ),
          if (diverged)
            Text(
              l10n.rulesMakePermanentDirty,
              // Der Grund wird gelesen, also `fg1`; `fg2` gehört dem
              // gesperrten Control daneben (`docs/UX.md` 6).
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
        ],
      ],
    );
  }
}

/// The daemon's own words about a rule it refused, under the form.
class _SaveFailed extends StatelessWidget {
  const _SaveFailed({required this.diagnostic});

  final Diagnostic diagnostic;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return HDiagnosticCard(
      code: diagnostic.code,
      severityLabel: ruleSeverityLabel(l10n, diagnostic.severity),
      color: ruleSeverityColor(tokens, diagnostic.severity),
      title: l10n.rulesSaveFailedTitle,
      // The daemon's sentence, with the field and the line it names, never a
      // rewritten one: the app's own text is the title, not the reason
      // (`docs/UX.md` 4.4).
      why: diagnostic.why,
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
    );
  }
}

/// What the pane says while no rule is open. A statement about a finished
/// thing, not an empty state: the list beside it is the thing to act on.
class _NoRuleOpen extends StatelessWidget {
  const _NoRuleOpen();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Center(
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Text(
          l10n.rulesEditorClosed,
          textAlign: TextAlign.center,
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        ),
      ),
    );
  }
}
