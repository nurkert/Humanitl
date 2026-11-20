/// The category Decision proxy: an intercepting proxy of the person behind
/// Humanitl (HUM-511, ADR-0022).
///
/// Address, CA file and who resolves are keys of `config.toml` and go through
/// `SetConfig` like every other row. The credentials go through `Egress` and
/// never into the file; "Check connection" asks the daemon to open a tunnel
/// through the proxy. Nothing here decides whether a value is good: the
/// daemon checks it and says so at the row (ADR-018).
library;

import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/untranslated_wording.dart';
import '../../../l10n/l10n.dart';
import '../registry/setting_entry.dart';
import '../settings_scope.dart';
import '../widgets/secret_field.dart';
import '../widgets/setting_controls.dart';
import '../widgets/setting_row.dart';

/// Where mitmproxy keeps its CA, the hint of the CA row.
const String mitmproxyCaHint = '~/.mitmproxy/mitmproxy-ca-cert.pem';

/// Asks the desktop for a file. Replaced in tests.
typedef FileChooser = Future<String?> Function({String? dialogTitle});

/// The chooser of the CA row. `file_picker` speaks to the XDG desktop portal
/// on Linux; a widget test has no portal and overrides this provider.
final Provider<FileChooser> caFileChooserProvider = Provider<FileChooser>(
  (Ref ref) =>
      ({String? dialogTitle}) async =>
          (await FilePicker.pickFile(dialogTitle: dialogTitle))?.path,
);

/// What `Egress(status)` answers. Read again whenever the configuration
/// changes (an address, a CA file or the resolver was written, so the hint
/// about a pending restart stands at once), after every change of the
/// credentials and after a check.
final FutureProvider<EgressStatus> egressStatusProvider =
    FutureProvider.autoDispose<EgressStatus>((Ref ref) {
      ref.watch(configProvider);
      return ref.watch(daemonClientProvider).egress(EgressOp.status);
    });

/// `egress.proxy`: the address, written when the field is left or
/// submitted; an empty field removes it.
class EgressProxyRow extends StatefulWidget {
  /// Creates the row for [config].
  const EgressProxyRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  State<EgressProxyRow> createState() => _EgressProxyRowState();
}

class _EgressProxyRowState extends State<EgressProxyRow> {
  late final TextEditingController _text = TextEditingController(
    text: widget.config.egressProxy,
  );

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final String text = _text.text.trim();
    if (text == widget.config.egressProxy) {
      return;
    }
    await SettingsScope.maybeOf(context)
        ?.write(FormKeys.egressProxy, text.isEmpty ? '-' : text);
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SettingRow(
      settingId: FormKeys.egressProxy,
      refusalKey: const Key('settings-egress-proxy-refusal'),
      fill: true,
      child: Focus(
        onFocusChange: (bool focused) {
          if (!focused) {
            unawaited(_save());
          }
        },
        child: HTextField(
          key: const Key('settings-egress-proxy'),
          controller: _text,
          semanticsLabel: l10n.settingsEgressProxyLabel,
          hint: l10n.settingsEgressProxyHint,
          onSubmitted: (String _) => unawaited(_save()),
        ),
      ),
    );
  }
}

