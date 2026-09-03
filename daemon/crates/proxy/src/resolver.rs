//! Der Resolver-Port: Namen zu Adressen, aber erst nach der Entscheidung.
//!
//! Der Proxy löst einen Hostnamen ausschließlich hier auf, und ausschließlich
//! nachdem eine Anfrage erlaubt wurde (ADR-006, `backlog/CONVENTIONS.md` 4.10:
//! kein `GaiResolver`, kein DNS im Connector). So kann eine Anfrage, die noch
//! in der Warteschlange liegt oder geblockt wird, keine DNS-Abfrage auslösen —
//! das wäre bereits ein beobachtbares Signal nach außen, bis zu 63 Bytes je
//! Label an den Namensdienst und dessen Upstream.
//!
//! Der einzige Aufrufer im Proxy ist [`Upstream::forward`](crate::upstream::Upstream::forward),
//! und der läuft erst, wenn der Flow in `Decided(Allow | AllowEdited)` steht.
//! Weder die Authority-Prüfung ([`crate::connect`]) noch das Halten
//! ([`crate::hold`]) fassen den Port an; der Test `dns_after_allow.rs` prüft
//! beides, einmal zur Laufzeit über einen Resolver, der jeden Aufruf vor der
//! Entscheidung als Verstoß aufschreibt, und einmal am Quelltext.
//!
//! # Aufbau
//!
//! [`ResolverPort`] ist der Stapel, den der Daemon verdrahtet, von außen nach
//! innen:
//!
//! 1. [`OverrideResolver`] beantwortet, was in `resolver.overrides` steht, ohne
//!    jede Abfrage.
//! 2. [`CachingResolver`] beantwortet, was noch nicht älter als
//!    `resolver.cache_ttl_secs` ist, ebenfalls ohne Abfrage.
//! 3. [`SystemResolver`] fragt den Namensdienst des Systems
//!    (`tokio::net::lookup_host`, also getaddrinfo in einem Blocking-Pool).
//!
//! [`ResolverPort::stats`] liefert die Zähler dieses Stapels
//! ([`ResolverStats`]). Sie sind absichtlich getrennt: Ein Cache-Treffer ist
//! keine Auflösung, und eine feste Zuordnung auch nicht. Nur `lookups` zählt,
//! was den Rechner wirklich verlassen hat; das ist die Zahl, die eine Aussage
//! über DNS-Leaks trägt (`backlog/CONVENTIONS.md` 4.13: nie mehr behaupten als
//! bewiesen ist).
//!
//! # Was der Zwischenspeicher nicht ist
//!
//! Er speichert Adressen, nie Entscheidungen. Ein Treffer ersetzt nur die
//! Abfrage beim Namensdienst; die Regelauswertung, der Hold und die Prüfung der
//! Adresse ([`select`]) laufen für jede Anfrage neu. Ein Eintrag, der schon da
//! war, kann deshalb keine Anfrage an einer späteren Sperre vorbeischleusen.
//! Fehlschläge werden nicht gespeichert, damit eine kurze Störung einen Host
//! nicht für die ganze Frist unerreichbar macht.
//!
//! Ein späterer `hickory`-Adapter (ADR, Post-MVP) ersetzt nur
//! [`SystemResolver`]; erst er kann `resolver.nameserver` bedienen.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use dashmap::DashMap;
use humanitl_config::{IpPreference, ResolverConfig};
use humanitl_core::diagnostics::codes::CONFIG_003;
use humanitl_core::{Diagnostic, FixAction, Severity, ip_is_private};

/// So viele Namen behält der Zwischenspeicher höchstens.
///
/// Ein Agent, der viele Hosts anfasst, soll den Daemon nicht wachsen lassen;
/// die Grenze ist großzügig genug, dass eine Sitzung sie im Alltag nicht
/// erreicht.
const CACHE_CAPACITY: usize = 512;

/// Warum eine Auflösung fehlschlug.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// Der Name ergab keine Adresse.
    #[error("no address for {host}")]
    NotFound {
        /// Der gefragte Name.
        host: String,
    },
    /// Der Namensdienst antwortete mit einem Fehler.
    #[error("resolving {host} failed: {reason}")]
    Failed {
        /// Der gefragte Name.
        host: String,
        /// Der Grund, so wie ihn das System nennt.
        reason: String,
    },
}

