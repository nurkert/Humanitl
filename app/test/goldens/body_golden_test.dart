// Goldens der vier Rumpf-Ansichten (HUM-030): Baum mit Funden, Formular,
// Rohtext mit Funden und Hex, je dunkel und hell.
// Erneuern mit `flutter test --update-goldens test/goldens`.

import 'dart:typed_data';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/body/body_kind.dart';
import 'package:humanitl/features/intercept/body/body_parser.dart';
import 'package:humanitl/features/intercept/body/form_view.dart';
import 'package:humanitl/features/intercept/body/hex_view.dart';
import 'package:humanitl/features/intercept/body/json_tree_view.dart';
import 'package:humanitl/features/intercept/body/raw_view.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../features/intercept/body/harness.dart';

/// Ein JSON-Rumpf mit zwei Funden: ein Schlüssel und eine Adresse.
ParsedBody goldenJson() {
  const String source =
      '{"repo":"acme/api","token":"ghp_A1B2C3D4E5","contact":'
      '{"mail":"a@b.de","name":"Müller"},"retries":3,"dry":false}';
  return parseBody(bytesOf(source), BodyKind.json, <Finding>[
    bodyFinding(
      start: source.indexOf('ghp_A1B2C3D4E5'),
      end: source.indexOf('ghp_A1B2C3D4E5') + 14,
      kind: 'api_key:github',
      tier: FindingTier.checksum,
    ),
    bodyFinding(
      start: source.indexOf('a@b.de'),
      end: source.indexOf('a@b.de') + 6,
    ),
  ]);
}

/// Ein Formularrumpf mit Prozentkodierung und einem Fund.
ParsedBody goldenForm() {
  const String source =
      'grant_type=refresh_token&scope=read+write&mail=a%40b.de&state=xyz';
  return parseBody(bytesOf(source), BodyKind.form, <Finding>[
    bodyFinding(
      start: source.indexOf('a%40b.de'),
      end: source.indexOf('a%40b.de') + 8,
    ),
  ]);
}

/// Bytes, die kein Text sind.
Uint8List goldenBytes() => Uint8List.fromList(<int>[
  0x89,
  0x50,
  0x4E,
  0x47,
  0x0D,
  0x0A,
  0x1A,
  0x0A,
  ...List<int>.generate(120, (int i) => (i * 37) % 256),
]);

/// Ein Widget im Theme [mode], mit Lokalisierung und Overlay.
Widget piece({required HThemeMode mode, required Widget child}) {
  final HTokens tokens = mode.resolve(Brightness.dark);
  return WidgetsApp(
    color: tokens.colors.bg0,
    debugShowCheckedModeBanner: false,
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    builder: (BuildContext context, Widget? _) => MediaQuery(
      data: const MediaQueryData(),
      child: HTheme(
        tokens: tokens,
        child: ColoredBox(
          color: tokens.colors.bg1,
          child: Overlay(
            initialEntries: <OverlayEntry>[
              OverlayEntry(builder: (BuildContext context) => child),
            ],
          ),
        ),
      ),
    ),
  );
}

void main() {
  const BoxConstraints box = BoxConstraints.tightFor(width: 620, height: 240);

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    goldenTest(
      'body_json_tree_findings_$name',
      fileName: 'body_json_tree_findings_$name',
      constraints: box,
      builder: () {
        final ParsedBody parsed = goldenJson();
        return piece(
          mode: mode,
          child: JsonTreeView(
            document: parsed.json!,
            findings: parsed.findings,
          ),
        );
      },
    );

    goldenTest(
      'body_form_$name',
      fileName: 'body_form_$name',
      constraints: box,
      builder: () {
        final ParsedBody parsed = goldenForm();
        return piece(
          mode: mode,
          child: FormView(pairs: parsed.form!, findings: parsed.findings),
        );
      },
    );

    goldenTest(
      'body_raw_findings_$name',
      fileName: 'body_raw_findings_$name',
      constraints: box,
      builder: () {
        final ParsedBody parsed = goldenJson();
        return piece(
          mode: mode,
          child: RawBodyView(text: parsed.text!, findings: parsed.findings),
        );
      },
    );

    goldenTest(
      'body_hex_$name',
      fileName: 'body_hex_$name',
      constraints: box,
      builder: () => piece(
        mode: mode,
        child: HexView(
          bytes: goldenBytes(),
          findings: parseBody(goldenBytes(), BodyKind.binary, <Finding>[
            bodyFinding(
              start: 16,
              end: 24,
              kind: 'api_key:aws',
              tier: FindingTier.checksum,
            ),
          ]).findings,
          limit: bodyHexLimit,
        ),
      ),
    );
  }
}
