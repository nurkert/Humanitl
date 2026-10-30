// Liest die GitHub-Releases beim Bauen, damit Besucher nie einen Dritten kontaktieren.
// Die Seite wird bei jedem veröffentlichten Release und täglich neu gebaut
// (.github/workflows/website.yml); so bleiben Dateien und Download-Zahlen aktuell.

export type Platform = 'macos' | 'linux';

export interface Asset {
  platform: Platform;
  /** Was die Datei ist, für die Beschriftung: Disk-Image, Zip, Debian-Paket, RPM, AppImage, Archiv. */
  kind: 'dmg' | 'zip' | 'deb' | 'rpm' | 'appimage' | 'tar';
  name: string;
  url: string;
  sizeMb: number;
  /** Wie oft GitHub die Datei ausgeliefert hat. */
  count: number;
}

export interface Release {
  version: string;
  url: string;
  publishedAt: Date;
  prerelease: boolean;
  assets: Asset[];
  checksumsUrl?: string;
  totalDownloads: number;
}

const api = 'https://api.github.com/repos/nurkert/Humanitl/releases?per_page=100';

const kinds: [Asset['kind'], Platform, RegExp][] = [
  ['dmg', 'macos', /macos\.dmg$/i],
  ['zip', 'macos', /macos\.zip$/i],
  ['deb', 'linux', /\.deb$/i],
  ['rpm', 'linux', /\.x86_64\.rpm$/i],
  ['appimage', 'linux', /x86_64\.AppImage$/i],
  ['tar', 'linux', /linux.*\.tar\.gz$/i],
];

interface ApiAsset {
  name: string;
  browser_download_url: string;
  size: number;
  download_count?: number;
}
interface ApiRelease {
  tag_name: string;
  html_url: string;
  published_at: string;
  prerelease: boolean;
  draft: boolean;
  assets: ApiAsset[];
}

// Ein Bau fragt GitHub einmal; Startseite und Download-Seite sehen dieselben Daten.
let cached: Promise<Release[]> | undefined;

export function loadReleases(): Promise<Release[]> {
  cached ??= fetchReleases();
  return cached;
}

async function fetchReleases(): Promise<Release[]> {
  const headers: Record<string, string> = { Accept: 'application/vnd.github+json' };
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  try {
    const response = await fetch(api, { headers, signal: AbortSignal.timeout(10_000) });
    if (!response.ok) return [];
    const data = (await response.json()) as ApiRelease[];
    return data
      .filter((release) => !release.draft)
      .map((release) => ({
        version: release.tag_name.replace(/^v/, ''),
        url: release.html_url,
        publishedAt: new Date(release.published_at),
        prerelease: release.prerelease,
        totalDownloads: release.assets.reduce((sum, asset) => sum + (asset.download_count ?? 0), 0),
        checksumsUrl: release.assets.find((asset) => /^SHA256SUMS$/i.test(asset.name))?.browser_download_url,
        assets: release.assets.flatMap((asset) => {
          const match = kinds.find(([, , pattern]) => pattern.test(asset.name));
          return match
            ? [
                {
                  kind: match[0],
                  platform: match[1],
                  name: asset.name,
                  url: asset.browser_download_url,
                  sizeMb: Math.round((asset.size / 1_048_576) * 10) / 10,
                  count: asset.download_count ?? 0,
                },
              ]
            : [];
        })
        // Die Reihenfolge der Muster ist die Reihenfolge auf der Seite: zuerst das Disk-Image
        // und das Debian-Paket, unabhängig von der Reihenfolge des Hochladens.
        .sort((a, b) => kinds.findIndex(([kind]) => kind === a.kind) - kinds.findIndex(([kind]) => kind === b.kind)),
      }))
      .sort((a, b) => b.publishedAt.getTime() - a.publishedAt.getTime());
  } catch {
    // Offline gebaut oder die API hakt: Die Seite zeigt dann den Verweis auf GitHub.
    return [];
  }
}
