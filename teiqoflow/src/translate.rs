use std::sync::OnceLock;
use std::time::Duration;
use ureq::Agent;
use ureq::tls::{TlsConfig, TlsProvider};

pub fn agent() -> &'static Agent {
    static AGENT: OnceLock<Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        Agent::config_builder()
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
            .timeout_connect(Some(Duration::from_secs(10)))
            .build()
            .new_agent()
    })
}

/// Translates with Google Translate's free web endpoint.
pub fn translate(text: &str, target: &str) -> Result<String, String> {
    let mut resp = agent()
        .get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", target)
        .query("dt", "t")
        .query("q", text)
        .call()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
    let parts = json.get(0).and_then(|v| v.as_array()).ok_or("Unexpected translator response")?;
    let out: String = parts.iter().filter_map(|p| p.get(0).and_then(|s| s.as_str())).collect();
    if out.trim().is_empty() {
        return Err("Empty translation".into());
    }
    Ok(out.trim().to_string())
}
