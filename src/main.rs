//! `pdx-ws`: publish Paradox Interactive game mods to the Steam Workshop.
//!
//! `plan` and `publish --mode steamcmd` are implemented; the other commands
//! are declared and exit with code 2 until their step of the implementation
//! sequence in `docs/spec/v1.md` (section 14) implements them.

use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use pdx_ws::change_note::ChangeNoteInput;
use pdx_ws::config::{self, Config};
use pdx_ws::copy::PublishedCopy;
use pdx_ws::exit;
use pdx_ws::plan::Plan;
use pdx_ws::steamcmd::session::Credentials;
use pdx_ws::steamcmd::{self, PublishOptions, Steamcmd};

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
    Publish(PublishArgs),
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

#[derive(Debug, Args)]
struct PublishArgs {
    #[command(flatten)]
    selection: Selection,
    /// How the plan reaches Steam.
    #[arg(long, value_enum, default_value_t = Mode::Steamcmd)]
    mode: Mode,
    /// The change note, in Markdown. Default: the changelog section for the
    /// descriptor's version, else `Version <version>`.
    #[arg(long, value_name = "TEXT")]
    change_note: Option<String>,
    /// Let `steamcmd` mode create a missing item (without tags).
    #[arg(long)]
    allow_create: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// steamcmd with a Steam session from the environment.
    Steamcmd,
    /// The logged-in Steam client.
    Client,
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
            Command::Publish(_) => "publish",
            Command::Init => "init",
            Command::Session(SessionCommand::Export) => "session export",
        }
    }
}

/// A failed command: what to print and the exit code.
struct Failure {
    label: Option<String>,
    message: String,
    details: Vec<String>,
    code: u8,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.label {
            Some(label) => write!(f, "error: {label}: {}", self.message)?,
            None => write!(f, "error: {}", self.message)?,
        }
        for detail in &self.details {
            write!(f, "\n  {detail}")?;
        }
        Ok(())
    }
}

impl From<pdx_ws::Error> for Failure {
    fn from(error: pdx_ws::Error) -> Self {
        Failure {
            label: None,
            message: error.to_string(),
            details: Vec::new(),
            code: error.exit_code(),
        }
    }
}

impl Failure {
    fn steamcmd(label: Option<String>, error: &steamcmd::Error) -> Self {
        Failure {
            label,
            message: error.to_string(),
            details: error.details().to_vec(),
            code: error.exit_code(),
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Plan(args) => plan(args),
        Command::Publish(args) => publish(args),
        command => {
            eprintln!("pdx-ws {}: not implemented yet", command.name());
            // Unimplemented commands use the configuration/validation exit code.
            return ExitCode::from(exit::VALIDATION);
        }
    };
    match result {
        Ok(()) => ExitCode::from(exit::SUCCESS),
        Err(failure) => {
            eprintln!("{failure}");
            ExitCode::from(failure.code)
        }
    }
}

fn plan(args: &PlanArgs) -> Result<(), Failure> {
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

/// Publishes the selected mods one after another and stops at the first
/// failure.
fn publish(args: &PublishArgs) -> Result<(), Failure> {
    if args.mode == Mode::Client {
        return Err(Failure {
            label: None,
            message: "client mode is not implemented yet".to_owned(),
            details: Vec::new(),
            code: exit::VALIDATION,
        });
    }
    let config = Config::load(&args.selection.config)?;
    let change_note = ChangeNoteInput::from_env(args.change_note.clone());
    let plan = Plan::build_with(&config, &args.selection.mods, &change_note)?;

    // Everything that can be checked before installing anything.
    for module in &plan.mods {
        if module.item.id.is_none() && !args.allow_create {
            return Err(Failure::steamcmd(
                Some(module.label()),
                &steamcmd::Error::NoItemId,
            ));
        }
        for warning in &module.warnings {
            eprintln!("warning: {}: {}", module.label(), warning.message);
        }
    }
    let credentials = Credentials::from_env()
        .map_err(|error| Failure::steamcmd(None, &steamcmd::Error::Session(error)))?;

    let mut log = |line: &str| eprintln!("steamcmd | {line}");
    let steamcmd = Steamcmd::install(&mut log).map_err(|error| Failure::steamcmd(None, &error))?;
    let options = PublishOptions {
        allow_create: args.allow_create,
        ..PublishOptions::default()
    };
    for module in &plan.mods {
        let label = module.label();
        let fail = |error: steamcmd::Error| Failure::steamcmd(Some(label.clone()), &error);
        let copy = PublishedCopy::create(module).map_err(|error| fail(error.into()))?;
        let published = steamcmd
            .publish(&credentials, module, copy.path(), &options, &mut log)
            .map_err(fail)?;
        println!(
            "{label}: published to Workshop item {} ({})",
            published.item_id,
            published.item_id.url()
        );
        if published.created {
            for write_back in &module.write_backs {
                println!(
                    "{label}: created the item; set {} to {} in {} and commit it",
                    write_back.key, published.item_id, write_back.file
                );
            }
        }
    }
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
