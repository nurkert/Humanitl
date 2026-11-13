//! Ein Abbild als Wurzel der Sandbox (HUM-439, HUM-292 Abschnitt 7).
//!
//! **Kein Start zieht.** Die Zeile trägt `--pull=never`, und vor jedem Start
//! fragt der Plan Podman mit `podman image inspect <name>@sha256:<digest>`,
//! ob genau dieses Abbild im lokalen Speicher liegt ([`verdict`]). Fehlt es
//! oder trägt das Abbild unter dem Namen einen anderen Digest, lehnt der
//! Start mit `SANDBOX_043` ab, und die Abhilfe ist der Befehl, der zieht
//! ([`pull_command_text`]). Gestartet wird danach mit dem Verweis samt Digest;
//! Podman löst ihn über den Digest auf, also über den Inhalt, nicht über
//! einen Namen, unter dem morgen ein anderes Abbild liegen könnte.
//!
//! **Ziehen nur auf Wunsch.** `humanitl sandbox image pull` (RPC
//! `SandboxRequest.image_pull`) ist der einzige Weg, auf dem der Adapter ein
//! Abbild holt ([`pull_args`]). Podman holt es dann selbst über das Netz des
//! Daemons, direkt bei der Registry, die der Name nennt: nicht durch den
//! Proxy von Humanitl, ohne die Proxy-Variablen des Hosts und ohne die
//! Spiegel und Suchlisten aus der `registries.conf` des Nutzers. Die
//! Signaturregel ist eine eigene Datei des Adapters
//! (`--signature-policy`), die jedes Abbild annimmt: Was gilt, ist der Digest,
//! den der Mensch genannt hat ([`SIGNATURE_POLICY`]). Ein kurzer Name ohne
//! Registry (`debian@sha256:...`) wird vorher abgelehnt, denn welche Registry
//! ihn bediente, stünde sonst in einer Datei, die der Daemon nicht schreibt.
//!
//! **Was das Abbild nicht bestimmen darf.** Die Umgebung eines Abbilds legt
//! Podman über die der `containers.conf` (gemessen im Code von Podman,
//! `pkg/specgen/generate/container.go`: erst die Vorgaben, dann das Abbild).
//! Dort stehen die Variablen des Shims; ein Abbild, das eine Variable mit
//! `HUMANITL_` setzt, könnte sie ersetzen und wird abgelehnt. Nutzer,
//! Einstiegspunkt, Volumes und Gesundheitsprüfung des Abbilds hebt die Zeile
//! selbst auf ([`super::plan::line`]).

use std::io;
use std::process::Output;
use std::time::Duration;

use humanitl_core::diagnostics::codes::{CONFIG_003, SANDBOX_043};
use humanitl_core::{Diagnostic, FixAction, Severity};

/// So lange darf ein Ziehen dauern, bevor der Daemon es abbricht.
pub(super) const PULL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Die Signaturregel des Adapters für das ausdrückliche Ziehen: jedes
/// Abbild. Die Vertrauensaussage trägt der Digest, nicht eine Signatur.
pub(super) const SIGNATURE_POLICY: &str = "{\"default\":[{\"type\":\"insecureAcceptAnything\"}]}\n";

/// Der Anfang jeder Variable, die der Shim liest oder die der Plan setzt.
const RESERVED_ENV_PREFIX: &str = "HUMANITL_";

/// Höchstens so viel der Fehlerausgabe von Podman steht in einem Befund.
const DETAIL_CHARS: usize = 400;

/// Was ein Abbild im lokalen Speicher ist, nachdem [`verdict`] es geprüft
/// hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Found {
    /// Die Kennung des Abbilds (`Id`).
    pub(super) id: String,
    /// Der Digest, den der Verweis nennt, als `sha256:<hex>`.
    pub(super) digest: String,
}

/// Name und Digest (`sha256:<hex>`) eines Verweises `name@sha256:<hex>`.
pub(super) fn split(reference: &str) -> Option<(&str, String)> {
    let (name, hex) = reference.rsplit_once("@sha256:")?;
    Some((name, format!("sha256:{hex}")))
}

/// Die Abhilfe für ein fehlendes Abbild, als Befehl zum Kopieren.
pub(super) fn pull_command_text(reference: &str) -> String {
    format!("humanitl sandbox image pull {reference}")
}

