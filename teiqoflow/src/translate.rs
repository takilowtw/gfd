use std::sync::OnceLock;
use std::time::Duration;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

pub fn agent() -> &'static Agent {
    static AGENT: OnceLock<Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        Agent::config_builder()
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(20)))
            .build()
            .new_agent()
    })
}

/// Translates `text` into `target`. `source` is a language code or "auto".
/// Free endpoints rate-limit some networks (HTTP 429), so several are tried in turn.
pub fn translate(text: &str, source: &str, target: &str) -> Result<String, String> {
    let mut errors = Vec::new();
    for (name, f) in [
        ("Google", google_chrome as fn(&str, &str, &str) -> Result<String, String>),
        ("Google gtx", google_gtx),
        ("MyMemory", mymemory),
    ] {
        match f(text, source, target) {
            Ok(t) if !t.trim().is_empty() => return Ok(t.trim().to_string()),
            Ok(_) => errors.push(format!("{name}: empty")),
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    Err(errors.join("; "))
}

fn get_json(req: ureq::RequestBuilder<ureq::typestate::WithoutBody>) -> Result<serde_json::Value, String> {
    let mut resp = req.call().map_err(|e| e.to_string())?;
    resp.body_mut().read_json().map_err(|e| e.to_string())
}

/// Endpoint used by Google's Chrome dictionary extension. Response: [["translation","src"]] or ["translation"].
fn google_chrome(text: &str, source: &str, target: &str) -> Result<String, String> {
    let json = get_json(
        agent()
            .get("https://clients5.google.com/translate_a/t")
            .query("client", "dict-chrome-ex")
            .query("sl", source)
            .query("tl", target)
            .query("q", text),
    )?;
    let first = json.get(0).ok_or("unexpected response")?;
    first
        .as_str()
        .or_else(|| first.get(0).and_then(|v| v.as_str()))
        .map(str::to_string)
        .ok_or_else(|| "unexpected response".into())
}

fn google_gtx(text: &str, source: &str, target: &str) -> Result<String, String> {
    let json = get_json(
        agent()
            .get("https://translate.googleapis.com/translate_a/single")
            .query("client", "gtx")
            .query("sl", source)
            .query("tl", target)
            .query("dt", "t")
            .query("q", text),
    )?;
    let parts = json.get(0).and_then(|v| v.as_array()).ok_or("unexpected response")?;
    Ok(parts.iter().filter_map(|p| p.get(0).and_then(|s| s.as_str())).collect())
}

fn mymemory(text: &str, source: &str, target: &str) -> Result<String, String> {
    if source == "auto" {
        return Err("needs a known source language".into());
    }
    let json = get_json(
        agent()
            .get("https://api.mymemory.translated.net/get")
            .query("q", text)
            .query("langpair", format!("{source}|{target}")),
    )?;
    json.pointer("/responseData/translatedText")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "unexpected response".into())
}
