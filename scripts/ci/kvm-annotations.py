#!/usr/bin/env python3
"""Schreibt die ganze Ausgabe jedes gefallenen Tests eines Laufs von
`cargo test` als GitHub-Annotation (HUM-445, HUM-470).

    scripts/ci/kvm-annotations.py LOGDATEI TITEL

Das Job-Log verlangt Admin-Rechte; `test-report.sh` nennt je Test nur die
erste Zeile seiner Zusicherung. Die Tests der Kernel-Stufe hängen an ihre
Meldung aber, was den Fehlschlag erklärt: den Bericht des Shims, die Anfragen
an der Tür, den Status des Emulators, die Ausgabe der Helfer und, wo
eingeschaltet, die Konsole des Gasts. Ohne sie war ein Fehlschlag, der nur in
der CI auftrat, nicht zu deuten (HUM-470). Je gefallenem Test eine
Annotation mit den Zeilen verbunden; GitHub zeigt höchstens zehn je Schritt.
Ohne gefallenen Test die letzten 80 Zeilen. Exit immer 0: Das Skript
berichtet, es urteilt nicht.
"""
import re
import sys


def main() -> None:
    path, title = sys.argv[1], sys.argv[2]
    try:
        text = open(path, errors="replace").read()
    except OSError as err:
        print(f"::error title={title}::{path}: {err}")
        return
    blocks = re.findall(
        r"^---- (\S+) stdout ----\n(.*?)(?=^---- |^failures:$)", text, re.S | re.M
    )
    if not blocks:
        blocks = [("tail", "\n".join(text.splitlines()[-80:]))]
    for name, body in blocks[:9]:
        body = body[-30000:].replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        print(f"::error title={title} {name}::{body}")


if __name__ == "__main__":
    main()
