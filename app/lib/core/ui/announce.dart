/// Saying something to whoever listens with their ears (`docs/UX.md` 6).
///
/// Politely, always: what interrupts is reserved for what happens *to* the
/// person -- the timeout warning and the timeout itself. Everything they
/// caused themselves waits its turn.
library;

import 'dart:async';

import 'package:flutter/semantics.dart';
import 'package:flutter/widgets.dart';

/// Says [message] politely, or does nothing where announcements do not exist.
void announcePolitely(BuildContext context, String message) {
  if (message.isEmpty || !MediaQuery.supportsAnnounceOf(context)) {
    return;
  }
  unawaited(
    SemanticsService.sendAnnouncement(
      View.of(context),
      message,
      Directionality.of(context),
    ),
  );
}
