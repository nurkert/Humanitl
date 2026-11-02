/// The category Network: whether the daemon trusts the certificate store of
/// the system (HUM-257, HUM-404).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../widgets/setting_controls.dart';
import '../widgets/setting_row.dart';

/// `tls.trust_system_store`. Vorgabe an; was aus bedeutet, steht hinter dem
/// Info-Symbol.
class TrustStoreRow extends StatelessWidget {
  /// Creates the row for [config].
  const TrustStoreRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  Widget build(BuildContext context) => SettingRow(
    settingId: FormKeys.tlsTrustSystemStore,
    refusalKey: const Key('settings-trust-system-store-refusal'),
    child: OnOffControl(
      key: const Key('settings-trust-system-store'),
      value: config.tlsTrustSystemStore,
      onChanged: (bool on) =>
          writeSetting(context, FormKeys.tlsTrustSystemStore, '$on'),
    ),
  );
}
