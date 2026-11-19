//! The URLs of a login run (HUM-520 points 1 and 2).
//!
//! - **Shown, opened only on a click.** The daemon reads the URLs a login
//!   prints ([`login_urls`]); a client shows each, and opens one on the host
//!   only after a click or confirmation and only when its host is one of the
//!   adapter's login hosts ([`LoginUrl::openable`]). Every other URL is
//!   shown, never opened.
//! - **A pasted callback.** Where the agent waits for the browser's callback
//!   on its own loopback (Codex: `localhost:1455`), the browser on the host
//!   cannot reach the sandbox, and no port of the host is forwarded into it.
//!   The person pastes the URL from the browser's address bar; the daemon
//!   accepts exactly `http://localhost:<port><path>?<query>`
//!   ([`check_callback`]) and delivers it with one process inside the run
//!   ([`delivery_command`]). This needs a sandbox with a loopback of its own
//!   (every Linux backend); under Seatbelt the sandbox shares the host's
//!   loopback and the profile admits only the session's two ports, so only
//!   the device flow works there ([`callback_offered`]).

use humanitl_core::diagnostics::codes::AGENT_014;
use humanitl_core::{Diagnostic, Severity};

use super::recipe::{LoginCallback, LoginRecipe};

/// The longest URL the daemon reads from the output or takes pasted.
pub const URL_CAP: usize = 4096;

/// A URL a login printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginUrl {
    /// The URL, as printed.
    pub url: String,
    /// Whether a client may open it on the host after a click: HTTPS to one
    /// of the adapter's login hosts, no user part, the default port.
    pub openable: bool,
}

/// The URLs in `text`, each once, in order, at most 16. Escape sequences of
/// the terminal are skipped; a URL ends at a space, a quote, an angle
/// bracket or a control character.
#[must_use]
pub fn login_urls(text: &str, login: &LoginRecipe) -> Vec<LoginUrl> {
    let plain = strip_escapes(text);
    let mut found: Vec<LoginUrl> = Vec::new();
    let mut rest = plain.as_str();
    while let Some(at) = rest.find("http") {
        let tail = &rest[at..];
        let end = tail
            .find(|c: char| {
                c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '`')
            })
            .unwrap_or(tail.len());
        let candidate = tail[..end].trim_end_matches(['.', ',', ')', ']', ';']);
        if (candidate.starts_with("https://") || candidate.starts_with("http://"))
            && candidate.len() <= URL_CAP
            && !found.iter().any(|seen| seen.url == candidate)
        {
            found.push(LoginUrl {
                url: candidate.to_owned(),
                openable: openable(candidate, login),
            });
            if found.len() == 16 {
                break;
            }
        }
        rest = &tail[end.max(4)..];
    }
    found
}

/// Whether `url` may be opened on the host: HTTPS, one of the login hosts
/// exactly, no user part, no other port.
#[must_use]
pub fn openable(url: &str, login: &LoginRecipe) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.port().is_none()
        && parsed
            .host_str()
            .is_some_and(|host| login.hosts.iter().any(|known| known.host == host))
}

/// Whether the client offers the field to paste a callback URL: the adapter
/// has a callback, and the backend gives the sandbox a loopback of its own.
#[must_use]
pub fn callback_offered(login: Option<&LoginRecipe>, own_loopback: bool) -> bool {
    own_loopback && login.is_some_and(|login| login.callback.is_some())
}

/// Checks a pasted callback URL: exactly `http://localhost:<port><path>?`
/// followed by a query of URL characters, no fragment. Answers the URL to
/// call inside the sandbox, on `127.0.0.1`.
///
/// # Errors
///
/// `AGENT_014` naming why the URL is not the callback: another scheme, host,
/// port or path, no query, a fragment, a character outside a query, or
/// longer than [`URL_CAP`].
pub fn check_callback(pasted: &str, callback: &LoginCallback) -> Result<String, Diagnostic> {
    callback_inside(pasted, callback).map_err(|why| {
        Diagnostic::builder(AGENT_014, Severity::Error)
            .why(format!("the pasted URL was not delivered: {why}"))
            .build()
    })
}

fn callback_inside(pasted: &str, callback: &LoginCallback) -> Result<String, String> {
    let pasted = pasted.trim();
    if pasted.len() > URL_CAP {
        return Err(format!("the URL is longer than {URL_CAP} characters"));
    }
    let prefix = format!("http://localhost:{}{}?", callback.port, callback.path);
    let Some(query) = pasted.strip_prefix(&prefix) else {
        return Err(format!(
            "the URL is not the agent's callback; it must begin with {prefix}"
        ));
    };
    if query.is_empty() {
        return Err("the URL carries no query".to_owned());
    }
    if let Some(bad) = query.chars().find(|c| !query_char(*c)) {
        return Err(format!(
            "the query holds {bad:?}, which a callback URL does not carry"
        ));
    }
    Ok(format!(
        "http://127.0.0.1:{}{}?{query}",
        callback.port, callback.path
    ))
}

