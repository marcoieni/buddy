use std::process::ExitCode;

mod cli;
mod credentials;
mod datadog;
mod fastly;
mod guest;
mod snapshot;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
