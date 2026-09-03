//! Der Domain-Katalog am Ereignisstrom (HUM-031).
//!
//! Der Katalog beantwortet zu jedem Ziel drei Fragen aus gebündelten Daten:
//! Wie heißt die registrierbare Domain, gehört sie zu einem bekannten Dienst,
//! und wie verbreitet ist sie? Dazu kommen zwei Zahlen aus dieser Sitzung:
//! wann der Host zum ersten Mal vorkam und wie oft. Geholt wird dafür nichts
//! (ADR-006).
//!
//! # Warum die Tabelle hier liegt und nicht im Proxy
//!
//! Gezählt werden darf genau einmal je eingetroffener Anfrage: `seen_count`
//! ist eine Beobachtung, keine Schätzung, und zwei Zuhörer am Ereignisstrom
//! dürfen sie nicht verdoppeln. Der Einzige, der weiß, wann eine Anfrage
//! ankommt, ist der Proxy — und `humanitl-proxy` darf `humanitl-catalog` nicht
//! kennen (`backlog/CONVENTIONS.md` 3.1). Die Naht dazwischen ist
//! [`humanitl_proxy::DomainSink`]: Der Proxy ruft sie in seinem
//! Veröffentlichungs-Trichter genau einmal je `Received` auf, diese Tabelle
//! fragt den Katalog und legt die Antwort ab. Wenn das Ereignis bei einem
//! Zuhörer ankommt, steht sie schon da.
//!
//! # Was die Tabelle hält
//!
//! Je Flow dieser Sitzung eine Antwort, so lange, wie der Daemon läuft — wie
//! die [`FlowRegistry`](humanitl_proxy::FlowRegistry) daneben, die dieselbe
//! Lebensdauer hat. Der Apex und die Katalog-Kennung wandern zusätzlich in die
//! Aufzeichnung ([`Recorder::set_domain`]), damit die History sie auch nach
//! einem Neustart zeigen kann; die Zähler bleiben ausdrücklich im Prozess,
//! denn „zum ersten Mal" heißt „zum ersten Mal in dieser Sitzung".

use std::sync::Arc;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use humanitl_catalog::{Catalog, DomainInfo};
use humanitl_core::{FlowId, HostName};
use humanitl_proxy::DomainSink;
use humanitl_recorder::Recorder;

/// Was der Katalog zu den Flows dieser Sitzung gesagt hat.
///
/// `Clone` ist sie nicht: Sie wird als [`Arc`] geteilt, einmal an den Proxy
/// (als [`DomainSink`]) und einmal an den gRPC-Dienst, der sie liest.
#[derive(Debug)]
pub struct DomainTable {
    catalog: Arc<Catalog>,
    recorder: Option<Recorder>,
    seen: DashMap<FlowId, DomainInfo>,
}

impl DomainTable {
    /// Eine Tabelle über diesem Katalog.
    ///
    /// Mit `recorder` wandern Apex und Katalog-Kennung zusätzlich in die
    /// Aufzeichnung; ohne ihn bleibt beides im Prozess.
    #[must_use]
    pub fn new(catalog: Arc<Catalog>, recorder: Option<Recorder>) -> Self {
        Self {
            catalog,
            recorder,
            seen: DashMap::new(),
        }
    }

    /// Der Katalog, den diese Tabelle befragt.
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Was der Katalog zu diesem Flow gesagt hat, als er ankam.
    #[must_use]
    pub fn get(&self, flow: FlowId) -> Option<DomainInfo> {
        self.seen.get(&flow).map(|entry| entry.value().clone())
    }

    /// Was der Katalog über einen Host sagt, ohne zu zählen.
    ///
    /// Der Weg für einen Flow, den diese Sitzung nicht gesehen hat: Die
    /// History zeigt auch Flows von gestern, und deren Zähler gehören nicht
    /// dieser Sitzung.
    #[must_use]
    pub fn describe(&self, host: &HostName) -> DomainInfo {
        self.catalog.describe(host)
    }

    /// Wie viele Flows dieser Sitzung eine Antwort haben.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Wahr, solange kein Flow eingetroffen ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl DomainSink for DomainTable {
    /// Verbucht die Beobachtung und legt die Antwort für den Ereignisstrom ab.
    ///
    /// Genau ein [`Catalog::info`] je Aufruf, und der Proxy ruft genau einmal
    /// je `Received` auf. Steht schon eine Antwort für diesen Flow, bleibt sie
    /// stehen und es wird nicht noch einmal gezählt: Eine doppelt gemeldete
    /// Anfrage wäre ein Fehler im Daemon, kein zweiter Besuch des Hosts.
    fn observe(&self, flow: FlowId, host: &HostName, at: SystemTime) {
        if self.seen.contains_key(&flow) {
            return;
        }
        let info = self.catalog.info(host, DateTime::<Utc>::from(at));
        if let Some(recorder) = self.recorder.as_ref() {
            recorder.set_domain(flow, info.apex.clone(), info.catalog_id.clone());
        }
        self.seen.insert(flow, info);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Arc;
    use std::time::SystemTime;

    use humanitl_catalog::Catalog;
    use humanitl_core::{FlowId, HostName};
    use humanitl_proxy::DomainSink as _;

    use super::DomainTable;

    #[test]
    fn a_flow_is_counted_once_and_the_answer_stays() {
        let table = DomainTable::new(Arc::new(Catalog::empty()), None);
        let flow = FlowId::new();
        let host = HostName::parse("api.github.com").unwrap();

        assert!(table.is_empty());
        table.observe(flow, &host, SystemTime::now());
        table.observe(flow, &host, SystemTime::now());

        let info = table.get(flow).expect("the flow has an answer");
        assert_eq!(info.seen_count, 1, "one request, one observation");
        assert_eq!(info.apex.as_deref(), Some("github.com"));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn a_flow_nobody_saw_has_no_answer() {
        let table = DomainTable::new(Arc::new(Catalog::empty()), None);
        assert!(table.get(FlowId::new()).is_none());
    }
}
