// Die Goldens aus HUM-054, je in vier Varianten (`golden_variants.dart`).
//
// Die Liste ist die eine Stelle, an der ein Name dieses Satzes steht.
// `goldenVariants` nimmt nur Namen von hier an, und
// `golden_variants_complete_test.dart` verlangt zu jedem Namen genau die vier
// Bilddateien. Ein neuer Name kommt also hierher und in seinen Test, und die
// Bilder entstehen mit `make goldens-update`.
//
// Gegenüber der Liste in `backlog/sprint-4.md` fehlen zwei Namen, deren
// Gegenstand es im MVP nicht gibt, und drei sind umbenannt:
//
//  - `action_bar_edited` fehlt: „Editierte Version senden" steht nicht in der
//    Aktionsleiste, sondern im Editor (HUM-047), und `editor_replaced_glow`
//    zeigt es.
//  - `settings_group_with_expert`, `settings_search_results` und
//    `settings_env_overridden` gehören zum Formular aus dem Schema, das die
//    zweite Umfangsentscheidung vom 2026-09-18 hinter das MVP verschoben hat.
//    An ihrer Stelle stehen die drei Zustände des handgebauten Formulars.
//
// Die Namen mit `states_` gehören zu HUM-058 und stehen in derselben Form;
// welche Zustände dort fehlen, weil HUM-054 sie schon festhält, sagt
// `states_golden_test.dart`.
library;

/// Jeder Golden-Name dieses Satzes, nach Widget-Familie.
const List<String> goldenNames = <String>[
  // queue_row_golden_test.dart
  'queue_row_held',
  'queue_row_held_findings',
  'queue_row_selected',
  'queue_row_hover',
  'queue_group_header',
  // request_card_golden_test.dart
  'request_card_json',
  'request_card_form',
  'request_card_binary',
  'request_card_large',
  'request_card_timed_out',
  // action_bar_golden_test.dart
  'action_bar_default',
  'action_bar_findings',
  'action_bar_hard_block',
  'action_bar_remember_open',
  // findings_pause_golden_test.dart
  'findings_pause',
  // editor_golden_test.dart
  'editor_split_findings',
  'editor_replaced_glow',
  'editor_mapping_open',
  'editor_json_invalid',
  // domain_panel_golden_test.dart
  'domain_known',
  'domain_unknown',
  // history_golden_test.dart
  'history_table',
  'history_detail_edited',
  // rules_golden_test.dart
  'rules_list',
  // isolation_golden_test.dart
  'isolation_panel_ok',
  'isolation_panel_failed',
  'isolation_ring_states',
  // audit_golden_test.dart
  'audit_status_ok',
  'audit_status_broken',
  // settings_golden_test.dart
  'settings_form',
  'settings_probe_result',
  'settings_refusal',
  // setup_golden_test.dart
  'setup_checklist',
  'setup_firstrun',
  'setup_partial',
  // ui_gallery_golden_test.dart
  'tokens_palette',
  // states_golden_test.dart (HUM-058)
  'states_daemon_missing',
  'states_incompatible',
  'states_sandbox_failed',
  'states_tls_rejected',
  'states_websocket_card',
  // icon_rail_golden_test.dart (HUM-254)
  'icon_rail_collapsed',
  'icon_rail_expanded',
  // states_golden_test.dart (HUM-250)
  'states_recorder_failure',
  // settings_entry_golden_test.dart (HUM-275)
  'settings_entry',
  // llm_servers_golden_test.dart (HUM-289)
  'settings_servers',
  'settings_server_remove',
  'status_llm_switch',
];
