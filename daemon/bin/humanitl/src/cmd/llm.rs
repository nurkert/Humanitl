//! `humanitl llm discover|test`: die Suche nach LLM-Servern im eigenen Netz
//! (HUM-076) und die Probe eines einzelnen Endpunkts (HUM-114).
//!
//! Ein gRPC-Aufruf und sonst nichts (ADR-018): Gesucht wird im Daemon, im
//! Host-Netz; hier wird angekündigt, gezählt und geschrieben. Die Ankündigung
//! ist Teil des Befehls und nicht Zierrat — ein Netzscan ist ein Vorgang, den
//! ein Mensch vorher verstehen soll, und dieselbe Zusage steht über dem Knopf
//! im Setup: nur das eigene `/24`, nur vier Ports, nur auf Zuruf.
//!
//! Die Zeilen kommen, sobald ein Server antwortet, und werden sofort
//! geschrieben. Wer den Befehl abbricht, beendet damit den Scan: Der Strom
//! fällt, und der Daemon hört auf zu fragen.

use humanitl_ipc::v1;
use serde_json::{Value, json};

use crate::cli::LlmCmd;
use crate::cmd::{Context, EXIT_OK, Failure, status_diagnostic};
use crate::render::table;

/// Die Spalten der Tabelle.
const HEADERS: [&str; 5] = ["HOST", "PORT", "PRODUCT", "MS", "MODELS"];

/// Wie viele Modellnamen eine Zeile zeigt, bevor sie zählt statt aufzuzählen.
const MODELS_IN_LINE: usize = 3;

/// Wie viele Modellnamen `llm test` untereinander zeigt.
const MODELS_SHOWN: usize = 6;

/// Wie viele Zeichen eines Modellnamens gezeigt werden.
const MODEL_NAME_CHARS: usize = 40;

/// Führt `humanitl llm <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; `LLM_008`, wenn es kein eigenes
/// Netz gibt oder das genannte weiter als ein `/24` ist; bei `test` außerdem
/// `LLM_007` für eine Adresse, die der Dienst nicht lesen kann, und `LLM_001`,
/// wenn der Endpunkt nicht antwortet. `LLM_003` ist keiner davon: Wer
/// antwortet, aber als keine bekannte API, hat geantwortet -- das steht als
/// Warnung unter der Ausgabe, und der Lauf endet mit `0`.
pub async fn run(ctx: &Context, cmd: &LlmCmd) -> Result<u8, Failure> {
    match cmd {
        LlmCmd::Discover { subnet, port } => discover(ctx, subnet.as_deref(), port).await,
        LlmCmd::Test { url, timeout_ms } => test(ctx, url, timeout_ms.unwrap_or_default()).await,
    }
}

/// `llm test URL [--timeout-ms N]`.
///
/// Gefragt wird im Daemon (ADR-018); hier steht die Ankündigung, die Ausgabe
/// und der Exit-Code. Ein Endpunkt, der nicht antwortet, ist ein Befund des
/// Dienstes (`LLM_001`) und keine eigene Erklärung: Was gemessen wurde, weiß
/// die Stelle, die gemessen hat.
async fn test(ctx: &Context, url: &str, timeout_ms: u32) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    announce_probe(url);

    let response = client
        .probe_llm(v1::ProbeLlmRequest {
            endpoint: url.to_owned(),
            timeout_ms,
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "ProbeLlm")))?
        .into_inner();

    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "endpoint": url,
            "flavor": flavor(response.flavor),
            "models": response.models,
            "latency_ms": response.latency_ms,
            "endpoint_is_private": response.endpoint_is_private,
            "diagnostics": response
                .diagnostics
                .iter()
                .filter_map(crate::cmd::from_proto)
                .map(|diagnostic| crate::render::diagnostic_json(&diagnostic))
                .collect::<Vec<Value>>(),
        }));
        return Ok(EXIT_OK);
    }

    ctx.render
        .line(&format!("flavor: {}", flavor(response.flavor)));
    ctx.render
        .line(&format!("latency: {} ms", response.latency_ms));
    if response.models.is_empty() {
        // Kein Modell ist eine Aussage und keine Lücke: Es hat sich nichts
        // gemeldet, was Humanitl kennt (`ProbeLlmResponse` im Vertrag).
        ctx.render.line("models: none");
    } else {
        ctx.render
            .line(&format!("models: {}", response.models.len()));
        for name in shown_models(&response.models) {
            ctx.render.line(&format!("  {name}"));
        }
        if let Some(hidden) = response.models.len().checked_sub(MODELS_SHOWN)
            && hidden > 0
        {
            ctx.render.line(&format!("  +{hidden} more"));
        }
    }
    // Die Befunde des Dienstes stehen unverändert darunter, auch die, die
    // keinen Fehlschlag bedeuten: `LLM_006` sagt, dass diese Adresse nicht im
    // eigenen Netz liegt, und das ist der Satz, der einen Menschen angeht.
    for wire in &response.diagnostics {
        if let Some(diagnostic) = crate::cmd::from_proto(wire) {
            ctx.render
                .note(crate::render::diagnostic_block(&diagnostic).trim_end());
        }
    }
    Ok(EXIT_OK)
}

