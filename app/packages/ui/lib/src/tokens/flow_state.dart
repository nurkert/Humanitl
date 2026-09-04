import 'package:flutter/widgets.dart';

import 'colors.dart';

/// The eight visual states a flow can be in.
///
/// This is a *presentation* enum, not the Rust `FlowState`. Several backend
/// states collapse into one appearance (a rule allow and a user allow differ
/// only by [autoRule]), and one backend state splits into two ([allowed] and
/// [allowedEdited]).
enum HFlowState {
  /// Waiting for a human decision.
  held,

  /// Allowed unchanged.
  allowed,

  /// Allowed after the request was edited.
  allowedEdited,

  /// Blocked, by the user or by a rule.
  blocked,

  /// The hold budget ran out before anyone decided.
  timedOut,

  /// A rule decided without asking.
  autoRule,

  /// Passed through to the configured LLM endpoint.
  passthroughLlm,

  /// The request failed, or a secret was found in it.
  error,
}

/// The vector glyphs of the design system.
///
/// The shapes follow the Lucide icons named in HUM-008. They are painted by
/// `HGlyphIcon` instead of being loaded from an icon font: `packages/ui` has no
/// component library yet, and a self-painted glyph is one fewer dependency to
/// unwind when one arrives.
enum HGlyph {
  /// Lucide `hourglass`: a request is held.
  hourglass,

  /// Lucide `arrow-up-right`: a request left the airlock.
  arrowUpRight,

  /// Lucide `arrow-up-right` with a pencil dot: it left after an edit.
  arrowUpRightPencil,

  /// Lucide `shield-x`: a request was blocked.
  shieldX,

  /// Lucide `clock-x`: a hold expired.
  clockX,

  /// Lucide `zap`: a rule decided.
  bolt,

  /// Lucide `chevrons-right`: traffic passes through.
  chevronsRight,

  /// Lucide `triangle-alert`: an error, or a secret.
  triangleAlert,

  /// Lucide `chevron-right`: the right half of a split pill.
  chevronRight,

  /// Lucide `x`: dismiss a sheet or a modal.
  close,

  /// Lucide `grip-vertical`: der Griff, an dem eine Zeile gezogen wird.
  grip,

  /// Lucide `trash-2`: etwas löschen.
  trash,

  /// Lucide `plus`: etwas anlegen.
  plus,

  /// Lucide `lock`: mitgeliefert, unveränderlich.
  lock,

  /// Ein Schwärzungsbalken: die Handlung, die eine Anfrage mit geschwärzten
  /// Fundstellen durchlässt. Lucide hat dafür keine Form, und `eye-off` sagt
  /// „versteckt", was etwas anderes ist.
  redactBar,
}

/// The eight state colours of one theme.
@immutable
class HStateColors {
  /// Creates a state palette. Use [dark] or [light].
  const HStateColors({
    required this.held,
    required this.allowed,
    required this.allowedEdited,
    required this.blocked,
    required this.timedOut,
    required this.autoRule,
    required this.passthroughLlm,
    required this.error,
  });

  /// The dark palette, literally the hexes of BACKLOG.md 5.
  static const HStateColors dark = HStateColors(
    held: HColors.held,
    allowed: HColors.allowed,
    allowedEdited: HColors.allowedEdited,
    blocked: HColors.blocked,
    timedOut: HColors.timedOut,
    autoRule: HColors.autoRule,
    passthroughLlm: HColors.passthrough,
    error: HColors.secret,
  );

  /// The light palette, derived from [dark] by [HColorDerivation.lightState].
  ///
  /// Never hand-written: the derivation is twelve percent darker in HSL plus a
  /// clamp that keeps every colour at 3:1 over the light surfaces.
  static final HStateColors light = HStateColors(
    held: HColorDerivation.lightState(HColors.held),
    allowed: HColorDerivation.lightState(HColors.allowed),
    allowedEdited: HColorDerivation.lightState(HColors.allowedEdited),
    blocked: HColorDerivation.lightState(HColors.blocked),
    timedOut: HColorDerivation.lightState(HColors.timedOut),
    autoRule: HColorDerivation.lightState(HColors.autoRule),
    passthroughLlm: HColorDerivation.lightState(HColors.passthrough),
    error: HColorDerivation.lightState(HColors.secret),
  );

  /// Die dunkle Palette, die ein Wort tragen darf.
  ///
  /// Die Flächenpalette [dark] ist auf 3:1 geklemmt; das reicht für eine Rail
  /// und für einen Bogen, nicht für einen Satz. Diese hier ist dieselbe Farbe,
  /// von den dunklen Flächen weg aufgehellt, bis sie auf jeder Fläche aus
  /// [HColorDerivation.textBackgrounds] 4,5:1 erreicht — also auch auf ihrer
  /// eigenen Tönung und auf ihrer eigenen Füllung (`docs/UX.md` 6).
  ///
  /// Die Flächen bleiben, wo sie sind: nur Text wechselt auf diese Palette.
  static final HStateColors darkText = _textOf(
    dark,
    HColorDerivation.darkSurfaces,
  );