/// Löst Hostnamen zu Adressen auf, nach der Entscheidung.
#[async_trait]
pub trait Resolver: Send + Sync {
    /// Alle Adressen für `host`, in der Reihenfolge des Namensdienstes.
    ///
    /// `host` ist ein normalisierter DNS-Name (A-Label). Die Auswahl einer
    /// Adresse (IPv4/IPv6-Präferenz, private Bereiche) trifft [`select`], nicht
    /// der Adapter.
    ///
    /// # Errors
    ///
    /// [`ResolveError`], wenn kein Eintrag existiert oder der Dienst scheitert.
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError>;
}

// ---------------------------------------------------------------------------
// Zähler
// ---------------------------------------------------------------------------

/// Was der Resolver-Stapel getan hat, als Momentaufnahme.
///
/// Die Felder sind getrennt, weil sie Verschiedenes bedeuten: Nur `lookups`
/// beschreibt Abfragen, die den Rechner verlassen haben. Wer wissen will, wie
/// oft der Proxy überhaupt eine Adresse brauchte, nimmt [`ResolverStats::answers`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResolverStats {
    /// Abfragen an den Namensdienst, also DNS-Verkehr des Hosts.
    ///
    /// Gezählt wird der Versuch, nicht der Erfolg: Ein Fehlschlag hat trotzdem
    /// gefragt und steht deshalb auch hier. Cache-Treffer und feste Zuordnungen
    /// zählen **nicht** mit, denn sie fragen niemanden.
    pub lookups: u64,
    /// Namen, die aus dem Zwischenspeicher beantwortet wurden. Keine Abfrage.
    pub cache_hits: u64,
    /// Namen, die eine feste Zuordnung aus `resolver.overrides` beantwortet
    /// hat. Keine Abfrage.
    pub overrides: u64,
    /// Abfragen aus `lookups`, die mit einem [`ResolveError`] endeten.
    pub failures: u64,
}

impl ResolverStats {
    /// Wie oft der Proxy nach einer Entscheidung eine Adresse gebraucht hat.
    ///
    /// Die Summe aus Abfragen, Cache-Treffern und festen Zuordnungen. Sie ist
    /// immer mindestens so groß wie `lookups`.
    #[must_use]
    pub const fn answers(&self) -> u64 {
        self.lookups
            .saturating_add(self.cache_hits)
            .saturating_add(self.overrides)
    }
}

/// Die laufenden Zähler hinter [`ResolverStats`].
///
/// Wird geteilt, weil die Schichten des Stapels verschiedene Felder erhöhen;
/// gelesen wird immer als Ganzes über [`ResolverMetrics::snapshot`].
#[derive(Debug, Default)]
pub struct ResolverMetrics {
    lookups: AtomicU64,
    cache_hits: AtomicU64,
    overrides: AtomicU64,
    failures: AtomicU64,
}

impl ResolverMetrics {
    /// Frische Zähler, alle auf null.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Der Stand der Zähler in diesem Augenblick.
    #[must_use]
    pub fn snapshot(&self) -> ResolverStats {
        ResolverStats {
            lookups: self.lookups.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            overrides: self.overrides.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
        }
    }

    fn count_lookup(&self) {
        self.lookups.fetch_add(1, Ordering::Relaxed);
    }

    fn count_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    fn count_override(&self) {
        self.overrides.fetch_add(1, Ordering::Relaxed);
    }

    fn count_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Auswahl und Prüfung der Adresse
// ---------------------------------------------------------------------------

/// Warum keine der aufgelösten Adressen benutzt werden darf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressRefusal {
    /// Der Name zeigt in einen privaten oder lokalen Bereich, und die
    /// entscheidende Regel trägt kein `allow_private`.
    ///
    /// Das ist der Rebinding-Fall aus ADR-006: ein öffentlicher Name, der auf
    /// den Router, den Metadaten-Dienst einer Cloud oder den LLM-Host zeigt.
    Private(IpAddr),
    /// Nach dem Aussortieren blieb keine Adresse übrig, zu der sich überhaupt
    /// eine TCP-Verbindung aufbauen ließe.
    NoUsable,
}