/// The characters of a query (RFC 3986), without `#`.
fn query_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '-' | '.'
                | '_'
                | '~'
                | '%'
                | '!'
                | '$'
                | '&'
                | '\''
                | '('
                | ')'
                | '*'
                | '+'
                | ','
                | ';'
                | '='
                | ':'
                | '@'
                | '/'
                | '?'
        )
}

/// The process that delivers a checked callback inside the run: `curl` from
/// the sandbox's `/usr`, past the proxy to the loopback, without globbing,
/// with a deadline. Argument words, never a shell.
#[must_use]
pub fn delivery_command(inside: &str) -> Vec<String> {
    [
        "curl",
        "--silent",
        "--show-error",
        "--globoff",
        "--noproxy",
        "*",
        "--max-time",
        "10",
        "--output",
        "/dev/null",
        "--",
        inside,
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}

/// `text` without the terminal's escape sequences (CSI, OSC and two-byte
/// ones).
fn strip_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
                // An OSC ends a word: a hyperlink's text follows.
                out.push(' ');
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::agent::depot::InstallHost;

    const LOGIN: LoginRecipe = LoginRecipe {
        args: &["login"],
        hosts: &[InstallHost {
            host: "login.example.invalid",
            evidence: "https://example.invalid/ (stand-in)",
        }],
        callback: Some(LoginCallback {
            port: 1455,
            path: "/auth/callback",
        }),
        named: None,
        device_flow_setting: Some("Device code login"),
    };

    #[test]
    fn urls_of_the_login_hosts_are_openable_and_others_only_shown() {
        let text = "Open \u{1b}[1mhttps://login.example.invalid/oauth?code=1&x=2\u{1b}[0m in a \
                    browser.\nOr https://evil.example.invalid/phish, or \
                    https://user@login.example.invalid/x and https://login.example.invalid:8443/x \
                    and http://login.example.invalid/plain.";
        let urls = login_urls(text, &LOGIN);
        let shown: Vec<(&str, bool)> = urls.iter().map(|u| (u.url.as_str(), u.openable)).collect();
        assert_eq!(
            shown,
            vec![
                ("https://login.example.invalid/oauth?code=1&x=2", true),
                ("https://evil.example.invalid/phish", false),
                ("https://user@login.example.invalid/x", false),
                ("https://login.example.invalid:8443/x", false),
                ("http://login.example.invalid/plain", false),
            ]
        );
    }

    #[test]
    fn exactly_the_callback_is_delivered() {
        let inside = check_callback(
            "http://localhost:1455/auth/callback?code=abc%2F1&state=xyz",
            &LOGIN.callback.unwrap(),
        )
        .unwrap();
        assert_eq!(
            inside,
            "http://127.0.0.1:1455/auth/callback?code=abc%2F1&state=xyz"
        );
        assert_eq!(delivery_command(&inside).last().unwrap(), &inside);
    }

    #[test]
    fn another_host_port_or_path_is_refused() {
        let callback = LOGIN.callback.unwrap();
        for url in [
            "http://127.0.0.1:1455/auth/callback?code=1",
            "http://localhost.evil.invalid:1455/auth/callback?code=1",
            "http://evil.invalid/?http://localhost:1455/auth/callback?code=1",
            "https://localhost:1455/auth/callback?code=1",
            "http://localhost:1456/auth/callback?code=1",
            "http://localhost:1455/auth/other?code=1",
            "http://localhost:1455/auth/callbackx?code=1",
            "http://localhost:1455/auth/callback",
            "http://localhost:1455/auth/callback?",
            "http://localhost:1455/auth/callback?code=1#frag",
            "http://localhost:1455/auth/callback?code=1 2",
            "http://user@localhost:1455/auth/callback?code=1",
        ] {
            assert!(check_callback(url, &callback).is_err(), "{url}");
        }
        let long = format!(
            "http://localhost:1455/auth/callback?code={}",
            "a".repeat(URL_CAP)
        );
        assert!(check_callback(&long, &callback).is_err());
    }

    /// Under Seatbelt the sandbox shares the host's loopback: no field to
    /// paste, and a failed login names the device flow setting.
    #[test]
    fn without_an_own_loopback_there_is_no_field_to_paste() {
        assert!(callback_offered(Some(&LOGIN), true));
        assert!(!callback_offered(Some(&LOGIN), false));
        let without = LoginRecipe {
            callback: None,
            ..LOGIN
        };
        assert!(!callback_offered(Some(&without), true));
        assert!(!callback_offered(None, true));
    }
}
