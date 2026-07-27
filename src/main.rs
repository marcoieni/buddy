mod cli;
mod credentials;
mod datadog;
mod fastly;
mod guest;
mod snapshot;

fn main() -> anyhow::Result<()> {
    cli::run()
}
