// Das Modell hinter den vier Ansichten, an den Rümpfen geprüft, die eine
// Anzeige anhalten oder täuschen sollen: Millionen Knoten, tausend Ebenen,
// eine einzige Zeile von Megabytes, doppelte Schlüssel, JSON, das keines ist.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/body/body_kind.dart';
import 'package:humanitl/features/intercept/body/body_parser.dart';
import 'package:humanitl/features/intercept/body/body_span.dart';
import 'package:humanitl/features/intercept/body/form_view.dart';
import 'package:humanitl/features/intercept/body/json_tree_view.dart';

import 'harness.dart';

void main() {
  test('raw_findings_decoration_positions', () {
    const String source = '{"email":"a@b.de"}';
    final int start = source.indexOf('a@b.de');
    final ParsedBody parsed = parseBody(
      bytesOf(source),
      BodyKind.json,
      <Finding>[bodyFinding(start: start, end: start + 6)],
    );
    final BodyFinding finding = parsed.findings.single;
    expect(
      parsed.text!.text.substring(finding.charStart, finding.charEnd),
      'a@b.de',
    );
  });

  test('form_decoded_plus', () {
    final ParsedBody parsed = parseBody(
      bytesOf('a=x+y&b=%40home'),
      BodyKind.form,
      const <Finding>[],
    );
    expect(parsed.form!.map((FormPair p) => p.name).toList(), <String>[
      'a',
      'b',
    ]);
    expect(parsed.form!.map((FormPair p) => p.value).toList(), <String>[
      'x y',
      '@home',
    ]);
  });

  test('form_finding_survives_percent_decoding', () {
    // `%40` sind drei Bytes und ein Zeichen; ohne die Tabelle säße die
    // Markierung um zwei Stellen daneben.
    const String source = 'to=a%40b.de';
    final int start = source.indexOf('a%40b.de');
    final ParsedBody parsed = parseBody(
      bytesOf(source),
      BodyKind.form,
      <Finding>[bodyFinding(start: start, end: source.length)],
    );
    final FormPair pair = parsed.form!.single;
    expect(pair.value, 'a@b.de');
    expect(pair.byteOfChar.first, start);
    // Ein Eintrag mehr als Zeichen: der Abschluss gibt dem letzten Zeichen
    // seinen Bereich.
    expect(pair.byteOfChar.length, pair.value.length + 1);
    expect(pair.byteOfChar.last, source.length);
  });

  test('form_finding_on_the_hex_digits_marks_the_whole_character', () {
    // `%40` sind drei Bytes und ein Zeichen. Ein Fund, der nur die beiden
    // Hex-Ziffern trifft, gehört trotzdem auf dieses eine Zeichen; wer nur das
    // erste Byte vergleicht, markiert hier gar nichts.
    const String source = 'to=a%40b.de';
    final int percent = source.indexOf('%40');
    final ParsedBody parsed = parseBody(
      bytesOf(source),
      BodyKind.form,
      <Finding>[bodyFinding(start: percent + 1, end: percent + 3)],
    );
    final List<BodyFinding> marks = formMarks(
      parsed.form!.single.byteOfChar,
      parsed.findings,
    );
    expect(marks, hasLength(1));
    expect(
      parsed.form!.single.value.substring(
        marks.single.charStart,
        marks.single.charEnd,
      ),
      '@',
    );
  });

  test('form_marks_split_into_runs_instead_of_bridging', () {
    // Zwei getrennte Treffer in einem Wert dürfen nicht zu einem Bereich
    // werden: die harmlosen Zeichen dazwischen trügen sonst denselben
    // Unterstrich.
    const String source = 'v=aXbXc';
    final ParsedBody parsed = parseBody(
      bytesOf(source),
      BodyKind.form,
      const <Finding>[],
    );
    final FormPair pair = parsed.form!.single;
    const BodyFinding first = BodyFinding(
      index: 0,
      kind: 'email',
      tier: FindingTier.regex,
      tone: BodyFindingTone.personal,
      byteStart: 2,
      byteEnd: 3,
      charStart: 0,
      charEnd: 0,
      needle: 'a',
    );
    const BodyFinding second = BodyFinding(
      index: 0,
      kind: 'email',
      tier: FindingTier.regex,
      tone: BodyFindingTone.personal,
      byteStart: 6,
      byteEnd: 7,
      charStart: 0,
      charEnd: 0,
      needle: 'c',
    );
    final List<BodyFinding> marks = formMarks(pair.byteOfChar, <BodyFinding>[
      first,
      second,
    ]);
    expect(marks, hasLength(2));
    expect(marks.first.charStart, 0);
    expect(marks.first.charEnd, 1);
    expect(marks.last.charStart, 4);
    expect(marks.last.charEnd, 5);
  });

  test('json_tree_lazy_children', () {
    // Zehntausend Knoten. Offen ist die Wurzel, sichtbar sind Wurzel und ihre
    // Kinder -- nicht der ganze Baum.
    final Map<String, Object?> source = <String, Object?>{
      for (int i = 0; i < 100; i++)
        'group$i': <String, Object?>{for (int j = 0; j < 99; j++) 'leaf$j': j},
    };
    final String text = jsonEncode(source);
    final Stopwatch watch = Stopwatch()..start();
    final ParsedBody parsed = parseBody(
      bytesOf(text),
      BodyKind.json,
      const <Finding>[],
    );
    watch.stop();
    final JsonDocument document = parsed.json!;
    expect(document.nodes.length, 1 + 100 + 100 * 99);
    expect(watch.elapsedMilliseconds, lessThan(1000));

    final Set<int> expanded = <int>{0};
    final List<int> root = visibleJsonNodes(document, expanded);
    expect(root.length, 101);

    final int group = root[1];
    expanded.add(group);
    final List<int> opened = visibleJsonNodes(document, expanded);
    expect(opened.length, 101 + document.nodes[group].childCount);
    expect(document.nodes[group].childCount, 99);
  });

  test('json_deep_nesting_does_not_stall_the_tree', () {
    // Fünftausend Ebenen. Weder das Modell noch die sichtbare Liste darf
    // daran hängen bleiben, und tiefer als `jsonMaxDepth` wird nicht gebaut.
    const int depth = 5000;
    final String text = '${'[' * depth}1${']' * depth}';
    final ParsedBody parsed = parseBody(
      bytesOf(text),
      BodyKind.json,
      const <Finding>[],
    );
    final JsonDocument document = parsed.json!;
    expect(document.depthCapped, isTrue);
    expect(document.nodes.length, lessThanOrEqualTo(jsonMaxDepth + 1));
    final Set<int> everything = <int>{
      for (int i = 0; i < document.nodes.length; i++) i,
    };
    expect(
      visibleJsonNodes(document, everything).length,
      document.nodes.length,
    );
  });

  test('json_wide_object_is_capped_and_says_so', () {
    final String text = jsonEncode(<String, Object?>{
      for (int i = 0; i < jsonMaxNodes + 10; i++) 'k$i': i,
    });
    final ParsedBody parsed = parseBody(
      bytesOf(text),
      BodyKind.json,
      const <Finding>[],
    );
    expect(parsed.json!.capped, isTrue);
    expect(parsed.json!.nodes.length, lessThanOrEqualTo(jsonMaxNodes));
  });

  test('json_duplicate_keys_are_reported', () {
    final ParsedBody parsed = parseBody(
      bytesOf('{"amount": 1, "amount": 999}'),
      BodyKind.json,
      const <Finding>[],
    );
    expect(parsed.json!.duplicateKeys, isTrue);
    expect(parsed.json!.nodes.length, 2);
  });

  test('json_duplicate_key_scan_ignores_strings_and_nesting', () {
    expect(hasDuplicateJsonKeys('{"a": "a", "b": {"a": 1}}'), isFalse);
    expect(hasDuplicateJsonKeys(r'{"a\"b": 1, "a\"b": 2}'), isTrue);
    expect(hasDuplicateJsonKeys('["a", "a"]'), isFalse);
  });

  test('json_that_is_not_json_is_not_an_empty_body', () {
    // Als JSON angekündigt, aber Text. Der Baum fehlt, der Text ist da, und
    // das Problem hat einen Namen -- "leer" wäre hier eine Lüge.
    final ParsedBody parsed = parseBody(
      bytesOf('this is not json at all'),
      BodyKind.json,
      const <Finding>[],
    );
    expect(parsed.json, isNull);
    expect(parsed.problem, BodyProblem.notJson);
    expect(parsed.text!.text, 'this is not json at all');
  });

  test('one_very_long_line_becomes_rows_without_shifting_offsets', () {
    // Acht Mebibyte in einer einzigen Zeile. Sie wird an bekannten Spalten
    // geschnitten; jede Zeile trägt ihren eigenen Anfang, also bleibt jede
    // Markierung an ihrem Zeichen.
    final String long = 'x' * (bodyRowChars * 3 + 7);
    final BodyText text = buildBodyText(bytesOf(long));
    expect(text.rows.length, 4);
    expect(text.rows.first.line, 1);
    expect(text.rows.last.line, 1);
    expect(text.rows.last.continued, isTrue);
    expect(text.rows.last.charEnd, long.length);
    for (final BodyRow row in text.rows) {
      expect(text.slice(row).length, row.length);
    }
  });

  test('rows_stop_at_the_cap_and_the_model_says_so', () {
    final String many = List<String>.filled(bodyMaxRows + 50, 'a').join('\n');
    final BodyText text = buildBodyText(bytesOf(many));
    expect(text.rowsCapped, isTrue);
    expect(text.rows.length, bodyMaxRows);
  });

  test('an_aborted_load_is_not_an_empty_body', () {
    final ParsedBody parsed = parseBody(
      bytesOf('{"a"'),
      BodyKind.json,
      const <Finding>[],
      problem: BodyProblem.incomplete,
    );
    expect(parsed.kind, isNot(BodyKind.empty));
    expect(parsed.problem, BodyProblem.incomplete);
  });

  test('binary_keeps_no_text_and_still_parses', () {
    final Uint8List noise = Uint8List.fromList(
      List<int>.generate(300, (int i) => (i * 37) % 256),
    );
    final ParsedBody parsed = parseBody(
      noise,
      BodyKind.binary,
      const <Finding>[],
    );
    expect(parsed.text, isNull);
    expect(parsed.json, isNull);
  });

  test('the_isolate_path_gives_the_same_answer', () async {
    final String text = jsonEncode(<String, Object?>{
      for (int i = 0; i < 5000; i++) 'k$i': 'value $i',
    });
    expect(text.length, greaterThan(bodyIsolateThreshold));
    final ParsedBody parsed = await parseBodyAsync(
      bytesOf(text),
      BodyKind.json,
      const <Finding>[],
    );
    expect(parsed.json!.nodes.length, 5001);
  });
}
