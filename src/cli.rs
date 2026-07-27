use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{datadog, fastly};

#[derive(Debug, Parser)]
#[command(about = "Manage Buddy VM cloud credentials")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Configure and verify the Datadog credentials in a VM.
    LoginDatadog {
        /// Lima VM name.
        vm: String,
    },
    /// Configure and verify the Fastly credentials in a VM.
    LoginFastly {
        /// Lima VM name.
        vm: String,
    },
    /// Dump or assert the Datadog permissions snapshot.
    DatadogPermissions {
        #[command(subcommand)]
        action: DatadogPermissionsAction,
    },
}

#[derive(Debug, Subcommand)]
enum DatadogPermissionsAction {
    /// Write the VM's current Datadog permissions to a snapshot.
    Dump {
        /// Lima VM name.
        vm: String,
    },
    /// Compare the VM's current Datadog permissions with a snapshot.
    Assert {
        /// Lima VM name.
        vm: String,
    },
}

pub(crate) fn run() -> Result<()> {
    match Cli::parse().command {
        Commands::LoginDatadog { vm } => datadog::login(&vm),
        Commands::LoginFastly { vm } => fastly::login(&vm),
        Commands::DatadogPermissions { action } => match action {
            DatadogPermissionsAction::Dump { vm } => datadog::dump_permissions(&vm),
            DatadogPermissionsAction::Assert { vm } => datadog::assert_permissions(&vm),
        },
    }
}