/// Wählt aus den aufgelösten Adressen die eine, die angeheftet wird.
///
/// Bevorzugt die Familie aus `prefer`; ist keine davon dabei, gewinnt die
/// erste Adresse. `None`, wenn die Liste leer ist.
///
/// Prüft nichts. Der Weg des Proxys geht über [`select`].
#[must_use]
pub fn pick(addrs: &[IpAddr], prefer: IpPreference) -> Option<IpAddr> {
    let wanted_v6 = matches!(prefer, IpPreference::Ipv6);
    addrs
        .iter()
        .find(|ip| ip.is_ipv6() == wanted_v6)
        .or_else(|| addrs.first())
        .copied()
}

/// Prüft die Antwort des Namensdienstes und wählt die Adresse, die angeheftet
/// wird.
///
/// In dieser Reihenfolge:
///
/// 1. Adressen, die nie ein Ziel sein können, fallen weg: Multicast, „dieses
///    Netz" (`0.0.0.0`, `::`) und die Broadcast-Adresse. Sie fallen auch dann
///    weg, wenn `allow_private` gesetzt ist, denn erlaubt ist damit ein
///    privates Netz, kein unmögliches Ziel.
/// 2. Ohne `allow_private` ist **jede** verbleibende private Adresse ein
///    [`AddressRefusal::Private`], nicht nur die, die die Präferenz gewählt
///    hätte. Wer eine Antwort aus einer öffentlichen und einer privaten Adresse
///    baut, soll nicht darauf hoffen können, dass die Prüfung an der falschen
///    Stelle hinschaut.
/// 3. Aus dem Rest wählt [`pick`] nach `prefer`.
///
/// # Errors
///
/// [`AddressRefusal::Private`] für den Rebinding-Fall,
/// [`AddressRefusal::NoUsable`], wenn nichts Verbindbares übrig bleibt (auch
/// bei leerer Antwort).
pub fn select(
    addrs: &[IpAddr],
    prefer: IpPreference,
    allow_private: bool,
) -> Result<IpAddr, AddressRefusal> {
    let usable: Vec<IpAddr> = addrs
        .iter()
        .copied()
        .filter(|ip| !is_unusable(*ip))
        .collect();
    if !allow_private && let Some(private) = usable.iter().copied().find(|ip| ip_is_private(*ip)) {
        return Err(AddressRefusal::Private(private));
    }
    pick(&usable, prefer).ok_or(AddressRefusal::NoUsable)
}

/// Wahr, wenn zu dieser Adresse keine sinnvolle TCP-Verbindung führt.
fn is_unusable(ip: IpAddr) -> bool {
    match canonical(ip) {
        IpAddr::V4(v4) => v4.is_multicast() || v4.is_unspecified() || v4.is_broadcast(),
        IpAddr::V6(v6) => v6.is_multicast() || v6.is_unspecified(),
    }
}

/// Entpackt IPv4-mapped (`::ffff:a.b.c.d`) und IPv4-compatible (`::a.b.c.d`)
/// Adressen, damit die Prüfung dieselbe Adresse in beiden Schreibweisen sieht.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4().map_or(ip, IpAddr::V4),
        other @ IpAddr::V4(_) => other,
    }
}

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// Der Namensdienst des Systems (getaddrinfo über `tokio::net::lookup_host`).
///
/// Der einzige Adapter, der wirklich fragt. `resolver.nameserver` kann er nicht
/// bedienen: getaddrinfo nimmt den Server aus `/etc/resolv.conf`. Der
/// `hickory`-Adapter, der das kann, ist Post-MVP; bis dahin meldet
/// [`ResolverPort::from_config`] den ungenutzten Schlüssel im Log.
#[derive(Debug, Clone, Default)]
pub struct SystemResolver;

#[async_trait]
impl Resolver for SystemResolver {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError> {
        // Port 0: wir wollen nur die Adressen, den Port kennt der Handler.
        let addrs: Vec<IpAddr> = tokio::net::lookup_host((host, 0))
            .await
            .map_err(|err| ResolveError::Failed {
                host: host.to_owned(),
                reason: err.to_string(),
            })?
            .map(|sock| sock.ip())
            .collect();
        if addrs.is_empty() {
            return Err(ResolveError::NotFound {
                host: host.to_owned(),
            });
        }
        Ok(addrs)
    }
}