/// `egress.proxy_ca`: the PEM file, the chooser, and the way back to none.
class EgressCaRow extends ConsumerWidget {
  /// Creates the row for [config].
  const EgressCaRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  Future<void> _choose(BuildContext context, WidgetRef ref) async {
    final String title = context.l10n.settingsEgressCaChoose;
    final String? path = await ref.read(caFileChooserProvider)(
      dialogTitle: title,
    );
    if (path != null && context.mounted) {
      writeSetting(context, FormKeys.egressProxyCa, path);
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String ca = config.egressProxyCa;
    return SettingRow(
      key: const Key('settings-egress-ca-row'),
      settingId: FormKeys.egressProxyCa,
      refusalKey: const Key('settings-egress-ca-refusal'),
      fill: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: <Widget>[
          HMiddleEllipsis(
            ca.isEmpty ? mitmproxyCaHint : ca,
            textKey: const Key('settings-egress-ca'),
            style: tokens.typography.mono12.tinted(
              ca.isEmpty ? tokens.colors.fg2 : tokens.colors.fg0,
            ),
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x2,
            children: <Widget>[
              if (ca.isNotEmpty)
                HButton(
                  key: const Key('settings-egress-ca-clear'),
                  variant: HButtonVariant.ghost,
                  onPressed: () =>
                      writeSetting(context, FormKeys.egressProxyCa, '-'),
                  child: Text(l10n.settingsEgressCaClear),
                ),
              HButton(
                key: const Key('settings-egress-ca-choose'),
                leading: const HGlyphIcon(HGlyph.folder, size: 14),
                onPressed: () => unawaited(_choose(context, ref)),
                child: Text(l10n.settingsEgressCaChoose),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// `egress.proxy_resolve`: who resolves the target's name.
class EgressResolveRow extends StatelessWidget {
  /// Creates the row for [config].
  const EgressResolveRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SettingRow(
      settingId: FormKeys.egressProxyResolve,
      refusalKey: const Key('settings-egress-resolve-refusal'),
      child: HSegmented<String>(
        key: const Key('settings-egress-resolve'),
        options: <HSegmentOption<String>>[
          HSegmentOption<String>(
            value: EgressResolve.proxy,
            label: l10n.settingsEgressResolveProxy,
          ),
          HSegmentOption<String>(
            value: EgressResolve.humanitl,
            label: l10n.settingsEgressResolveHumanitl,
          ),
        ],
        selected: config.egressProxyResolve,
        onSelect: (String value) {
          if (value != config.egressProxyResolve) {
            writeSetting(context, FormKeys.egressProxyResolve, value);
          }
        },
      ),
    );
  }
}

/// `egress.proxy_auth_ref`: whether credentials are stored, a masked field
/// for new ones, Save and Remove. The credentials go through `Egress` and
/// never come back.
class EgressAuthRow extends ConsumerStatefulWidget {
  /// Creates the row.
  const EgressAuthRow({super.key});

  @override
  ConsumerState<EgressAuthRow> createState() => _EgressAuthRowState();
}

class _EgressAuthRowState extends ConsumerState<EgressAuthRow> {
  final TextEditingController _secret = TextEditingController();
  bool _busy = false;
  Diagnostic? _refusal;

  @override
  void dispose() {
    _secret.dispose();
    super.dispose();
  }

  Future<void> _run(EgressOp op) async {
    final String credentials = _secret.text;
    setState(() {
      _busy = true;
      _refusal = null;
    });
    Diagnostic? refusal;
    try {
      await ref.read(daemonClientProvider).egress(op, credentials: credentials);
      ref.invalidate(egressStatusProvider);
    } on DaemonException catch (error) {
      refusal = error.diagnostic;
    } finally {
      // Ob angenommen oder abgelehnt: Das Feld behält nichts.
      _secret.clear();
    }
    if (mounted) {
      setState(() {
        _busy = false;
        _refusal = refusal;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final EgressStatus? status = ref.watch(egressStatusProvider).value;
    final bool stored = status?.authSet ?? false;
    return SettingRow(
      key: const Key('settings-egress-auth-row'),
      settingId: FormKeys.egressProxyAuthRef,
      refusal: _refusal,
      refusalKey: const Key('settings-egress-auth-refusal'),
      fill: true,
      status: <Widget>[
        Row(
          children: <Widget>[
            HGlyphIcon(
              HGlyph.lock,
              size: 12,
              color: stored ? tokens.stateText.allowed : tokens.colors.fg2,
            ),
            SizedBox(width: tokens.spacing.x1),
            Flexible(
              child: Text(
                stored
                    ? l10n.settingsEgressAuthStored
                    : l10n.settingsEgressAuthNone,
                key: const Key('settings-egress-auth-state'),
                style: tokens.typography.ui12.tinted(
                  stored ? tokens.colors.fg0 : tokens.colors.fg1,
                ),
              ),
            ),
          ],
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: <Widget>[
          SecretField(
            controller: _secret,
            inputKey: const Key('settings-egress-auth-input'),
            semanticsLabel: l10n.settingsEgressAuthLabel,
            hint: l10n.settingsEgressAuthHint,
            enabled: !_busy,
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x2,
            children: <Widget>[
              if (stored)
                HButton(
                  key: const Key('settings-egress-auth-remove'),
                  variant: HButtonVariant.ghost,
                  onPressed: _busy
                      ? null
                      : () => unawaited(_run(EgressOp.removeAuth)),
                  child: Text(l10n.settingsEgressAuthRemove),
                ),
              HButton(
                key: const Key('settings-egress-auth-save'),
                onPressed: _busy
                    ? null
                    : () => unawaited(_run(EgressOp.setAuth)),
                child: Text(l10n.settingsEgressAuthSave),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// "Check connection": a tunnel through `egress.proxy` to the daemon's fixed
/// target and the TLS handshake in it, and the line that says what came
/// back. Also says when the file names another proxy than the running
/// daemon uses.
class EgressCheckRow extends ConsumerStatefulWidget {
  /// Creates the row.
  const EgressCheckRow({super.key});

  @override
  ConsumerState<EgressCheckRow> createState() => _EgressCheckRowState();
}

class _EgressCheckRowState extends ConsumerState<EgressCheckRow> {
  bool _checking = false;
  EgressStatus? _result;
  Diagnostic? _refusal;

  Future<void> _check() async {
    setState(() {
      _checking = true;
      _result = null;
      _refusal = null;
    });
    EgressStatus? result;
    Diagnostic? refusal;
    try {
      result = await ref.read(daemonClientProvider).egress(EgressOp.check);
    } on DaemonException catch (error) {
      refusal = error.diagnostic;
    }
    // Die Prüfung liest die Datei, wie sie jetzt steht; der Stand daneben
    // soll dasselbe sagen.
    ref.invalidate(egressStatusProvider);
    if (mounted) {
      setState(() {
        _checking = false;
        _result = result;
        _refusal = refusal;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final EgressStatus? status = ref.watch(egressStatusProvider).value;
    final EgressStatus? result = _result;
    final Diagnostic? failure = result == null || result.reachable
        ? null
        : (result.diagnostics.isEmpty ? null : result.diagnostics.first);
    final TextStyle quiet = tokens.typography.ui12.tinted(tokens.colors.fg1);
    return SettingRow(
      settingId: SettingIds.egressCheck,
      refusal: _refusal,
      refusalKey: const Key('settings-egress-check-refusal'),
      status: <Widget>[
        if (status != null && status.restartPending)
          Text(
            status.active.isDirect
                ? l10n.settingsEgressRestartPendingDirect
                : l10n.settingsEgressRestartPending(status.active.proxy),
            key: const Key('settings-egress-restart'),
            style: tokens.typography.ui12.tinted(tokens.colors.warningText),
          ),
        if (_checking) Text(l10n.settingsEgressChecking, style: quiet),
        if (result != null && result.reachable)
          Text(
            l10n.settingsEgressCheckOk(result.checkTarget),
            key: const Key('settings-egress-check-result'),
            style: tokens.typography.ui12.tinted(tokens.stateText.allowed),
          ),
        if (result != null && !result.reachable)
          if (failure == null)
            Text(
              l10n.settingsEgressCheckFailedPlain,
              key: const Key('settings-egress-check-result'),
              style: tokens.typography.ui12.tinted(tokens.stateText.error),
            )
          else
            DiagnosticCause(
              diagnostic: failure,
              textKey: const Key('settings-egress-check-result'),
              style: tokens.typography.ui12.tinted(tokens.stateText.error),
            ),
      ],
      child: HButton(
        key: const Key('settings-egress-check-now'),
        onPressed: _checking ? null : () => unawaited(_check()),
        child: Text(l10n.settingsEgressCheckNow),
      ),
    );
  }
}