/// Die Namen, die eine Ausgabe zeigt, jeder auf [`MODEL_NAME_CHARS`] gekürzt.
///
/// Dieselben Grenzen wie in der Oberfläche (`app/lib/core/domain/llm.dart`,
/// `LlmModelLimits`): Ein Name aus dem Netz ist Text von einer Maschine, über
/// die noch niemand entschieden hat, und er bekommt hier nicht mehr Platz als
/// dort. Gekürzt wird an Zeichen und nicht an Bytes, sonst stünde am Ende ein
/// halbes Zeichen.
fn shown_models(models: &[String]) -> Vec<String> {
    models
        .iter()
        .take(MODELS_SHOWN)
        .map(|name| {
            if name.chars().count() <= MODEL_NAME_CHARS {
                name.clone()
            } else {
                name.chars().take(MODEL_NAME_CHARS).collect()
            }
        })
        .collect()
}

/// Welche API geantwortet hat, in einem Wort.
fn flavor(flavor: i32) -> &'static str {
    match v1::LlmProduct::try_from(flavor) {
        Ok(v1::LlmProduct::Ollama) => "ollama",
        Ok(v1::LlmProduct::OpenaiCompatible) => "openai-compatible",
        _ => "unknown",
    }
}

/// Sagt vor dem ersten Paket, wohin gleich eine Verbindung geht.
///
/// **Geht mit Absicht nicht durch [`crate::render::Renderer`]**, aus demselben
/// Grund wie [`announce`] und wie im Doctor: Ein Ausgabeschalter darf nicht
/// bestimmen, ob ein Mensch von einer Verbindung erfährt.
fn announce_probe(endpoint: &str) {
    eprintln!(
        "contacting {endpoint} now: two GET requests, /api/tags then /v1/models, \
         no credentials, no redirects"
    );
}

/// `llm discover [--subnet CIDR] [--port PORT]...`.
async fn discover(ctx: &Context, subnet: Option<&str>, ports: &[u16]) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    announce(subnet, ports);

    let mut stream = client
        .discover_llm(v1::DiscoverRequest {
            subnet: subnet.unwrap_or_default().to_owned(),
            ports: ports.iter().map(|port| u32::from(*port)).collect(),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "DiscoverLlm")))?
        .into_inner();

    let mut found: Vec<v1::DiscoverResult> = Vec::new();
    while let Some(result) = stream
        .message()
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "DiscoverLlm")))?
    {
        // Während der Scan läuft, sieht ein Mensch auf `stderr`, dass etwas
        // passiert; die Tabelle steht danach auf `stdout` und ist als Ganzes
        // ausgerichtet. Ein Strom halbfertiger Tabellenzeilen wäre beides
        // nicht: weder Fortschritt noch Tabelle.
        if !ctx.render.is_json() {
            ctx.render.detail(&format!(
                "found {}:{} ({})",
                result.host,
                result.port,
                product(&result)
            ));
        }
        found.push(result);
    }

    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "servers": found.iter().map(server_json).collect::<Vec<Value>>(),
            "count": found.len(),
        }));
        return Ok(EXIT_OK);
    }

    if !found.is_empty() {
        let rows: Vec<Vec<String>> = found.iter().map(row).collect();
        ctx.render.line(table(&HEADERS, &rows).trim_end());
    }

    if found.is_empty() {
        ctx.render.note(
            "no LLM server answered. Ollama listens on localhost only unless OLLAMA_HOST=0.0.0.0 \
             is set; a server on another port needs --port",
        );
    } else {
        ctx.render.note(&format!(
            "{} server(s); set one with humanitl --llm http://HOST:PORT",
            found.len()
        ));
    }
    Ok(EXIT_OK)
}