/// Ein Eintrag des Zwischenspeichers: die Adressen und der Zeitpunkt, ab dem
/// sie nicht mehr gelten.
#[derive(Debug, Clone)]
struct CacheEntry {
    addrs: Vec<IpAddr>,
    expires: Instant,
}

/// Merkt sich Antworten für `resolver.cache_ttl_secs` Sekunden.
///
/// Der Zwischenspeicher spart Abfragen, nicht Entscheidungen: Er liegt
/// vollständig hinter dem Entscheidungspunkt, wird nur aus
/// [`Upstream::forward`](crate::upstream::Upstream::forward) erreicht und
/// liefert Adressen, die danach dieselbe Prüfung durchlaufen wie eine frische
/// Antwort ([`select`]). Eine Frist von null Sekunden schaltet ihn ganz ab.
///
/// Gespeichert wird die ganze Adressliste, nicht die ausgewählte Adresse: So
/// wirkt eine Änderung an `resolver.prefer` sofort, ohne dass jemand den
/// Speicher leeren muss.
pub struct CachingResolver {
    inner: Arc<dyn Resolver>,
    entries: DashMap<String, CacheEntry>,
    ttl: Duration,
    metrics: Arc<ResolverMetrics>,
}

impl std::fmt::Debug for CachingResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachingResolver")
            .field("ttl", &self.ttl)
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

impl CachingResolver {
    /// Ein Zwischenspeicher vor `inner`, mit `ttl` als Frist.
    ///
    /// `ttl` von null bedeutet: nichts speichern, jede Anfrage geht an `inner`.
    #[must_use]
    pub fn new(inner: Arc<dyn Resolver>, ttl: Duration, metrics: Arc<ResolverMetrics>) -> Self {
        Self {
            inner,
            entries: DashMap::new(),
            ttl,
            metrics,
        }
    }

    /// Wirft alle Einträge weg.
    ///
    /// Für den Daemon nach einer Änderung an `resolver.*`: Danach ist keine
    /// Antwort mehr im Umlauf, die unter der alten Einstellung entstanden ist.
    pub fn clear(&self) {
        self.entries.clear();
    }

    /// Wie viele Namen gerade gespeichert sind, abgelaufene eingeschlossen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Wahr, wenn nichts gespeichert ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Ein gültiger Eintrag für `host`; ein abgelaufener wird dabei entfernt.
    fn hit(&self, host: &str) -> Option<Vec<IpAddr>> {
        if self.ttl.is_zero() {
            return None;
        }
        let now = Instant::now();
        // Die Leihe des Eintrags endet mit diesem Block; `remove` unter einer
        // laufenden Leihe würde dashmap verklemmen.
        let fresh = {
            let entry = self.entries.get(host)?;
            (entry.expires > now).then(|| entry.addrs.clone())
        };
        if fresh.is_none() {
            self.entries.remove(host);
        }
        fresh
    }

    /// Legt eine frische Antwort ab.
    fn store(&self, host: &str, addrs: &[IpAddr]) {
        if self.ttl.is_zero() || addrs.is_empty() {
            return;
        }
        let Some(expires) = Instant::now().checked_add(self.ttl) else {
            return;
        };
        if self.entries.len() >= CACHE_CAPACITY {
            self.evict();
        }
        self.entries.insert(
            host.to_owned(),
            CacheEntry {
                addrs: addrs.to_vec(),
                expires,
            },
        );
    }

    /// Macht Platz: erst alles Abgelaufene, dann der Eintrag, dessen Frist als
    /// Nächstes endet.
    fn evict(&self) {
        let now = Instant::now();
        self.entries.retain(|_host, entry| entry.expires > now);
        if self.entries.len() < CACHE_CAPACITY {
            return;
        }
        // Die Leihe des Iterators endet mit dieser Anweisung, erst danach wird
        // entfernt.
        let next = self
            .entries
            .iter()
            .min_by_key(|entry| entry.expires)
            .map(|entry| entry.key().clone());
        if let Some(host) = next {
            self.entries.remove(&host);
        }
    }
}

