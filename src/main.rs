//! `pdx-ws`: publish Paradox Interactive game mods to the Steam Workshop.
//!
//! Commands: `plan`, `publish` in `steamcmd` or `client` mode, `init`,
//! `session export`, and the hidden `github-report` that the GitHub Action
//! runs (`docs/spec/v1.md`, section 3).

use std::fmt;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use pdx_ws::change_note::ChangeNoteInput;
use pdx_ws::client::steam::SteamClient;
use pdx_ws::client::{self, drift};
use pdx_ws::config::{self, Config};
use pdx_ws::exit;
use pdx_ws::github;
use pdx_ws::plan::Plan;
use pdx_ws::publish::{self, Outcome};
use pdx_ws::steamcmd::export;
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
    Init(InitArgs),
    /// Manage the Steam session used by `steamcmd` mode.
    #[command(subcommand)]
    Session(SessionCommand),
    /// Internal to the GitHub Action: write the job summary, annotations
    /// and outputs from the JSON of `plan` and `publish`.
    #[command(hide = true)]
    GithubReport(GithubReportArgs),
}

#[derive(Debug, Args)]
struct GithubReportArgs {
    /// The output of `plan --json`.
    #[arg(long, value_name = "PATH")]
    plan: PathBuf,
    /// The output of `publish --json`; empty or missing when `publish`
    /// stopped before publishing.
    #[arg(long, value_name = "PATH")]
    results: Option<PathBuf>,
    /// `workshop.toml` as the workflow named it, relative to the repository
    /// root.
    #[arg(long, value_name = "PATH", default_value = config::FILE_NAME)]
    config: String,
    /// Nothing was sent to Steam.
    #[arg(long)]
    dry_run: bool,
    /// Why the run is a dry run when the workflow did not ask for one.
    #[arg(long, value_name = "TEXT")]
    dry_run_reason: Option<String>,
    /// The exit code of `publish`.
    #[arg(long, value_name = "CODE")]
    publish_exit: Option<u8>,
    /// Also write the summary to this file.
    #[arg(long, value_name = "PATH")]
    summary_copy: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct InitArgs {
    #[command(flatten)]
    selection: Selection,
    /// The change note, in Markdown. Default: the changelog section for the
    /// descriptor's version, else `Version <version>`.
    #[arg(long, value_name = "TEXT")]
    change_note: Option<String>,
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
    /// Stop at the first mod that fails; by default the others are still
    /// published.
    #[arg(long)]
    fail_fast: bool,
    /// Print the per-mod results as JSON instead of a table.
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
    Export(ExportArgs),
}

#[derive(Debug, Args)]
struct ExportArgs {
    /// The Steam user name. Default: asked on the terminal.
    #[arg(long, value_name = "NAME")]
    user: Option<String>,
    /// Write the session even when stdout is a terminal.
    #[arg(long)]
    stdout: bool,
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

