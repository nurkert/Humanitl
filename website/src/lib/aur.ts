// Fragt beim Bauen, ob das AUR-Paket humanitl-bin veröffentlicht ist (HUM-339), nie aus dem
// Browser: Besucher kontaktieren keinen Dritten. Hochgeladen wird das Paket vom Maintainer
// von Hand nach dem Release; bis dahin, und wenn das AUR nicht antwortet, nennt die Seite
// es nicht. Ein Fehler beim Fragen bricht den Bau also nie ab.

export const aurPackage = 'humanitl-bin';
export const aurPackageUrl = `https://aur.archlinux.org/packages/${aurPackage}`;

const api = `https://aur.archlinux.org/rpc/v5/info?arg[]=${aurPackage}`;

export interface AurPackage {
  /** Fassung im AUR, mit pkgrel, etwa „0.0.12-1“. */
  version: string;
  url: string;
}

interface AurInfo {
  resultcount?: number;
  type?: string;
  results?: { Name?: unknown; Version?: unknown }[];
}

export async function loadAur(): Promise<AurPackage | null> {
  try {
    const response = await fetch(api, { headers: { Accept: 'application/json' }, signal: AbortSignal.timeout(10_000) });
    if (!response.ok) return null;
    const data = (await response.json()) as AurInfo;
    if (data.type !== 'multiinfo') return null;
    const entry = data.results?.find((result) => result.Name === aurPackage);
    // Nur eine Fassung aus Ziffern, Punkten und dem pkgrel landet auf der Seite.
    if (!entry || typeof entry.Version !== 'string' || !/^[0-9][0-9.]*-[0-9]+$/.test(entry.Version)) return null;
    return { version: entry.Version, url: aurPackageUrl };
  } catch {
    // Offline gebaut, Zeitüberschreitung oder unerwartete Antwort: das Paket bleibt ungenannt.
    return null;
  }
}