#[async_trait]
impl Resolver for CachingResolver {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError> {
        if let Some(addrs) = self.hit(host) {
            self.metrics.count_cache_hit();
            return Ok(addrs);
        }
        self.metrics.count_lookup();
        match self.inner.resolve(host).await {
            Ok(addrs) => {
                self.store(host, &addrs);
                Ok(addrs)
            }
            Err(err) => {
                // Fehlschläge werden nicht gespeichert: Eine kurze Störung darf
                // einen Host nicht für die ganze Frist unerreichbar machen.
                self.metrics.count_failure();
                Err(err)
            }
        }
    }
}

/// Beantwortet feste Zuordnungen aus `resolver.overrides` selbst und reicht
/// alles Übrige weiter.
///
/// Damit lässt sich ein Host ohne Namensdienst erreichen (Testaufbau,
/// abgeschottetes Netz). Ein Treffer ist keine Auflösung und zählt deshalb in
/// [`ResolverStats::overrides`], nicht in `lookups`.
pub struct OverrideResolver {
    map: BTreeMap<String, IpAddr>,
    fallback: Arc<dyn Resolver>,
    metrics: Arc<ResolverMetrics>,
}

impl std::fmt::Debug for OverrideResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverrideResolver")
            .field("map", &self.map)
            .finish_non_exhaustive()
    }
}

impl OverrideResolver {
    /// Ein Vorschalter mit `map` vor `fallback`.
    ///
    /// Die Schlüssel werden wie Hostnamen normalisiert (klein, ohne Punkt am
    /// Ende), damit `API.GITHUB.COM.` in der Konfiguration denselben Eintrag
    /// trifft wie der Name aus der Anfrage (ADR-007).
    #[must_use]
    pub fn new(
        map: BTreeMap<String, IpAddr>,
        fallback: Arc<dyn Resolver>,
        metrics: Arc<ResolverMetrics>,
    ) -> Self {
        let map = map
            .into_iter()
            .map(|(host, ip)| (normalize_host(&host), ip))
            .collect();
        Self {
            map,
            fallback,
            metrics,
        }
    }

    /// Liest die Zuordnungen aus der Konfiguration.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit [`CONFIG_003`], wenn ein Wert keine IP-Adresse ist.
    /// Ein Name als Wert wäre ein zweiter Namensdienst durch die Hintertür und
    /// ist deshalb kein Tippfehler, den man großzügig auslegen darf.
    pub fn from_config(
        overrides: &BTreeMap<String, String>,
        fallback: Arc<dyn Resolver>,
        metrics: Arc<ResolverMetrics>,
    ) -> Result<Self, Diagnostic> {
        let mut map = BTreeMap::new();
        for (host, value) in overrides {
            let ip: IpAddr = value.parse().map_err(|_err| {
                Diagnostic::builder(CONFIG_003, Severity::Error)
                    .why(format!(
                        "resolver.overrides[\"{host}\"] is \"{value}\", which is not an IP address"
                    ))
                    .fix(FixAction::ChangeSetting {
                        key: format!("resolver.overrides.{host}"),
                        value: "an IPv4 or IPv6 address, for example 127.0.0.1".to_owned(),
                    })
                    .build()
            })?;
            map.insert(host.clone(), ip);
        }
        Ok(Self::new(map, fallback, metrics))
    }

    /// Wie viele feste Zuordnungen es gibt.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Wahr, wenn keine feste Zuordnung eingetragen ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[async_trait]
impl Resolver for OverrideResolver {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError> {
        if let Some(ip) = self.map.get(&normalize_host(host)) {
            self.metrics.count_override();
            return Ok(vec![*ip]);
        }
        self.fallback.resolve(host).await
    }
}

/// Ein Hostname in der Form, in der verglichen wird: klein, ohne Punkt am Ende.
fn normalize_host(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

// ---------------------------------------------------------------------------
// Der verdrahtete Port
// ---------------------------------------------------------------------------

/// Der fertige Resolver-Stapel des Daemons: Zuordnungen, Zwischenspeicher,
/// Namensdienst, mit Zählern.
///
/// Der Daemon baut ihn einmal, gibt ihn als
/// [`Arc<dyn Resolver>`](Resolver) an [`Upstream`](crate::upstream::Upstream)
/// und behält denselben [`Arc`], um [`ResolverPort::stats`] für
/// `daemon status --json` zu lesen.
pub struct ResolverPort {
    top: Arc<dyn Resolver>,
    cache: Arc<CachingResolver>,
    metrics: Arc<ResolverMetrics>,
}

impl std::fmt::Debug for ResolverPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolverPort")
            .field("cache", &self.cache)
            .field("stats", &self.metrics.snapshot())
            .finish_non_exhaustive()
    }
}