  /// Die helle Palette, die ein Wort tragen darf.
  ///
  /// Der Grund, aus dem es sie gibt: [light] wird von
  /// [HColorDerivation.lightState] auf 3:1 geklemmt, und damit konnte im
  /// hellen Theme keine Zustandsfarbe legal Text tragen. Zwei Screens haben
  /// das umgangen, indem sie das Wort auf `fg0` umgeschaltet haben, sobald
  /// eine Füllung darunter lief. Diese Palette ist die Farbe selbst,
  /// abgedunkelt bis 4,5:1 (`docs/UX.md` 6 und 9, Punkt 24).
  static final HStateColors lightText = _textOf(
    light,
    HColorDerivation.lightSurfaces,
  );

  /// Leitet aus einer Flächenpalette die Textpalette über [surfaces] ab.
  static HStateColors _textOf(HStateColors area, List<Color> surfaces) {
    Color text(HFlowState state) =>
        HColorDerivation.textVariant(area.resolve(state), surfaces: surfaces);
    return HStateColors(
      held: text(HFlowState.held),
      allowed: text(HFlowState.allowed),
      allowedEdited: text(HFlowState.allowedEdited),
      blocked: text(HFlowState.blocked),
      timedOut: text(HFlowState.timedOut),
      autoRule: text(HFlowState.autoRule),
      passthroughLlm: text(HFlowState.passthroughLlm),
      error: text(HFlowState.error),
    );
  }

  /// Waiting for a decision.
  final Color held;

  /// Allowed unchanged.
  final Color allowed;

  /// Allowed after an edit.
  final Color allowedEdited;

  /// Blocked.
  final Color blocked;

  /// Timed out.
  final Color timedOut;

  /// Decided by a rule, deliberately dimmer than [allowed].
  final Color autoRule;

  /// Passed through to the LLM endpoint.
  final Color passthroughLlm;

  /// Error or secret found.
  final Color error;

  /// The colour of [state] in this palette.
  Color resolve(HFlowState state) => switch (state) {
    HFlowState.held => held,
    HFlowState.allowed => allowed,
    HFlowState.allowedEdited => allowedEdited,
    HFlowState.blocked => blocked,
    HFlowState.timedOut => timedOut,
    HFlowState.autoRule => autoRule,
    HFlowState.passthroughLlm => passthroughLlm,
    HFlowState.error => error,
  };

  /// The eight colours in the order of [HFlowState.values].
  List<Color> get all => HFlowState.values.map(resolve).toList(growable: false);
}

/// The canonical lookup for a state colour.
///
/// `backlog/CONVENTIONS.md` 3.9 names this entry point; the extension
/// [HFlowStateColor] is the same table read from the other side.
abstract final class FlowStateColor {
  /// The colour of [state] for [brightness], dark unless told otherwise.
  static Color of(
    HFlowState state, [
    Brightness brightness = Brightness.dark,
  ]) => palette(brightness).resolve(state);

  /// The whole palette of [brightness].
  static HStateColors palette(Brightness brightness) =>
      brightness == Brightness.dark ? HStateColors.dark : HStateColors.light;

  /// Die Farbe, in der [state] als *Text* stehen darf, für [brightness].
  ///
  /// Eine Fläche darf 3:1 messen, ein Wort nicht (`docs/UX.md` 6).
  static Color text(
    HFlowState state, [
    Brightness brightness = Brightness.dark,
  ]) => textPalette(brightness).resolve(state);

  /// Die ganze Textpalette von [brightness].
  static HStateColors textPalette(Brightness brightness) =>
      brightness == Brightness.dark
      ? HStateColors.darkText
      : HStateColors.lightText;
}

/// Reading a state's appearance from the state itself.
extension HFlowStateColor on HFlowState {
  /// The colour of this state for [brightness].
  Color color(Brightness brightness) => FlowStateColor.of(this, brightness);

  /// Die Farbe, in der dieser Zustand als Text stehen darf.
  Color textColor(Brightness brightness) =>
      FlowStateColor.text(this, brightness);

  /// The glyph that stands for this state.
  HGlyph get glyph => switch (this) {
    HFlowState.held => HGlyph.hourglass,
    HFlowState.allowed => HGlyph.arrowUpRight,
    HFlowState.allowedEdited => HGlyph.arrowUpRightPencil,
    HFlowState.blocked => HGlyph.shieldX,
    HFlowState.timedOut => HGlyph.clockX,
    HFlowState.autoRule => HGlyph.bolt,
    HFlowState.passthroughLlm => HGlyph.chevronsRight,
    HFlowState.error => HGlyph.triangleAlert,
  };

  /// The ARB key of this state's label, camelCase with the feature prefix
  /// `state` (CONVENTIONS 4.11). Resolved by the app, not by this package:
  /// `packages/ui` never hard-wires a user-visible string.
  String get l10nKey => switch (this) {
    HFlowState.held => 'stateHeld',
    HFlowState.allowed => 'stateAllowed',
    HFlowState.allowedEdited => 'stateAllowedEdited',
    HFlowState.blocked => 'stateBlocked',
    HFlowState.timedOut => 'stateTimedOut',
    HFlowState.autoRule => 'stateAutoRule',
    HFlowState.passthroughLlm => 'statePassthroughLlm',
    HFlowState.error => 'stateError',
  };

  /// True when the glyph carries the accent pencil dot of an edited request.
  bool get hasAccentDot => this == HFlowState.allowedEdited;
}
