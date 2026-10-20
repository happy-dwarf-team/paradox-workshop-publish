//! `pdx-ws`: publish Paradox Interactive game mods to the Steam Workshop.
//!
//! This is the CLI skeleton from step 1 of the implementation sequence in
//! `docs/spec/v1.md` (section 14). Every command is declared but exits with
//! code 2 until its own step implements it.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use pdx_ws::exit;

#[derive(Debug, Parser)]
#[command(name = "pdx-ws", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the publish plan; sends nothing to Steam.
    Plan,
    /// Execute the publish plan in the chosen mode.
    Publish,
    /// Create the Workshop item in `client` mode and write its ID back.
    Init,
    /// Manage the Steam session used by `steamcmd` mode.
    #[command(subcommand)]
    Session(SessionCommand),
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    /// Log in to steamcmd interactively and print the Steam session to stdout.
    Export,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Command::Plan => "plan",
            Command::Publish => "publish",
            Command::Init => "init",
            Command::Session(SessionCommand::Export) => "session export",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    eprintln!("pdx-ws {}: not implemented yet", cli.command.name());
    // Unimplemented commands use the configuration/validation exit code.
    ExitCode::from(exit::VALIDATION)
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