impl ResolverPort {
    /// Der Stapel über dem Namensdienst des Systems.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit [`CONFIG_003`], wenn `resolver.overrides` einen Wert
    /// enthält, der keine IP-Adresse ist.
    pub fn from_config(config: &ResolverConfig) -> Result<Self, Diagnostic> {
        if let Some(nameserver) = &config.nameserver {
            // Ehrlich statt still: Der Systemadapter nimmt den Server aus
            // /etc/resolv.conf und kann diesen Schlüssel nicht bedienen.
            tracing::warn!(
                %nameserver,
                "resolver.nameserver is set but the system resolver cannot use it; \
                 the hickory adapter that can is post-MVP"
            );
        }
        Self::over(Arc::new(SystemResolver), config)
    }

    /// Derselbe Stapel über einem anderen Adapter; im Test über einem Mock.
    ///
    /// # Errors
    ///
    /// Wie [`ResolverPort::from_config`].
    pub fn over(inner: Arc<dyn Resolver>, config: &ResolverConfig) -> Result<Self, Diagnostic> {
        let metrics = Arc::new(ResolverMetrics::new());
        let cache = Arc::new(CachingResolver::new(
            inner,
            Duration::from_secs(config.cache_ttl_secs),
            Arc::clone(&metrics),
        ));
        let top: Arc<dyn Resolver> = if config.overrides.is_empty() {
            Arc::clone(&cache) as Arc<dyn Resolver>
        } else {
            Arc::new(OverrideResolver::from_config(
                &config.overrides,
                Arc::clone(&cache) as Arc<dyn Resolver>,
                Arc::clone(&metrics),
            )?)
        };
        Ok(Self {
            top,
            cache,
            metrics,
        })
    }

    /// Der Stand der Zähler: was wirklich gefragt wurde und was nicht.
    #[must_use]
    pub fn stats(&self) -> ResolverStats {
        self.metrics.snapshot()
    }

    /// Leert den Zwischenspeicher (nach einer Änderung an `resolver.*`).
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Wie viele Namen gerade zwischengespeichert sind.
    #[must_use]
    pub fn cached(&self) -> usize {
        self.cache.len()
    }
}

#[async_trait]
impl Resolver for ResolverPort {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError> {
        self.top.resolve(host).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::BTreeMap;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use async_trait::async_trait;
    use humanitl_config::{IpPreference, ResolverConfig};

    use super::{
        AddressRefusal, CachingResolver, OverrideResolver, ResolveError, Resolver, ResolverMetrics,
        ResolverPort, pick, select,
    };

    /// Ein Adapter, der jeden Aufruf zählt und immer dieselbe Antwort gibt.
    struct Counting {
        answer: Vec<IpAddr>,
        calls: AtomicUsize,
        fail: bool,
    }

    impl Counting {
        fn answering(answer: Vec<IpAddr>) -> Arc<Self> {
            Arc::new(Self {
                answer,
                calls: AtomicUsize::new(0),
                fail: false,
            })
        }

        fn failing() -> Arc<Self> {
            Arc::new(Self {
                answer: Vec::new(),
                calls: AtomicUsize::new(0),
                fail: true,
            })
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Resolver for Counting {
        async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(ResolveError::NotFound {
                    host: host.to_owned(),
                });
            }
            Ok(self.answer.clone())
        }
    }

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn pick_honours_the_preference_then_falls_back() {
        let public = v4(93, 184, 216, 34);
        let v6 = IpAddr::V6(Ipv6Addr::LOCALHOST);

        assert_eq!(pick(&[public, v6], IpPreference::Ipv4), Some(public));
        assert_eq!(pick(&[public, v6], IpPreference::Ipv6), Some(v6));
        // Nur eine Familie da: die Präferenz kann nicht erfüllt werden, die
        // erste Adresse gewinnt.
        assert_eq!(pick(&[public], IpPreference::Ipv6), Some(public));
        assert_eq!(pick(&[], IpPreference::Ipv4), None);
    }

