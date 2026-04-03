use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    agentark::run().await
}
