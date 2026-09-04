/// What the daemon logged about this sandbox (HUM-040).
///
/// The agent's own output is something else and arrives with the terminal;
/// this is the daemon speaking about the sandbox -- it started, it stopped,
/// it refused. The list follows the newest line, and stops following as soon
/// as somebody scrolls: nothing moves under the eye that is reading
/// (`docs/UX.md` 2.8).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/sandbox_status_provider.dart';

/// The log tab.
class LogTab extends ConsumerStatefulWidget {
  /// Creates the log tab.
  const LogTab({super.key});

  @override
  ConsumerState<LogTab> createState() => _LogTabState();
}

class _LogTabState extends ConsumerState<LogTab> {
  final ScrollController _scroll = ScrollController();
  bool _follow = true;

  @override
  void initState() {
    super.initState();
    _scroll.addListener(_watchScroll);
  }

  @override
  void dispose() {
    _scroll
      ..removeListener(_watchScroll)
      ..dispose();
    super.dispose();
  }

  /// Following is off as soon as the view is not at the bottom any more, and
  /// back on when it returns there. The control follows the hand; the hand
  /// does not have to find a control.
  void _watchScroll() {
    if (!_scroll.hasClients) {
      return;
    }
    final bool atEnd =
        _scroll.offset >= _scroll.position.maxScrollExtent - HSpace.x1;
    if (atEnd != _follow) {
      setState(() => _follow = atEnd);
    }
  }

  void _stickToEnd() {
    if (!_follow || !_scroll.hasClients) {
      return;
    }
    _scroll.jumpTo(_scroll.position.maxScrollExtent);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<SandboxLogLine> lines = ref.watch(sandboxLogProvider);
    if (lines.isEmpty) {
      return Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Text(
          l10n.sandboxLogEmpty,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      );
    }
    // A new line is put at the bottom after the frame that added it; jumping
    // during build would scroll a list that is not laid out yet.
    WidgetsBinding.instance.addPostFrameCallback((Duration _) => _stickToEnd());
    return ListView.builder(
      controller: _scroll,
      key: const PageStorageKey<String>('sandbox-log'),
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x3,
        vertical: tokens.spacing.x1,
      ),
      // The lines are one line each and never change height, so the list
      // knows its extent and scrolling costs the same at any length
      // (`docs/UX.md` 7).
      itemExtent: HSize.rowBody,
      itemCount: lines.length,
      itemBuilder: (BuildContext context, int index) {
        final SandboxLogLine line = lines[index];
        return Align(
          alignment: Alignment.centerLeft,
          child: Text.rich(
            TextSpan(
              children: <InlineSpan>[
                TextSpan(
                  text: '${_clock(line.at)}  ',
                  style: tokens.typography.mono11.tinted(tokens.colors.fg2),
                ),
                TextSpan(
                  text: line.text,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                ),
              ],
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        );
      },
    );
  }

  /// `hh:mm:ss` of [at], the resolution the log is written in.
  String _clock(DateTime at) {
    String two(int n) => n.toString().padLeft(2, '0');
    return '${two(at.hour)}:${two(at.minute)}:${two(at.second)}';
  }
}