    #[test]
    fn select_refuses_any_private_address_in_the_answer() {
        let public = v4(93, 184, 216, 34);
        let private = v4(10, 0, 0, 1);
        let loopback = v4(127, 0, 0, 1);

        // Auch wenn die Präferenz die öffentliche Adresse gewählt hätte: Eine
        // Antwort, die eine private Adresse enthält, ist der Rebinding-Fall.
        assert_eq!(
            select(&[public, private], IpPreference::Ipv4, false),
            Err(AddressRefusal::Private(private))
        );
        assert_eq!(
            select(&[loopback], IpPreference::Ipv4, false),
            Err(AddressRefusal::Private(loopback))
        );
        // IPv4-mapped ist dieselbe Adresse.
        let mapped = IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped());
        assert_eq!(
            select(&[mapped], IpPreference::Ipv4, false),
            Err(AddressRefusal::Private(mapped))
        );
        // Mit `allow_private` ist genau das erlaubt.
        assert_eq!(select(&[loopback], IpPreference::Ipv4, true), Ok(loopback));
    }

    #[test]
    fn select_drops_addresses_that_can_never_be_a_target() {
        let public = v4(93, 184, 216, 34);
        let multicast = v4(224, 0, 0, 1);
        let unspecified = v4(0, 0, 0, 0);

        assert_eq!(
            select(&[multicast, public], IpPreference::Ipv4, false),
            Ok(public)
        );
        // Auch mit `allow_private` bleibt Unmögliches unmöglich.
        assert_eq!(
            select(&[multicast], IpPreference::Ipv4, true),
            Err(AddressRefusal::NoUsable)
        );
        assert_eq!(
            select(&[unspecified], IpPreference::Ipv4, true),
            Err(AddressRefusal::NoUsable)
        );
        assert_eq!(
            select(&[], IpPreference::Ipv4, true),
            Err(AddressRefusal::NoUsable)
        );
    }

    #[tokio::test]
    async fn a_cache_hit_is_not_a_lookup() {
        let inner = Counting::answering(vec![v4(93, 184, 216, 34)]);
        let metrics = Arc::new(ResolverMetrics::new());
        let cache = CachingResolver::new(
            Arc::clone(&inner) as Arc<dyn Resolver>,
            Duration::from_secs(60),
            Arc::clone(&metrics),
        );

        assert_eq!(cache.resolve("api.github.com").await.unwrap().len(), 1);
        assert_eq!(cache.resolve("api.github.com").await.unwrap().len(), 1);
        assert_eq!(cache.resolve("api.github.com").await.unwrap().len(), 1);

        assert_eq!(inner.calls(), 1, "asked the name service exactly once");
        let stats = metrics.snapshot();
        assert_eq!(stats.lookups, 1, "only the first call left the machine");
        assert_eq!(stats.cache_hits, 2);
        assert_eq!(stats.failures, 0);
        assert_eq!(stats.answers(), 3);
    }

    #[tokio::test]
    async fn an_expired_entry_is_asked_again() {
        let inner = Counting::answering(vec![v4(93, 184, 216, 34)]);
        let metrics = Arc::new(ResolverMetrics::new());
        let cache = CachingResolver::new(
            Arc::clone(&inner) as Arc<dyn Resolver>,
            Duration::from_millis(30),
            Arc::clone(&metrics),
        );

        assert!(cache.resolve("api.github.com").await.is_ok());
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(cache.resolve("api.github.com").await.is_ok());

        assert_eq!(inner.calls(), 2, "the stale answer was not reused");
        assert_eq!(metrics.snapshot().lookups, 2);
        assert_eq!(metrics.snapshot().cache_hits, 0);
    }

    #[tokio::test]
    async fn a_ttl_of_zero_switches_the_cache_off() {
        let inner = Counting::answering(vec![v4(93, 184, 216, 34)]);
        let metrics = Arc::new(ResolverMetrics::new());
        let cache = CachingResolver::new(
            Arc::clone(&inner) as Arc<dyn Resolver>,
            Duration::ZERO,
            Arc::clone(&metrics),
        );

        assert!(cache.resolve("api.github.com").await.is_ok());
        assert!(cache.resolve("api.github.com").await.is_ok());

        assert_eq!(inner.calls(), 2);
        assert_eq!(metrics.snapshot().cache_hits, 0);
        assert!(cache.is_empty(), "nothing is kept at all");
    }

