/// "Humanitl X is available" with its Update button in the status bar
/// (HUM-382).
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../daemon_restart.dart';
import '../update.dart';

/// The quiet notice of a newer release, and the one click that installs it.
///
/// Whether a release is newer, whether its signature holds and how this
/// installation gets it, the daemon says; this widget shows it. It asks the
/// daemon at start and then every [updatePollIntervalProvider]; the daemon
/// alone decides whether that question leaves the machine (`update.check`,
/// once a day at most). Errors of that automatic question stay silent.
class UpdateNotice extends ConsumerStatefulWidget {
  /// Creates the notice for a daemon that speaks contract minor
  /// [daemonMinor].
  const UpdateNotice({required this.daemonMinor, super.key});

  /// `Info.proto_minor` of the daemon; from [updateMinor] on it knows
  /// `Update`.
  final int daemonMinor;

  @override
  ConsumerState<UpdateNotice> createState() => _UpdateNoticeState();
}

class _UpdateNoticeState extends ConsumerState<UpdateNotice> {
  Timer? _timer;

  /// True from the click until the update ended, however it ended.
  bool _busy = false;

  /// The last progress of a running update.
  UpdateProgress? _progress;

  /// The installed version, after the daemon put it in place.
  String? _installed;

  /// The line that says why the update failed, until the next click.
  String? _failure;

  /// True when the update is in place but the daemon restart failed.
  bool _restartFailed = false;

  bool get _supported => widget.daemonMinor >= updateMinor;

