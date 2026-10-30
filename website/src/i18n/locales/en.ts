import type { Copy } from './de';

const en: Copy = {
  meta: {
    title: 'Humanitl · You decide what goes out',
    description:
      'Humanitl puts your AI coding agent in a sandbox without a network. Whatever none of your rules allows, it holds until you decide. Local, open source, for Linux and macOS.',
  },
  nav: {
    how: 'How it works',
    security: 'Security',
    download: 'Download',
    menu: 'Menu',
    home: 'Humanitl, home',
    language: 'Language',
    docs: 'Docs',
  },
  hero: {
    title: ['You decide', 'what goes out.'],
    lede: 'Your AI agent works in a sandbox without a network. Whatever none of your rules allows, Humanitl holds until you decide.',
    cta: 'Download',
    ctaFor: 'for macOS and Linux',
    source: 'Source on GitHub',
    scroll: 'Keep scrolling',
  },
  stage: {
    title: 'Modules in focus',
    previous: 'Previous module',
    next: 'Next module',
    of: 'of',
    modules: [
      {
        id: 'intercept',
        name: 'Intercept',
        title: 'What no rule allows, stops.',
        text: 'Headers, parameters, body: you see what your agent wants to send, and you allow it, edit it or block it.',
        alt: 'Humanitl, Intercept: 15 held requests to registry.npmjs.org, the first with all its headers, below it Allow, Edit + Allow and Block.',
      },
      {
        id: 'notice',
        name: 'Reason',
        title: 'Explained, not hidden.',
        text: 'Humanitl says why a request waits: no rule matches, so it asks you. The arrow turns a decision into a rule.',
        alt: 'Humanitl explains a waiting request: held because no rule matches, the default is ask.',
      },
      {
        id: 'history',
        name: 'History',
        title: 'Read back what went out.',
        text: 'Requests land in the history, filterable by host, decision and findings. If recording fails, traffic goes on, and the app says so on every card.',
        alt: 'Humanitl, History: a table of requests with time, method, host, path and decision.',
      },
      {
        id: 'rules',
        name: 'Rules',
        title: 'Rules you can read.',
        text: 'Allow a domain for this session or for good, block another. Whatever no rule decides waits for you.',
        alt: 'Humanitl, Rules: bundled rules for models.dev and a local model, below them the default ask.',
      },
      {
        id: 'sandbox',
        name: 'Sandbox',
        title: 'A sandbox without a network.',
        text: 'The agent sees its project and nothing else of your machine. Its only way out goes through Humanitl.',
        alt: 'Humanitl, Sandbox: the agent’s terminal and the table of paths it sees.',
      },
      {
        id: 'palette',
        name: 'Commands',
        title: 'Keyboard first.',
        text: 'The command palette reaches every section. Keys decide a request: Enter allows it, E opens it for editing, B blocks it.',
        alt: 'Humanitl with the command palette open over Intercept.',
      },
      {
        id: 'settings',
        name: 'Model',
        title: 'Your model, your key.',
        text: 'You enter the endpoint and the model yourself. On Linux the access key lives in the system keyring, otherwise in a file only your account can read. It never stands in the configuration.',
        alt: 'Humanitl, Settings: endpoint, model and access key of the language model.',
      },
    ],
  },
  guarantees: {
    title: 'Physics, not trust.',
    lede: 'Three guarantees, each checkable with one command. On Linux with bubblewrap and seccomp, on macOS with Seatbelt. Two channels stay open and visible: the project folder and the declared connection to the language model.',
    items: [
      {
        title: 'No way out.',
        text: 'On Linux the sandbox has no network interface. On macOS every connection except the door ends in EPERM.',
      },
      {
        title: 'Exactly one door.',
        text: 'A single socket leads out of the sandbox, and it leads to Humanitl. That is where you decide.',
      },
      {
        title: 'The kernel holds.',
        text: 'seccomp on Linux, a Seatbelt profile on macOS: the agent cannot open a new door for itself.',
      },
    ],
    check: 'Check all three at once:',
    more: 'The security argument',
  },
  closing: {
    title: 'Keep your hand on the network.',
    text: 'Free and open source. No account, no cloud: Humanitl runs on your machine.',
    cta: 'Download Humanitl',
  },
  download: {
    title: 'Download',
    metaDescription: 'Download Humanitl for macOS and Linux: disk image, zip, Debian package and archive, straight from the GitHub releases.',
    lede: 'Pick your system. Every file comes unchanged from the GitHub releases, built by GitHub Actions from the public source.',
    version: 'Version',
    published: 'published on',
    whatsNew: 'What’s new?',
    download: 'download',
    downloads: 'downloads',
    downloadsNote: 'How often GitHub has served the files of every version.',
    systems: 'System',
    recommended: 'Recommended',
    get: 'Download',
    checksums: 'SHA-256 checksums',
    none: 'The releases cannot be reached right now. Every file is on GitHub.',
    toGitHub: 'To the releases on GitHub',
    older: 'Earlier versions',
    steps: 'What comes next',
    kinds: {
      dmg: 'Disk image',
      zip: 'Zip archive',
      deb: 'Debian package',
      appimage: 'AppImage, no installation',
      tar: 'Archive without the app',
    },
    platforms: {
      macos: {
        name: 'macOS',
        req: 'Apple Silicon and Intel. Tested on macOS 15 and 27.',
        steps: [
          'Drag Humanitl.app into Applications.',
          'The app is signed ad hoc only, so macOS refuses the first start. Allow it once in System Settings › Privacy & Security › “Open Anyway”.',
          'Start Humanitl. Under “Set up Humanitl”, “Install and start” sets up the background service as a launchd agent.',
        ],
        command:
          'xattr -dr com.apple.quarantine /Applications/Humanitl.app\n/Applications/Humanitl.app/Contents/MacOS/bin/humanitl daemon install\n/Applications/Humanitl.app/Contents/MacOS/bin/humanitl sandbox check',
        commandNote: 'Or all of it in a terminal: allow the first start, set up the service, check the sandbox.',
      },
      linux: {
        name: 'Linux',
        req: 'Debian 13 or Ubuntu 24.04 and newer, x86_64.',
        steps: [
          'Set up the APT repository apt.nurkert.de and install Humanitl. Later versions then arrive with apt upgrade.',
          'Set up the background service.',
          'Check the sandbox: all three guarantees must hold.',
        ],
        command: 'curl -fsSL https://apt.nurkert.de/install/humanitl | sudo sh\nhumanitl daemon install\nhumanitl sandbox check',
        commandNote: 'In a terminal. The script places the repository’s signing key, adds the source and installs the package.',
        script: 'If you do not trust a pipe into a root shell, read the script first or download the Debian package from this page.',
      },
    },
  },
  legal: {
    imprint: 'Imprint',
    privacy: 'Privacy',
    imprintTitle: 'Imprint',
    privacyTitle: 'Privacy policy',
    forkNote:
      'This page was built from a copy (fork) of the Humanitl project. Whoever runs it publicly usually needs an imprint of their own and sets the repository variable IMPRINT_URL for it.',
  },
  privacy: {
    updated: 'As of 30 October 2026',
    sections: [
      {
        title: 'In short',
        html: 'This website sets <strong>no cookies</strong>, uses <strong>no tracking</strong> or analytics and loads <strong>no third-party content</strong>. Fonts, images and scripts come from this domain only. There are no forms and no accounts. The download figures are fetched from GitHub when the site is built, not by your browser.',
      },
      {
        title: 'Controller',
        html: 'The person named in the <a href="/en/imprint/">imprint</a> is responsible for this website.',
      },
      {
        title: 'Hosting by GitHub Pages',
        html: 'The website is served by GitHub Pages, a service of GitHub B.V., Prins Bernhardplein 200, 1097 JB Amsterdam, Netherlands, and GitHub Inc., USA. When you visit, GitHub processes technically necessary data such as the IP address, date and time, the page requested and details of browser and operating system, to deliver the page and protect it from abuse. The legal basis is our legitimate interest in a secure and reliable service (Art. 6(1)(f) GDPR). A transfer to the USA is possible; GitHub is certified under the EU-U.S. Data Privacy Framework. Details: <a href="https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement">GitHub’s privacy statement</a>.',
      },
      {
        title: 'Downloads and external links',
        html: 'The download buttons lead to the files on GitHub. Only when you follow such a link do GitHub’s privacy terms apply.',
      },
      {
        title: 'The app',
        html: 'Humanitl runs locally on your machine and sends <strong>no data to us</strong>: no account, no telemetry, no crash reports. What the app lets onto the network, you decide request by request.',
      },
      {
        title: 'Your rights',
        html: 'You have the right of access, rectification, erasure and restriction of processing, to data portability and to object to processing based on legitimate interests (Art. 15 to 21 GDPR). You may also lodge a complaint with a data protection authority (Art. 77 GDPR). As we store no data about visitors ourselves, requests about server logs are usually a matter for GitHub.',
      },
    ],
  },
  footer: {
    line: 'Open source under the GNU GPL 3.0.',
    github: 'GitHub',
  },
  docs: {
    title: 'Documentation',
    lede: 'Everything about Humanitl, built straight from docs/ in the repository: installing and using it, the full reference, the security model and every architecture decision.',
    nav: 'Documentation',
    onThisPage: 'On this page',
    breadcrumb: 'Breadcrumb',
    pagination: 'Pagination',
    previous: 'Previous',
    next: 'Next',
    editOnGitHub: 'Edit on GitHub',
    history: 'History',
    sourceOnGitHub: 'Source on GitHub',
    pages: 'pages',
    appliesTo: 'Applies to Humanitl 0.1',
    menu: 'Menu',
    close: 'Close',
    german: 'German',
    generated: 'Generated',
    groups: {
      start: '',
      using: 'Install Humanitl and run your first session.',
      reference: 'Every rule, command, configuration key and diagnostic code.',
      security: 'What Humanitl promises, what it defends against and where the limits are.',
      building: 'How Humanitl is built: layers, design and flows.',
      decisions: 'The architecture decisions, each with its reasons and the alternatives.',
      more: 'Further pages.',
    },
  },
};

export default en;