    #[tokio::test]
    async fn a_failure_is_not_remembered() {
        let inner = Counting::failing();
        let metrics = Arc::new(ResolverMetrics::new());
        let cache = CachingResolver::new(
            Arc::clone(&inner) as Arc<dyn Resolver>,
            Duration::from_secs(60),
            Arc::clone(&metrics),
        );

        assert!(cache.resolve("broken.example").await.is_err());
        assert!(cache.resolve("broken.example").await.is_err());

        assert_eq!(inner.calls(), 2, "a failure is retried, not cached");
        let stats = metrics.snapshot();
        assert_eq!(stats.lookups, 2);
        assert_eq!(stats.failures, 2);
        assert_eq!(stats.cache_hits, 0);
    }

    #[tokio::test]
    async fn clearing_the_cache_asks_again() {
        let inner = Counting::answering(vec![v4(93, 184, 216, 34)]);
        let metrics = Arc::new(ResolverMetrics::new());
        let cache = CachingResolver::new(
            Arc::clone(&inner) as Arc<dyn Resolver>,
            Duration::from_secs(60),
            Arc::clone(&metrics),
        );

        assert!(cache.resolve("api.github.com").await.is_ok());
        cache.clear();
        assert!(cache.resolve("api.github.com").await.is_ok());

        assert_eq!(inner.calls(), 2);
    }

    #[tokio::test]
    async fn an_override_answers_without_asking_anyone() {
        let inner = Counting::answering(vec![v4(93, 184, 216, 34)]);
        let metrics = Arc::new(ResolverMetrics::new());
        let mut map = BTreeMap::new();
        map.insert("PINNED.TEST.".to_owned(), v4(203, 0, 113, 7));
        let resolver = OverrideResolver::new(
            map,
            Arc::clone(&inner) as Arc<dyn Resolver>,
            Arc::clone(&metrics),
        );

        assert_eq!(
            resolver.resolve("pinned.test").await.unwrap(),
            vec![v4(203, 0, 113, 7)]
        );
        assert_eq!(inner.calls(), 0, "an override never asks");
        let stats = metrics.snapshot();
        assert_eq!(stats.overrides, 1);
        assert_eq!(stats.lookups, 0, "an override is not a lookup");

        // Was nicht eingetragen ist, geht weiter nach unten.
        assert!(resolver.resolve("api.github.com").await.is_ok());
        assert_eq!(inner.calls(), 1);
    }

    #[test]
    fn an_override_that_is_not_an_address_is_a_config_diagnostic() {
        let metrics = Arc::new(ResolverMetrics::new());
        let inner = Counting::answering(vec![v4(93, 184, 216, 34)]);
        let mut overrides = BTreeMap::new();
        overrides.insert("pinned.test".to_owned(), "example.com".to_owned());
        let err = OverrideResolver::from_config(
            &overrides,
            Arc::clone(&inner) as Arc<dyn Resolver>,
            metrics,
        )
        .expect_err("a name is not an address");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("resolver.overrides"));
    }

    #[tokio::test]
    async fn the_port_counts_the_whole_stack() {
        let inner = Counting::answering(vec![v4(93, 184, 216, 34)]);
        let mut overrides = std::collections::BTreeMap::new();
        overrides.insert("pinned.test".to_owned(), "203.0.113.7".to_owned());
        let config = ResolverConfig {
            overrides,
            cache_ttl_secs: 60,
            ..ResolverConfig::default()
        };
        let port =
            ResolverPort::over(Arc::clone(&inner) as Arc<dyn Resolver>, &config).expect("config");

        assert_eq!(
            port.resolve("pinned.test").await.unwrap(),
            vec![v4(203, 0, 113, 7)]
        );
        assert!(port.resolve("api.github.com").await.is_ok());
        assert!(port.resolve("api.github.com").await.is_ok());

        let stats = port.stats();
        assert_eq!(stats.overrides, 1);
        assert_eq!(stats.lookups, 1);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.answers(), 3);
        assert_eq!(inner.calls(), 1);

        port.clear_cache();
        assert_eq!(port.cached(), 0);
    }
}