/// `SANDBOX_043`: Das Abbild liegt nicht mit diesem Digest vor. `present`
/// ist der Digest, der unter dem Namen liegt, wenn es einen gibt.
pub(super) fn missing(reference: &str, present: Option<&str>, detail: &str) -> Diagnostic {
    let name = split(reference).map_or(reference, |(name, _)| name);
    let mut why = match present {
        Some(digest) => format!(
            "the image {reference} is not in the local storage of the container engine; under \
             the name {name} it holds {digest}, another image than the one pinned"
        ),
        None => {
            format!("the image {reference} is not in the local storage of the container engine")
        }
    };
    why.push_str(
        ". A start never pulls an image; pulling is a step of its own that uses the network \
         directly, not through the proxy",
    );
    let detail = detail.trim();
    if !detail.is_empty() {
        why.push_str(" (");
        why.extend(detail.chars().take(DETAIL_CHARS));
        why.push(')');
    }
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(why)
        .fix(FixAction::CopyCommand(pull_command_text(reference)))
        .build()
}

/// `SANDBOX_043`: Das Abbild liegt vor, taugt aber nicht als Wurzel.
fn unusable(reference: &str, why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(format!(
            "the image {reference} cannot be the root of the sandbox: {why}"
        ))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.podman.rootfs".to_owned(),
            value: "host".to_owned(),
        })
        .build()
}

