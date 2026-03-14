use anyhow::Result;

pub fn internal_api_base_url() -> String {
    let bind_addr = std::env::var("AGENTARK_BIND").unwrap_or_else(|_| "127.0.0.1:8990".to_string());
    let tls_enabled = std::env::var("AGENTARK_TLS_CERT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .is_some()
        && std::env::var("AGENTARK_TLS_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .is_some();
    let scheme = if tls_enabled { "https" } else { "http" };
    format!("{}://{}", scheme, bind_addr)
}

pub fn build_internal_control_client(timeout_secs: u64) -> Result<reqwest::Client> {
    let mut builder =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(timeout_secs.max(1)));

    if let Some(cert_path) = std::env::var("AGENTARK_TLS_CERT")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        let cert_bytes = std::fs::read(&cert_path)
            .map_err(|e| anyhow::anyhow!("Failed to read TLS cert '{}': {}", cert_path, e))?;
        let cert = reqwest::Certificate::from_pem(&cert_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse TLS cert '{}': {}", cert_path, e))?;
        builder = builder.add_root_certificate(cert);
    }

    Ok(builder.build()?)
}