/// Sagt vor dem ersten Paket, was gleich geschieht.
///
/// **Geht mit Absicht nicht durch [`crate::render::Renderer`].** Dessen
/// `detail` zeigt nur unter `-v`, sein `note` schweigt unter `--json` und
/// unter `-q`, und ein Ausgabeschalter darf nicht bestimmen, ob ein Mensch
/// erfährt, dass sein Rechner gleich 1016 Verbindungen in sein Netz aufbaut.
/// Dieselbe Regel wie bei `daemon install` und beim Doctor. `stdout` bleibt
/// unberührt, ein einziger JSON-Wert also weiterhin ein einziger.
fn announce(subnet: Option<&str>, ports: &[u16]) {
    let where_ = subnet.map_or_else(
        || "the local /24 of the default route".to_owned(),
        str::to_owned,
    );
    let ports = if ports.is_empty() {
        humanitl_ipc::DEFAULT_DISCOVER_PORTS
            .iter()
            .map(u16::to_string)
            .collect::<Vec<String>>()
            .join(", ")
    } else {
        ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<String>>()
            .join(", ")
    };
    eprintln!(
        "searching {where_} for LLM servers; connection attempts to ports {ports} and nothing else"
    );
}

/// Die Zellen einer Zeile.
fn row(result: &v1::DiscoverResult) -> Vec<String> {
    vec![
        result.host.clone(),
        result.port.to_string(),
        product(result),
        result.latency_ms.to_string(),
        models(&result.models),
    ]
}

/// Was der Server ist, in einem Wort.
fn product(result: &v1::DiscoverResult) -> String {
    let name = match v1::LlmProduct::try_from(result.product) {
        Ok(v1::LlmProduct::Ollama) => "ollama",
        Ok(v1::LlmProduct::OpenaiCompatible) => "openai-compatible",
        _ => "unknown",
    };
    if result.auth_required {
        format!("{name} (auth required)")
    } else {
        name.to_owned()
    }
}

/// Die Modelle, gekürzt auf das, was in eine Zeile passt.
fn models(models: &[String]) -> String {
    if models.is_empty() {
        return "-".to_owned();
    }
    if models.len() <= MODELS_IN_LINE {
        return models.join(", ");
    }
    format!(
        "{}, +{} more",
        models[..MODELS_IN_LINE].join(", "),
        models.len() - MODELS_IN_LINE
    )
}

/// Ein Server als JSON-Wert, vollständig und ungekürzt.
fn server_json(result: &v1::DiscoverResult) -> Value {
    json!({
        "host": result.host,
        "port": result.port,
        "product": product(result),
        "models": result.models,
        "latency_ms": result.latency_ms,
        "auth_required": result.auth_required,
        "endpoint": format!("http://{}:{}", result.host, result.port),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{models, product, server_json};
    use humanitl_ipc::v1;

    fn result() -> v1::DiscoverResult {
        v1::DiscoverResult {
            host: "192.168.2.37".to_owned(),
            port: 11434,
            product: v1::LlmProduct::Ollama as i32,
            models: vec!["qwen2.5-coder:14b".to_owned()],
            latency_ms: 12,
            auth_required: false,
        }
    }

    #[test]
    fn a_server_behind_a_login_says_so_in_the_product_column() {
        let mut needs_auth = result();
        needs_auth.product = v1::LlmProduct::OpenaiCompatible as i32;
        needs_auth.auth_required = true;
        needs_auth.models.clear();

        assert_eq!(product(&needs_auth), "openai-compatible (auth required)");
        assert_eq!(
            models(&needs_auth.models),
            "-",
            "nobody asked it for models, so the column stays empty"
        );
    }

    /// Die Zeile bleibt eine Zeile, auch wenn ein Server dreißig Modelle
    /// nennt; die vollständige Liste steht in `--json`.
    #[test]
    fn a_long_model_list_is_counted_in_the_line_and_complete_in_json() {
        let mut many = result();
        many.models = (0..30).map(|index| format!("model-{index}")).collect();

        let line = models(&many.models);
        assert_eq!(line, "model-0, model-1, model-2, +27 more");

        let value = server_json(&many);
        assert_eq!(value["models"].as_array().unwrap().len(), 30);
        assert_eq!(value["endpoint"], "http://192.168.2.37:11434");
    }

    #[test]
    fn an_unknown_product_is_named_unknown_and_not_guessed() {
        let mut odd = result();
        odd.product = v1::LlmProduct::Unspecified as i32;
        assert_eq!(product(&odd), "unknown");
        odd.product = 999;
        assert_eq!(product(&odd), "unknown");
    }
}