/// Das Urteil über die Antwort von `podman image inspect <reference>`: das
/// Abbild mit genau diesem Digest, oder `SANDBOX_043`.
///
/// Gezählt wird der Digest, den Podman für das Abbild selbst nennt (`Digest`
/// und `RepoDigests`), nicht der Name: Podman sucht einen Verweis mit Digest
/// zwar über den Digest, aber ein Urteil, das sich darauf verließe, hinge an
/// einer Fassung der Engine.
///
/// # Errors
///
/// `SANDBOX_043`, wenn Podman nicht antwortet, das Abbild nicht kennt,
/// es einen anderen Digest trägt oder seine Umgebung eine Variable des Shims
/// setzt.
pub(super) fn verdict(reference: &str, answer: &io::Result<Output>) -> Result<Found, Diagnostic> {
    let output = match answer {
        Ok(output) => output,
        Err(err) => return Err(missing(reference, None, &err.to_string())),
    };
    if !output.status.success() {
        return Err(missing(
            reference,
            None,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }
    let Some((_, digest)) = split(reference) else {
        return Err(missing(
            reference,
            None,
            "the reference names no sha256 digest",
        ));
    };
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| missing(reference, None, &format!("unreadable answer: {err}")))?;
    let Some(image) = parsed.as_array().and_then(|list| list.first()) else {
        return Err(missing(reference, None, "the answer names no image"));
    };
    let text = |key: &str| image.get(key).and_then(serde_json::Value::as_str);
    let own = text("Digest");
    let repo_digests: Vec<&str> = image
        .get("RepoDigests")
        .and_then(serde_json::Value::as_array)
        .map(|list| list.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    let matches = own == Some(digest.as_str())
        || repo_digests
            .iter()
            .any(|entry| entry.rsplit_once('@').is_some_and(|(_, d)| d == digest));
    if !matches {
        let present = own.or_else(|| {
            repo_digests
                .first()
                .and_then(|entry| entry.rsplit_once('@').map(|(_, d)| d))
        });
        return Err(missing(reference, present, ""));
    }
    let env: Vec<&str> = image
        .get("Config")
        .and_then(|config| config.get("Env"))
        .and_then(serde_json::Value::as_array)
        .map(|list| list.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    if let Some(reserved) = env
        .iter()
        .map(|pair| pair.split_once('=').map_or(*pair, |(key, _)| key))
        .find(|key| key.starts_with(RESERVED_ENV_PREFIX))
    {
        return Err(unusable(
            reference,
            &format!(
                "its environment sets {reserved}, a name the shim of the sandbox reads, and the \
                 container engine would let the image's value win over the daemon's"
            ),
        ));
    }
    Ok(Found {
        id: text("Id").unwrap_or_default().to_owned(),
        digest,
    })
}

/// Der Digest, den ein Abbild unter dem Namen trägt, aus der Antwort von
/// `podman image inspect <name>`; `None`, wenn es keines gibt.
pub(super) fn present_digest(answer: &io::Result<Output>) -> Option<String> {
    let output = answer.as_ref().ok().filter(|out| out.status.success())?;
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    parsed
        .as_array()?
        .first()?
        .get("Digest")?
        .as_str()
        .filter(|digest| !digest.is_empty())
        .map(str::to_owned)
}

/// Prüft den Verweis eines ausdrücklichen Ziehens: mit Digest und mit der
/// Registry im Namen.
///
/// # Errors
///
/// `CONFIG_003`, wenn der Digest fehlt oder der Name keine Registry nennt.
pub(super) fn check_pull_reference(reference: &str) -> Result<(), Diagnostic> {
    if !humanitl_config::image_pinned_by_digest(reference) {
        return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!(
                "{reference:?} is not pinned by digest; an image is pulled only as \
                 NAME@sha256: and 64 hex digits, so that the name cannot stand for another \
                 image tomorrow"
            ))
            .fix(FixAction::CopyCommand(pull_command_text(
                "registry.example/name@sha256:<64 hex digits>",
            )))
            .build());
    }
    let name = split(reference).map_or(reference, |(name, _)| name);
    if !humanitl_config::image_names_registry(reference) {
        return Err(Diagnostic::builder(CONFIG_003, Severity::Error)
            .why(format!(
                "{reference:?} names no registry; a short name would be resolved by a search \
                 list the daemon does not write. Name the registry, for example \
                 docker.io/library/{name}"
            ))
            .fix(FixAction::CopyCommand(pull_command_text(&format!(
                "docker.io/library/{reference}"
            ))))
            .build());
    }
    Ok(())
}

/// Die Argumente von `podman` nach den globalen Flags für ein Ziehen.
pub(super) fn pull_args(reference: &str, signature_policy: &std::path::Path) -> Vec<String> {
    vec![
        "pull".to_owned(),
        "--quiet".to_owned(),
        format!("--signature-policy={}", signature_policy.display()),
        "--".to_owned(),
        reference.to_owned(),
    ]
}

/// `SANDBOX_043`: Das Ziehen scheiterte.
pub(super) fn pull_failed(reference: &str, answer: &io::Result<Output>) -> Diagnostic {
    let detail = match answer {
        Ok(output) => String::from_utf8_lossy(&output.stderr).into_owned(),
        Err(err) => err.to_string(),
    };
    let mut why = format!(
        "the container engine could not pull {reference} from its registry; nothing was \
         started"
    );
    let detail = detail.trim();
    if !detail.is_empty() {
        why.push_str(" (");
        why.extend(detail.chars().take(DETAIL_CHARS));
        why.push(')');
    }
    Diagnostic::builder(SANDBOX_043, Severity::Blocking)
        .why(why)
        .fix(FixAction::CopyCommand(pull_command_text(reference)))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::process::ExitStatusExt as _;
    use std::process::ExitStatus;

    use super::*;

    const PINNED: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    const OTHER: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    fn reference() -> String {
        format!("localhost/humanitl-test@sha256:{PINNED}")
    }

    /// Die Antwort von Podman; ein negativer `code` ist ein Aufruf, der gar
    /// nicht antwortete (Frist abgelaufen), mit `stderr` als Grund.
    fn answer(code: i32, stdout: &str, stderr: &str) -> io::Result<Output> {
        if code < 0 {
            return Err(io::Error::other(stderr.to_owned()));
        }
        Ok(Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    fn inspect(digest: &str, env: &[&str]) -> String {
        serde_json::json!([{
            "Id": "f00d",
            "Digest": format!("sha256:{digest}"),
            "RepoDigests": [format!("localhost/humanitl-test@sha256:{digest}")],
            "Config": { "Env": env },
        }])
        .to_string()
    }

    /// Das Abbild mit genau dem Digest geht durch; ein anderer Digest unter
    /// demselben Namen ist `SANDBOX_043` mit dem Befehl, der zieht. Das ist
    /// die Prüfung, ohne die ein Start ein anderes Abbild nähme.
    #[test]
    fn only_the_pinned_digest_passes() {
        let found = verdict(
            &reference(),
            &answer(0, &inspect(PINNED, &["PATH=/bin"]), ""),
        )
        .expect("pinned");
        assert_eq!(found.digest, format!("sha256:{PINNED}"));
        assert_eq!(found.id, "f00d");

        let err = verdict(&reference(), &answer(0, &inspect(OTHER, &[]), ""))
            .expect_err("another digest");
        assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
        assert!(err.why.contains(&format!("sha256:{OTHER}")), "{}", err.why);
        assert!(err.why.contains("never pulls"), "{}", err.why);
        assert_eq!(
            err.fix,
            Some(FixAction::CopyCommand(format!(
                "humanitl sandbox image pull {}",
                reference()
            )))
        );
    }

    /// Ein Verweis auf eine Liste (mehrere Plattformen) steht nur in
    /// `RepoDigests`; das genügt.
    #[test]
    fn a_digest_in_repo_digests_counts() {
        let body = serde_json::json!([{
            "Id": "f00d",
            "Digest": format!("sha256:{OTHER}"),
            "RepoDigests": [
                format!("localhost/humanitl-test@sha256:{OTHER}"),
                format!("localhost/humanitl-test@sha256:{PINNED}"),
            ],
        }])
        .to_string();
        verdict(&reference(), &answer(0, &body, "")).expect("listed digest");
    }

    #[test]
    fn a_missing_image_or_a_silent_engine_is_sandbox_043() {
        for answer in [
            answer(125, "", "Error: localhost/humanitl-test: image not known"),
            answer(0, "[]", ""),
            answer(0, "not json", ""),
            answer(-1, "", "no answer within 30s"),
        ] {
            let err = verdict(&reference(), &answer).expect_err("missing");
            assert_eq!(err.code.as_str(), "SANDBOX_043", "{}", err.why);
            assert!(err.fix.is_some());
        }
        let err = verdict(&reference(), &answer(125, "", "image not known")).unwrap_err();
        assert!(err.why.contains("image not known"), "{}", err.why);
    }

    /// Die Umgebung eines Abbilds geht in Podman vor die der
    /// `containers.conf`; eine Variable des Shims darin wird abgelehnt.
    #[test]
    fn an_image_that_sets_a_shim_variable_is_refused() {
        let err = verdict(
            &reference(),
            &answer(
                0,
                &inspect(PINNED, &["PATH=/bin", "HUMANITL_BRIDGES=x"]),
                "",
            ),
        )
        .expect_err("reserved");
        assert_eq!(err.code.as_str(), "SANDBOX_043");
        assert!(err.why.contains("HUMANITL_BRIDGES"), "{}", err.why);
        verdict(
            &reference(),
            &answer(0, &inspect(PINNED, &["MY_HUMANITL_X=1"]), ""),
        )
        .expect("only the prefix counts");
    }

    #[test]
    fn the_present_digest_is_read_from_a_name_only_answer() {
        assert_eq!(
            present_digest(&answer(0, &inspect(OTHER, &[]), "")),
            Some(format!("sha256:{OTHER}"))
        );
        assert_eq!(present_digest(&answer(125, "", "image not known")), None);
        let err = missing(&reference(), Some("sha256:abc"), "");
        assert!(
            err.why
                .contains("under the name localhost/humanitl-test it holds sha256:abc")
        );
    }

    #[test]
    fn a_pull_needs_a_digest_and_a_registry() {
        check_pull_reference(&format!("docker.io/library/debian@sha256:{PINNED}"))
            .expect("qualified");
        check_pull_reference(&reference()).expect("localhost");
        check_pull_reference(&format!("registry:5000/x@sha256:{PINNED}")).expect("port");
        let err = check_pull_reference("docker.io/library/debian:trixie").expect_err("tag");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        let err = check_pull_reference(&format!("debian@sha256:{PINNED}")).expect_err("short");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("docker.io/library/debian"), "{}", err.why);
        let err = check_pull_reference(&format!("library/debian@sha256:{PINNED}"))
            .expect_err("no registry");
        assert_eq!(err.code.as_str(), "CONFIG_003");
    }

    #[test]
    fn the_pull_line_names_the_adapters_signature_policy() {
        let args = pull_args(&reference(), std::path::Path::new("/run/e/policy.json"));
        assert_eq!(
            args,
            [
                "pull".to_owned(),
                "--quiet".to_owned(),
                "--signature-policy=/run/e/policy.json".to_owned(),
                "--".to_owned(),
                reference(),
            ]
        );
        let policy: serde_json::Value = serde_json::from_str(SIGNATURE_POLICY).expect("json");
        assert_eq!(policy["default"][0]["type"], "insecureAcceptAnything");
    }
}
