//! `pdx-ws`: publish Paradox Interactive game mods to the Steam Workshop.
//!
//! `plan` is implemented; the other commands are declared and exit with
//! code 2 until their step of the implementation sequence in
//! `docs/spec/v1.md` (section 14) implements them.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use pdx_ws::change_note::ChangeNoteInput;
use pdx_ws::config::{self, Config};
use pdx_ws::exit;
use pdx_ws::plan::Plan;

#[derive(Debug, Parser)]
#[command(name = "pdx-ws", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the publish plan; sends nothing to Steam.
    Plan(PlanArgs),
    /// Execute the publish plan in the chosen mode.
    Publish,
    /// Create the Workshop item in `client` mode and write its ID back.
    Init,
    /// Manage the Steam session used by `steamcmd` mode.
    #[command(subcommand)]
    Session(SessionCommand),
}

#[derive(Debug, Args)]
struct PlanArgs {
    #[command(flatten)]
    selection: Selection,
    /// The change note, in Markdown. Default: the changelog section for the
    /// descriptor's version, else `Version <version>`.
    #[arg(long, value_name = "TEXT")]
    change_note: Option<String>,
    /// Print the plan as JSON instead of text.
    #[arg(long)]
    json: bool,
}

/// Which config file and which of its mods.
#[derive(Debug, Args)]
struct Selection {
    /// The config file.
    #[arg(long, value_name = "PATH", default_value = config::FILE_NAME)]
    config: PathBuf,
    /// Only the mod with this `id`; repeatable. Default: every mod.
    #[arg(long = "mod", value_name = "ID")]
    mods: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    /// Log in to steamcmd interactively and print the Steam session to stdout.
    Export,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Command::Plan(_) => "plan",
            Command::Publish => "publish",
            Command::Init => "init",
            Command::Session(SessionCommand::Export) => "session export",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Plan(args) => match plan(&args) {
            Ok(()) => ExitCode::from(exit::SUCCESS),
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(error.exit_code())
            }
        },
        command => {
            eprintln!("pdx-ws {}: not implemented yet", command.name());
            // Unimplemented commands use the configuration/validation exit code.
            ExitCode::from(exit::VALIDATION)
        }
    }
}

fn plan(args: &PlanArgs) -> Result<(), pdx_ws::Error> {
    let config = Config::load(&args.selection.config)?;
    let change_note = ChangeNoteInput::from_env(args.change_note.clone());
    let plan = Plan::build_with(&config, &args.selection.mods, &change_note)?;
    let mut stdout = io::stdout().lock();
    // A closed stdout (e.g. `| head`) is not a plan error.
    let _ = if args.json {
        serde_json::to_writer_pretty(&mut stdout, &plan)
            .map_err(io::Error::from)
            .and_then(|()| writeln!(stdout))
    } else {
        write!(stdout, "{plan}")
    };
    Ok(())
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
