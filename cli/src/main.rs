//! `crownplugin`: create, check, pack and manage CrownOS themes and plugins.

mod check;
mod client;
mod scaffold;
mod skill;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use crownplugin_model::archive::{self, EXTENSION};
use crownplugin_model::{Package, SettingValue};
use crownplugin_proto::{Decision, Kind, Source};

use client::Daemon;
use scaffold::Template;

#[derive(Parser)]
#[command(name = "crownplugin", version, about = "Create, check and manage CrownOS themes and plugins")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start a new theme or plugin from a working template.
    New {
        #[arg(value_enum)]
        kind: Template,
        /// Reverse-DNS id, e.g. dev.alice.cpu-graph.
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Check a package folder; exits non-zero on any error.
    Validate { dir: PathBuf },
    /// Pack a folder into a .crownpkg.
    Pack {
        dir: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Install from a folder, a .crownpkg or a git URL.
    Install {
        source: String,
        /// Branch or tag, for git URLs.
        #[arg(long)]
        rev: Option<String>,
        /// Turn it on right away (activates a theme, enables a plugin).
        #[arg(long)]
        enable: bool,
    },
    Uninstall { id: String },
    /// Everything installed, with its state.
    List,
    Enable { id: String },
    Disable { id: String },
    /// Activate a theme, or `none` for the stock look.
    Theme { id: String },
    /// Run a folder in place, reloading on every save and streaming its logs.
    Dev { dir: PathBuf },
    Logs {
        id: String,
        #[arg(short, long)]
        follow: bool,
    },
    /// Read or change a plugin setting.
    Set { id: String, key: String, value: Option<String> },
    /// Record a decision for a capability: allow, deny or ask.
    Grant { id: String, capability: String, decision: String },
    /// Install the authoring skill for coding agents, or print the Luau API types.
    Skill {
        #[arg(long)]
        types: bool,
    },
}

fn parse_value(text: &str) -> SettingValue {
    match text {
        "true" => SettingValue::Bool(true),
        "false" => SettingValue::Bool(false),
        _ => text
            .parse::<f64>()
            .map_or_else(|_| SettingValue::Text(text.to_owned()), SettingValue::Number),
    }
}

fn source(text: &str, rev: Option<String>) -> Result<Source> {
    if text.starts_with("https://") || text.starts_with("git@") {
        return Ok(Source::Git { url: text.to_owned(), rev });
    }
    let path = Path::new(text).canonicalize().with_context(|| format!("{text} does not exist"))?;
    Ok(Source::Path(path.to_string_lossy().into_owned()))
}

fn print_findings(findings: &check::Findings) {
    for warning in &findings.warnings {
        eprintln!("warning: {warning}");
    }
    for error in &findings.errors {
        eprintln!("error: {error}");
    }
}

fn activate(daemon: &mut Daemon, id: &str, kind: Kind) -> Result<()> {
    match kind {
        Kind::Theme => {
            let pending = daemon.client().set_theme(Some(id.to_owned()));
            daemon.wait(pending)
        }
        Kind::Plugin => {
            let pending = daemon.client().set_enabled(id.to_owned(), true);
            daemon.wait(pending)
        }
    }
}

fn kind_of(daemon: &mut Daemon, id: &str) -> Result<Kind> {
    daemon
        .list()?
        .into_iter()
        .find(|entity| entity.id == id)
        .map(|entity| entity.kind)
        .with_context(|| format!("`{id}` is not installed"))
}

fn run(command: Command) -> Result<ExitCode> {
    match command {
        Command::New { kind, id, name, dir } => {
            let dir = scaffold::create(kind, &id, name.as_deref(), dir.as_deref())?;
            println!("created {}", dir.display());
            println!("next: crownplugin dev {}", dir.display());
        }
        Command::Validate { dir } => {
            let findings = check::check(&dir)?;
            print_findings(&findings);
            if !findings.errors.is_empty() {
                return Ok(ExitCode::FAILURE);
            }
            println!("ok");
        }
        Command::Pack { dir, out } => {
            let findings = check::check(&dir)?;
            print_findings(&findings);
            if !findings.errors.is_empty() {
                bail!("fix the errors above before packing");
            }
            let package = Package::load(&dir)?;
            let out = out.unwrap_or_else(|| {
                PathBuf::from(format!("{}-{}.{EXTENSION}", package.id(), package.manifest.version()))
            });
            archive::pack(&package, &out)?;
            println!("wrote {}", out.display());
        }
        Command::Install { source: text, rev, enable } => {
            let mut daemon = Daemon::connect()?;
            let pending = daemon.client().install(source(&text, rev)?);
            let id = daemon.wait(pending)?;
            if enable {
                let kind = kind_of(&mut daemon, &id)?;
                activate(&mut daemon, &id, kind)?;
            }
            println!("installed {id}");
        }
        Command::Uninstall { id } => {
            let mut daemon = Daemon::connect()?;
            let pending = daemon.client().uninstall(id);
            daemon.wait(pending)?;
        }
        Command::List => {
            let mut daemon = Daemon::connect()?;
            for entity in daemon.list()? {
                println!(
                    "{} {:<32} {:<8} {:<10} {:?}",
                    if entity.active { "●" } else { "○" },
                    entity.id,
                    format!("{:?}", entity.kind).to_lowercase(),
                    entity.version,
                    entity.status
                );
            }
        }
        Command::Enable { id } => {
            let mut daemon = Daemon::connect()?;
            let kind = kind_of(&mut daemon, &id)?;
            activate(&mut daemon, &id, kind)?;
        }
        Command::Disable { id } => {
            let mut daemon = Daemon::connect()?;
            let pending = daemon.client().set_enabled(id, false);
            daemon.wait(pending)?;
        }
        Command::Theme { id } => {
            let mut daemon = Daemon::connect()?;
            let pending = daemon.client().set_theme((id != "none").then_some(id));
            daemon.wait(pending)?;
        }
        Command::Dev { dir } => {
            let findings = check::check(&dir)?;
            print_findings(&findings);
            let mut daemon = Daemon::connect()?;
            let path = dir.canonicalize()?.to_string_lossy().into_owned();
            let pending = daemon.client().dev_load(path);
            let id = daemon.wait(pending)?;
            let kind = kind_of(&mut daemon, &id)?;
            activate(&mut daemon, &id, kind)?;
            println!("running {id} from {}; edits reload it. Ctrl-C to stop.", dir.display());
            let pending = daemon.client().logs(id.clone());
            for line in daemon.wait(pending)? {
                client::print_line(&line);
            }
            daemon.follow_logs(&id)?;
        }
        Command::Logs { id, follow } => {
            let mut daemon = Daemon::connect()?;
            let pending = daemon.client().logs(id.clone());
            for line in daemon.wait(pending)? {
                client::print_line(&line);
            }
            if follow {
                daemon.follow_logs(&id)?;
            }
        }
        Command::Set { id, key, value } => {
            let mut daemon = Daemon::connect()?;
            match value {
                Some(value) => {
                    let pending = daemon.client().set_setting(id, key, parse_value(&value));
                    daemon.wait(pending)?;
                }
                None => {
                    let pending = daemon.client().settings(id);
                    let values = daemon.wait(pending)?;
                    let value = values
                        .into_iter()
                        .find(|(name, _)| *name == key)
                        .map(|(_, value)| value)
                        .with_context(|| format!("no setting `{key}`"))?;
                    println!("{value:?}");
                }
            }
        }
        Command::Grant { id, capability, decision } => {
            let decision = match decision.as_str() {
                "allow" => Some(Decision::Granted),
                "deny" => Some(Decision::Denied),
                "ask" => None,
                other => bail!("`{other}`: use allow, deny or ask"),
            };
            let mut daemon = Daemon::connect()?;
            let pending = daemon.client().set_grant(id, capability, decision);
            daemon.wait(pending)?;
        }
        Command::Skill { types } => {
            if types {
                print!("{}", skill::types());
            } else {
                for dir in skill::install()? {
                    println!("installed {}", dir.display());
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