  @override
  void initState() {
    super.initState();
    if (!_supported) {
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        unawaited(ref.read(updateStatusProvider.notifier).checkQuietly());
      }
    });
    final Duration? interval = ref.read(updatePollIntervalProvider);
    if (interval != null) {
      _timer = Timer.periodic(interval, (_) {
        if (mounted) {
          unawaited(ref.read(updateStatusProvider.notifier).checkQuietly());
        }
      });
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  Future<void> _update(UpdateStatus status) async {
    setState(() {
      _busy = true;
      _failure = null;
      _restartFailed = false;
      _progress = null;
      _installed = null;
    });
    try {
      final String? failure = appliedByDaemon(status.method)
          ? await _apply(status)
          : await _package(status);
      if (mounted && failure != null) {
        setState(() => _failure = failure);
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  /// The daemon downloads, checks and swaps; then the restart and relaunch.
  Future<String?> _apply(UpdateStatus status) async {
    final DaemonClient client = ref.read(daemonClientProvider);
    final AppLocalizations l10n = context.l10n;
    String? appPath;
    try {
      await for (final UpdateEvent event in client.applyUpdate(
        status.latestVersion,
        appimage: ref.read(updateAppImageProvider),
      )) {
        if (!mounted) {
          return null;
        }
        switch (event) {
          case UpdateEventProgress(:final UpdateProgress progress):
            setState(() => _progress = progress);
          case UpdateEventInstalled(
            :final String version,
            appPath: final String path,
          ):
            setState(() => _installed = version);
            appPath = path;
          case UpdateEventStatus():
            break;
        }
      }
    } on DaemonException catch (error) {
      return DiagnosticL10n.resolve(error.diagnostic, l10n).cause;
    }
    if (appPath == null) {
      return null;
    }
    return _finish(appPath);
  }

  /// The package manager through pkexec, in this session; then the restart
  /// and relaunch of this app's own executable, which the package replaced.
  Future<String?> _package(UpdateStatus status) async {
    setState(
      () => _progress = const UpdateProgress(stage: UpdateStage.installing),
    );
    final String? failure = await runPackageUpgrade(
      status,
      ref.read(updatePackageRunnerProvider),
    );
    if (failure != null || !mounted) {
      return failure;
    }
    setState(() => _installed = status.latestVersion);
    return _finish(ref.read(updateOwnExecutableProvider));
  }

  Future<String?> _finish(String appPath) async {
    final UpdateFinish finish = await finishUpdate(
      appPath: appPath,
      daemonMinor: widget.daemonMinor,
      cli: ref.read(daemonCliProvider),
      restartRunner: ref.read(daemonRestartRunnerProvider),
      relaunch: ref.read(updateRelaunchStarterProvider),
      quit: ref.read(updateQuitProvider),
    );
    return switch (finish) {
      UpdateRelaunched() => null,
      UpdateRestartFailed(:final String detail) => () {
        _restartFailed = true;
        return detail;
      }(),
      UpdateFinishFailed(:final String detail) => detail,
    };
  }

  String _progressLine(AppLocalizations l10n, UpdateProgress? progress) =>
      switch (progress?.stage) {
        UpdateStage.downloading => switch (progress?.percent) {
          final int percent => l10n.shellUpdateDownloading(percent),
          null => l10n.shellUpdateDownloadingUnknown,
        },
        UpdateStage.verifying => l10n.shellUpdateVerifying,
        UpdateStage.installing => l10n.shellUpdateInstalling,
        null => l10n.shellUpdateDownloadingUnknown,
      };

  @override
  Widget build(BuildContext context) {
    if (!_supported) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final TextStyle style = tokens.typography.ui11.tinted(tokens.colors.fg1);
    final UpdateStatus? status = ref.watch(updateStatusProvider);
    final String? installed = _installed;
    final String? failure = _failure;
    if (installed != null && failure != null && _restartFailed) {
      return Text(
        l10n.shellUpdateRestartFailed(installed, failure),
        key: const Key('status-update-restart-failed'),
        overflow: TextOverflow.ellipsis,
        style: tokens.typography.ui11.tinted(tokens.colors.warningText),
      );
    }
    if (installed != null && failure == null) {
      return Text(
        l10n.shellUpdateInstalled(installed),
        key: const Key('status-update-installed'),
        overflow: TextOverflow.ellipsis,
        style: style,
      );
    }
    if (_busy) {
      return Text(
        _progressLine(l10n, _progress),
        key: const Key('status-update-progress'),
        overflow: TextOverflow.ellipsis,
        style: style,
      );
    }
    if (status == null || !status.available) {
      return const SizedBox.shrink();
    }
    final Diagnostic? notice = status.notice;
    final bool clickable =
        status.canApply &&
        (appliedByDaemon(status.method) ||
            packageCommandFor(status.method) != null);
    final String? copy = switch (notice?.fix) {
      FixActionCopyCommand(:final String command) => command,
      _ => null,
    };
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Flexible(
          child: Semantics(
            tooltip: notice == null
                ? null
                : DiagnosticL10n.resolve(notice, l10n).cause,
            child: Text(
              l10n.shellUpdateAvailable(status.latestVersion),
              key: const Key('status-update-available'),
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.ui11.medium.tinted(tokens.colors.fg0),
            ),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        if (clickable)
          HButton(
            key: const Key('status-update-button'),
            variant: HButtonVariant.ghost,
            onPressed: () => unawaited(_update(status)),
            child: Text(l10n.shellUpdateButton, style: style),
          )
        else if (copy != null) ...<Widget>[
          Flexible(
            child: Text(
              copy,
              key: const Key('status-update-command'),
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.mono11.tinted(tokens.colors.fg1),
            ),
          ),
          SizedBox(width: tokens.spacing.x1),
          HButton(
            key: const Key('status-update-copy'),
            variant: HButtonVariant.ghost,
            onPressed: () =>
                unawaited(Clipboard.setData(ClipboardData(text: copy))),
            child: Text(l10n.shellUpdateCopy, style: style),
          ),
        ],
        if (failure != null) ...<Widget>[
          SizedBox(width: tokens.spacing.x2),
          Flexible(
            child: Text(
              l10n.shellUpdateFailed(failure),
              key: const Key('status-update-failed'),
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.ui11.tinted(tokens.colors.warningText),
            ),
          ),
        ],
      ],
    );
  }
}