    fn client(label: Option<String>, error: &client::Error) -> Self {
        Failure {
            label,
            message: error.to_string(),
            details: Vec::new(),
            code: error.exit_code(),
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Plan(args) => plan(args),
        Command::Publish(args) => publish(args),
        Command::Init(args) => init(args),
        Command::Session(SessionCommand::Export(args)) => session_export(args),
        Command::GithubReport(args) => github_report(args),
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

/// Publishes the selected mods one after another, then prints one result
/// per mod. A failed mod does not stop the others unless `--fail-fast` is
/// given; problems found before any upload stop the whole run.
fn publish(args: &PublishArgs) -> Result<(), Failure> {
    let config = Config::load(&args.selection.config)?;
    let change_note = ChangeNoteInput::from_env(args.change_note.clone());
    let plan = Plan::build_with(&config, &args.selection.mods, &change_note)?;
    if args.mode == Mode::Client {
        return publish_client(args, &plan);
    }

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
    let report = publish::run(&plan, args.fail_fast, |module| {
        eprintln!("Publishing {}", module.label());
        let result = publish::through_steamcmd(&steamcmd, &credentials, module, &options, &mut log);
        if let Err(error) = &result {
            eprintln!("{}", Failure::steamcmd(Some(module.label()), error));
        }
        result
    });
    finish(&report, args.json)
}

/// `publish --mode client`: every selected mod must have an item already
/// (`pdx-ws init` creates them), and all must be of one game.
fn publish_client(args: &PublishArgs, plan: &Plan) -> Result<(), Failure> {
    if args.allow_create {
        return Err(Failure {
            label: None,
            message: "--allow-create is for steamcmd mode; client mode creates items with \
                      `pdx-ws init`"
                .to_owned(),
            details: Vec::new(),
            code: exit::VALIDATION,
        });
    }
    for module in &plan.mods {
        if module.item.id.is_none() {
            return Err(Failure::client(
                Some(module.label()),
                &client::Error::NoItemId,
            ));
        }
        for warning in &module.warnings {
            eprintln!("warning: {}: {}", module.label(), warning.message);
        }
    }
    let mut steam = connect(plan)?;
    let report = publish::run(plan, args.fail_fast, |module| {
        eprintln!("Publishing {}", module.label());
        let result = client::publish(&mut steam, module, &mut |event| {
            eprintln!("steam | {event}");
        });
        if let Err(error) = &result {
            eprintln!("{}", Failure::client(Some(module.label()), error));
        }
        result
    });
    finish(&report, args.json)
}

/// `pdx-ws init`: creates the Workshop item of every selected mod that has
/// none yet, in `client` mode, and writes each ID back for the modder to
/// commit.
fn init(args: &InitArgs) -> Result<(), Failure> {
    let config = Config::load(&args.selection.config)?;
    let change_note = ChangeNoteInput::from_env(args.change_note.clone());
    let mut plan = Plan::build_with(&config, &args.selection.mods, &change_note)?;
    plan.mods.retain(|module| match module.item.id {
        Some(item_id) => {
            eprintln!(
                "{}: already has Workshop item {item_id}; nothing to create",
                module.label()
            );
            false
        }
        None => true,
    });
    if plan.mods.is_empty() {
        return Ok(());
    }
    for module in &plan.mods {
        for warning in &module.warnings {
            eprintln!("warning: {}: {}", module.label(), warning.message);
        }
    }
    let mut steam = connect(&plan)?;
    for module in &plan.mods {
        let label = module.label();
        eprintln!("Creating the Workshop item of {label}");
        let created = client::init(&mut steam, &config, module, &change_note, &mut |event| {
            eprintln!("steam | {event}");
        })
        .map_err(|error| Failure::client(Some(label.clone()), &error))?;
        println!(
            "{label}: created Workshop item {} ({}); commit {}",
            created.item_id,
            created.item_id.url(),
            created.write_back
        );
    }
    Ok(())
}

/// Connects to Steam as the selected mods' game and reports checksum drift.
fn connect(plan: &Plan) -> Result<SteamClient, Failure> {
    let game = client::single_game(plan)
        .map_err(|error| Failure::client(None, &error))?
        .ok_or_else(|| Failure {
            label: None,
            message: "no mod is selected".to_owned(),
            details: Vec::new(),
            code: exit::VALIDATION,
        })?;
    let steam = SteamClient::connect(game).map_err(|error| Failure::client(None, &error))?;
    for module in &plan.mods {
        for warning in drift::cross_check(&steam, module) {
            eprintln!("warning: {}: {warning}", module.label());
        }
    }
    Ok(steam)
}

/// Prints the per-mod results and turns them into the exit code.
fn finish(report: &publish::Report, json: bool) -> Result<(), Failure> {
    let mut stdout = io::stdout().lock();
    // A closed stdout (e.g. `| head`) does not change the exit code.
    let _ = if json {
        serde_json::to_writer_pretty(&mut stdout, report)
            .map_err(io::Error::from)
            .and_then(|()| writeln!(stdout))
    } else {
        write!(stdout, "{report}")
    };
    match report.exit_code() {
        exit::SUCCESS => Ok(()),
        code => Err(Failure {
            label: None,
            message: format!(
                "not every mod was published ({} failed, {} skipped)",
                report.count(Outcome::Failed),
                report.count(Outcome::Skipped)
            ),
            details: Vec::new(),
            code,
        }),
    }
}

/// `pdx-ws session export`: the session goes to stdout only, everything
/// else to stderr.
fn session_export(args: &ExportArgs) -> Result<(), Failure> {
    let usage = |message: String| Failure {
        label: None,
        message,
        details: Vec::new(),
        code: exit::VALIDATION,
    };
    if !export::may_write_session(io::stdout().is_terminal(), args.stdout) {
        let user = args.user.as_deref().unwrap_or("<name>");
        return Err(usage(format!(
            "stdout is a terminal, so the Steam session would be shown on screen; pipe it into \
             a secret instead, or pass --stdout\n\n{}",
            export::guidance(user)
        )));
    }
    let user = match &args.user {
        Some(user) => user.trim().to_owned(),
        None if io::stdin().is_terminal() => {
            eprint!("Steam user name: ");
            let mut line = String::new();
            io::stdin()
                .lock()
                .read_line(&mut line)
                .map_err(|error| usage(format!("cannot read the user name: {error}")))?;
            line.trim().to_owned()
        }
        None => String::new(),
    };
    if user.is_empty() {
        return Err(usage("pass the Steam user name with --user".to_owned()));
    }
    let dir = export::data_dir().ok_or_else(|| {
        usage(
            "cannot find the per-user data folder (LOCALAPPDATA, XDG_DATA_HOME or HOME)".to_owned(),
        )
    })?;

    eprintln!(
        "pdx-ws keeps steamcmd in {}. Log in as {user} with your password and Steam Guard code \
         when steamcmd asks.",
        dir.display()
    );
    let mut log = |line: &str| eprintln!("steamcmd | {line}");
    let steamcmd =
        Steamcmd::install_in(&dir, &mut log).map_err(|error| Failure::steamcmd(None, &error))?;
    let session = steamcmd
        .export_session(&user, &mut log)
        .map_err(|error| Failure::steamcmd(None, &error))?;

    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{session}")
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure {
            label: None,
            message: format!("cannot write the Steam session to stdout: {error}"),
            details: Vec::new(),
            code: exit::PUBLISH_FAILED,
        })?;
    eprintln!(
        "\nThe Steam session for {user} went to stdout; the local copy is deleted.\n\n{}",
        export::guidance(&user)
    );
    Ok(())
}

/// `pdx-ws github-report`, run by the GitHub Action after `plan` and
/// `publish`: the job summary goes to `GITHUB_STEP_SUMMARY`, the outputs to
/// `GITHUB_OUTPUT` and the annotations to stdout.
fn github_report(args: &GithubReportArgs) -> Result<(), Failure> {
    let failure = |message: String| Failure {
        label: None,
        message,
        details: Vec::new(),
        code: exit::VALIDATION,
    };
    let read = |path: &std::path::Path| {
        fs::read_to_string(path)
            .map_err(|error| failure(format!("cannot read {}: {error}", path.display())))
    };
    let plan: github::PlanView = serde_json::from_str(&read(&args.plan)?)
        .map_err(|error| failure(format!("{} is not a plan: {error}", args.plan.display())))?;
    let results = match &args.results {
        Some(path) if path.is_file() => {
            let text = read(path)?;
            if text.trim().is_empty() {
                None
            } else {
                Some(
                    serde_json::from_str::<github::ResultsView>(&text).map_err(|error| {
                        failure(format!(
                            "{} is not a publish result: {error}",
                            path.display()
                        ))
                    })?,
                )
            }
        }
        _ => None,
    };
    let run = github::Run {
        dry_run: args.dry_run,
        dry_run_reason: args.dry_run_reason.clone(),
        publish_exit: args.publish_exit,
        config: args.config.clone(),
    };

    let summary = github::summary(&plan, results.as_ref(), &run);
    let append = |variable: &str, text: &str| -> Result<(), Failure> {
        let Some(path) = std::env::var_os(variable) else {
            return Ok(());
        };
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(|error| failure(format!("cannot write {variable}: {error}")))
    };
    append("GITHUB_STEP_SUMMARY", &summary)?;
    if let Some(path) = &args.summary_copy {
        fs::write(path, &summary)
            .map_err(|error| failure(format!("cannot write {}: {error}", path.display())))?;
    }
    let outputs: String = github::outputs(&plan, results.as_ref())
        .iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect();
    append("GITHUB_OUTPUT", &outputs)?;
    for annotation in github::annotations(&plan, results.as_ref(), &run) {
        println!("{annotation}");
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
