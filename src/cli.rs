use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "github-actionspec")]
#[command(about = "Validate GitHub Actions workflow contracts expressed in CUE.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Validate {
        #[arg(long = "schema", required = true)]
        schema: Vec<PathBuf>,
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        actual: PathBuf,
    },
    Discover {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long = "declarations-dir", default_value = ".github/actionspec")]
        declarations_dir: PathBuf,
    },
    ValidateRepo {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        workflow: String,
        #[arg(long)]
        actual: PathBuf,
        #[arg(long = "declarations-dir", default_value = ".github/actionspec")]
        declarations_dir: PathBuf,
    },
}
