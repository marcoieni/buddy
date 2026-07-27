mod cli;
mod credentials;
mod datadog;
mod fastly;
mod guest;

fn main() -> anyhow::Result<()> {
    cli::run()
}
