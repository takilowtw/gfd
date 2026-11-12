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

/// Like `agent()` but without the overall timeout, for model downloads that take minutes.
pub fn download_agent() -> &'static Agent {
    static AGENT: OnceLock<Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        Agent::config_builder()
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
            .timeout_connect(Some(Duration::from_secs(15)))
            .build()
            .new_agent()
    })
}

/// Translators offered in settings: id and English label.
pub const TRANSLATORS: &[(&str, &str)] = &[("google", "Google (free)"), ("deepl", "DeepL (API key)"), ("mymemory", "MyMemory (free)")];

/// Translates `text` into `target` with the chosen translator. `source` is a language code or "auto".
/// Google's free endpoints rate-limit some networks (HTTP 429), so for Google several are tried in turn.
pub fn translate(text: &str, source: &str, target: &str, translator: &str, deepl_key: &str) -> Result<String, String> {
    let attempt = |name: &str, r: Result<String, String>| match r {
        Ok(t) if !t.trim().is_empty() => Ok(t.trim().to_string()),
        Ok(_) => Err(format!("{name}: empty")),
        Err(e) => Err(format!("{name}: {e}")),
    };
    match translator {
        "deepl" => attempt("DeepL", deepl(text, source, target, deepl_key)),
        "mymemory" => attempt("MyMemory", mymemory(text, source, target)),
        _ => {
            let mut errors = Vec::new();
            for (name, f) in [
                ("Google", google_chrome as fn(&str, &str, &str) -> Result<String, String>),
                ("Google gtx", google_gtx),
                ("MyMemory", mymemory),
            ] {
                match attempt(name, f(text, source, target)) {
                    Ok(t) => return Ok(t),
                    Err(e) => errors.push(e),
                }
            }
            Err(errors.join("; "))
        }
    }
}

/// DeepL API. Free-plan keys end in ":fx" and use a separate host.
fn deepl(text: &str, source: &str, target: &str, key: &str) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("add your API key in settings".into());
    }
    let host = if key.ends_with(":fx") { "api-free.deepl.com" } else { "api.deepl.com" };
    let base = |c: &str| c.split('-').next().unwrap_or(c).to_ascii_uppercase();
    // DeepL wants a regional variant for some target languages.
    let target = match base(target).as_str() {
        "EN" => "EN-US".to_string(),
        "PT" => "PT-BR".to_string(),
        "ZH" => "ZH-HANS".to_string(),
        other => other.to_string(),
    };
    let mut body = serde_json::json!({ "text": [text], "target_lang": target });
    if source != "auto" {
        body["source_lang"] = base(source).into();
    }
    let mut resp = agent()
        .post(format!("https://{host}/v2/translate"))
        .header("Authorization", format!("DeepL-Auth-Key {key}"))
        .send_json(&body)
        .map_err(|e| match e {
            ureq::Error::StatusCode(403) => "invalid API key".to_string(),
            ureq::Error::StatusCode(456) => "monthly limit reached".to_string(),
            e => e.to_string(),
        })?;
    let json: serde_json::Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    json.pointer("/translations/0/text")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "unexpected response".into())
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
