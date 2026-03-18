mod command;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use enum_as_inner::EnumAsInner;

use attic_server::config;
use command::make_token::{self, MakeToken};
use command::scrub_storage::{self, ScrubStorage};

/// Attic server administration utilities.
#[derive(Debug, Parser)]
#[clap(version, author = "Zhaofeng Li <hello@zhaofeng.li>")]
#[clap(propagate_version = true)]
pub struct Opts {
    /// Path to the config file.
    #[clap(short = 'f', long, global = true)]
    config: Option<PathBuf>,

    /// The sub-command.
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand, EnumAsInner)]
pub enum Command {
    MakeToken(Box<MakeToken>),
    ScrubStorage(ScrubStorage),
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let config = config::load_config(opts.config.as_deref(), false).await?;

    match opts.command {
        Command::MakeToken(_) => make_token::run(config, opts).await?,
        Command::ScrubStorage(_) => scrub_storage::run(config, opts).await?,
    }

    Ok(())
}
