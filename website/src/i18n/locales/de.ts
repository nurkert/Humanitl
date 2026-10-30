// Deutsche Texte, die Quelle der Form: en.ts ist gegen diesen Typ geprüft.
const de = {
  meta: {
    title: 'Humanitl · Du entscheidest, was rausgeht',
    description:
      'Humanitl sperrt deinen KI-Coding-Agenten in eine Sandbox ohne Netz. Was keine deiner Regeln erlaubt, hält es an, bis du entscheidest. Lokal, quelloffen, für Linux und macOS.',
  },
  nav: {
    how: 'So funktioniert es',
    security: 'Sicherheit',
    download: 'Download',
    menu: 'Menü',
    home: 'Humanitl, zur Startseite',
    language: 'Sprache',
  },
  hero: {
    title: ['Du entscheidest,', 'was rausgeht.'],
    lede: 'Dein KI-Agent arbeitet in einer Sandbox ohne Netz. Was keine deiner Regeln erlaubt, hält Humanitl an, bis du entscheidest.',
    cta: 'Download',
    ctaFor: 'für macOS und Linux',
    source: 'Quellcode auf GitHub',
    scroll: 'Weiter scrollen',
  },
  stage: {
    title: 'Module im Fokus',
    previous: 'Vorheriges Modul',
    next: 'Nächstes Modul',
    of: 'von',
    modules: [
      {
        id: 'intercept',
        name: 'Intercept',
        title: 'Was keine Regel erlaubt, hält an.',
        text: 'Kopfzeilen, Parameter, Body: Du siehst, was dein Agent senden will, und gibst es frei, änderst es oder blockierst es.',
        alt: 'Humanitl, Bereich Intercept: 15 angehaltene Anfragen an registry.npmjs.org, die erste mit allen Kopfzeilen, darunter Allow, Edit + Allow und Block.',
      },
      {
        id: 'notice',
        name: 'Begründung',
        title: 'Erklärt, nicht versteckt.',
        text: 'Humanitl sagt, warum eine Anfrage wartet: Keine Regel passt, also fragt es dich. Mit dem Pfeil wird aus einer Entscheidung eine Regel.',
        alt: 'Humanitl erklärt eine wartende Anfrage: angehalten, weil keine Regel passt, Standard ist fragen.',
      },
      {
        id: 'history',
        name: 'Verlauf',
        title: 'Nachlesen, was rausging.',
        text: 'Die Anfragen landen im Verlauf, filterbar nach Host, Entscheidung und Funden. Fällt die Aufzeichnung aus, läuft der Verkehr weiter, und die App sagt es an jeder Karte.',
        alt: 'Humanitl, Bereich History: eine Tabelle der Anfragen mit Zeit, Methode, Host, Pfad und Entscheidung.',
      },
      {
        id: 'rules',
        name: 'Regeln',
        title: 'Regeln, die du lesen kannst.',
        text: 'Erlaube eine Domain für diese Sitzung oder für immer, sperre eine andere. Was keine Regel trifft, wartet auf dich.',
        alt: 'Humanitl, Bereich Rules: mitgelieferte Regeln für models.dev und ein lokales Modell, darunter die Vorgabe fragen.',
      },
      {
        id: 'sandbox',
        name: 'Sandbox',
        title: 'Eine Sandbox ohne Netz.',
        text: 'Der Agent sieht sein Projekt und sonst nichts von deinem Rechner. Sein einziger Weg nach draußen führt durch Humanitl.',
        alt: 'Humanitl, Bereich Sandbox: das Terminal des Agenten und die Tabelle der Pfade, die er sieht.',
      },
      {
        id: 'palette',
        name: 'Befehle',
        title: 'Die Tastatur zuerst.',
        text: 'Die Befehlspalette erreicht jeden Bereich. Über eine Anfrage entscheiden Tasten: Enter gibt frei, E öffnet sie zum Ändern, B blockiert.',
        alt: 'Humanitl mit geöffneter Befehlspalette über dem Intercept-Bereich.',
      },
      {
        id: 'settings',
        name: 'Modell',
        title: 'Dein Modell, dein Schlüssel.',
        text: 'Endpunkt und Modell trägst du selbst ein. Der Zugangsschlüssel liegt unter Linux im Schlüsselbund des Systems, sonst in einer Datei, die nur dein Konto lesen kann. In der Konfiguration steht er nie.',
        alt: 'Humanitl, Einstellungen: Endpunkt, Modell und Zugangsschlüssel des Sprachmodells.',
      },
    ],
  },
  guarantees: {
    title: 'Physik statt Vertrauen.',
    lede: 'Drei Garantien, jede mit einem Befehl prüfbar. Unter Linux mit bubblewrap und seccomp, unter macOS mit Seatbelt. Offen und sichtbar bleiben zwei Kanäle: der Projektordner und die erklärte Verbindung zum Sprachmodell.',
    items: [
      {
        title: 'Kein Weg nach draußen.',
        text: 'Unter Linux hat die Sandbox kein Netzwerk-Interface. Unter macOS endet jede Verbindung außer der Tür mit EPERM.',
      },
      {
        title: 'Genau eine Tür.',
        text: 'Ein einziger Socket führt aus der Sandbox, und er führt zu Humanitl. Dort entscheidest du.',
      },
      {
        title: 'Der Kernel hält dicht.',
        text: 'seccomp unter Linux, ein Seatbelt-Profil unter macOS: Der Agent kann sich keine neue Tür öffnen.',
      },
    ],
    check: 'Alle drei auf einmal prüfen:',
    more: 'Die Sicherheitsargumentation',
  },
  closing: {
    title: 'Behalte die Hand am Netz.',
    text: 'Kostenlos und quelloffen. Kein Konto, keine Cloud: Humanitl läuft auf deinem Rechner.',
    cta: 'Humanitl herunterladen',
  },
  download: {
    title: 'Download',
    metaDescription: 'Humanitl für macOS und Linux herunterladen: Disk-Image, Zip, Debian-Paket und Archiv, direkt aus den GitHub-Releases.',
    lede: 'Wähle dein System. Jede Datei kommt unverändert aus den GitHub-Releases, gebaut von GitHub Actions aus dem öffentlichen Quellcode.',
    version: 'Version',
    published: 'veröffentlicht am',
    whatsNew: 'Was ist neu?',
    download: 'Download',
    downloads: 'Downloads',
    downloadsNote: 'So oft hat GitHub die Dateien aller Versionen ausgeliefert.',
    systems: 'System',
    recommended: 'Empfohlen',
    get: 'Herunterladen',
    checksums: 'SHA-256-Prüfsummen',
    none: 'Die Releases sind gerade nicht erreichbar. Alle Dateien liegen auf GitHub.',
    toGitHub: 'Zu den Releases auf GitHub',
    older: 'Frühere Versionen',
    steps: 'So geht es weiter',
    kinds: {
      dmg: 'Disk-Image',
      zip: 'Zip-Archiv',
      deb: 'Debian-Paket',
      tar: 'Archiv ohne App',
    },
    platforms: {
      macos: {
        name: 'macOS',
        req: 'Apple Silicon und Intel. Getestet mit macOS 15 und 27.',
        steps: [
          'Humanitl.app in den Ordner Programme ziehen.',
          'Die App ist nur ad hoc signiert, macOS verweigert deshalb den ersten Start. Einmal erlauben unter Systemeinstellungen › Datenschutz & Sicherheit › „Trotzdem öffnen“.',
          'Humanitl starten. Unter „Humanitl einrichten“ richtet „Installieren und starten“ den Hintergrunddienst als launchd-Agent ein.',
        ],
        command:
          'xattr -dr com.apple.quarantine /Applications/Humanitl.app\n/Applications/Humanitl.app/Contents/MacOS/bin/humanitl daemon install\n/Applications/Humanitl.app/Contents/MacOS/bin/humanitl sandbox check',
        commandNote: 'Oder alles im Terminal: den ersten Start erlauben, Dienst einrichten, Sandbox prüfen.',
      },
      linux: {
        name: 'Linux',
        req: 'Debian 13 oder Ubuntu 24.04 und neuer, x86_64.',
        steps: [
          'Das Debian-Paket installieren.',
          'Den Hintergrunddienst einrichten.',
          'Die Sandbox prüfen: Alle drei Garantien müssen halten.',
        ],
        command: 'sudo apt install ./humanitl_@VERSION@_amd64.deb\nhumanitl daemon install\nhumanitl sandbox check',
        commandNote: 'Im Terminal, im Ordner der Datei:',
      },
    },
  },
  legal: {
    imprint: 'Impressum',
    privacy: 'Datenschutz',
    imprintTitle: 'Impressum',
    privacyTitle: 'Datenschutzerklärung',
    forkNote:
      'Diese Seite wurde aus einer Kopie (Fork) des Humanitl-Projekts gebaut. Wer sie öffentlich betreibt, braucht in der Regel ein eigenes Impressum und hinterlegt dafür in seinem Repository die Variable IMPRINT_URL.',
  },
  privacy: {
    updated: 'Stand: 30. Oktober 2026',
    sections: [
      {
        title: 'Kurz gesagt',
        html: 'Diese Website setzt <strong>keine Cookies</strong>, nutzt <strong>kein Tracking</strong>, keine Analyse-Werkzeuge und lädt <strong>keine Inhalte von Dritten</strong> nach. Schriften, Bilder und Skripte kommen ausschließlich von dieser Domain. Es gibt keine Formulare und kein Kundenkonto. Die Download-Zahlen werden beim Bauen der Seite bei GitHub abgefragt, nicht von deinem Browser.',
      },
      {
        title: 'Verantwortlicher',
        html: 'Verantwortlich für diese Website ist die im <a href="/impressum/">Impressum</a> genannte Person.',
      },
      {
        title: 'Hosting durch GitHub Pages',
        html: 'Die Website wird über GitHub Pages bereitgestellt, einen Dienst der GitHub B.V., Prins Bernhardplein 200, 1097 JB Amsterdam, Niederlande, bzw. der GitHub Inc., USA. Beim Aufruf verarbeitet GitHub technisch notwendige Daten wie IP-Adresse, Datum und Uhrzeit, die aufgerufene Seite sowie Angaben zu Browser und Betriebssystem, um die Seite auszuliefern und vor Missbrauch zu schützen. Rechtsgrundlage ist unser berechtigtes Interesse an einer sicheren und zuverlässigen Bereitstellung (Art. 6 Abs. 1 lit. f DSGVO). Eine Übermittlung in die USA ist möglich; GitHub ist unter dem EU-U.S. Data Privacy Framework zertifiziert. Details: <a href="https://docs.github.com/de/site-policy/privacy-policies/github-general-privacy-statement">Datenschutzerklärung von GitHub</a>.',
      },
      {
        title: 'Downloads und externe Links',
        html: 'Die Download-Knöpfe führen zu den Dateien auf GitHub. Erst wenn du einem solchen Link folgst, gelten die Datenschutzbestimmungen von GitHub.',
      },
      {
        title: 'Die App',
        html: 'Humanitl läuft lokal auf deinem Rechner und sendet <strong>keine Daten an uns</strong>: kein Konto, keine Telemetrie, keine Absturzberichte. Was die App ins Netz lässt, entscheidest du Anfrage für Anfrage.',
      },
      {
        title: 'Deine Rechte',
        html: 'Du hast das Recht auf Auskunft, Berichtigung, Löschung und Einschränkung der Verarbeitung, auf Datenübertragbarkeit sowie auf Widerspruch gegen eine Verarbeitung auf Grundlage berechtigter Interessen (Art. 15 bis 21 DSGVO). Außerdem kannst du dich bei einer Datenschutz-Aufsichtsbehörde beschweren (Art. 77 DSGVO). Da wir selbst keine Daten über Besucher speichern, richten sich Anfragen zu den Server-Protokollen in der Regel an GitHub.',
      },
    ],
  },
  footer: {
    line: 'Open Source unter der GNU GPL 3.0.',
    github: 'GitHub',
  },
};

export type Copy = typeof de;
export default de;
