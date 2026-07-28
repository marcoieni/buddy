mod cli;
mod credentials;
mod datadog;
mod fastly;
mod guest;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}
