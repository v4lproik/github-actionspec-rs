use clap::Parser;
use github_actionspec_rs::cli::{Cli, Command};
use github_actionspec_rs::discovery::discover_declarations;
use github_actionspec_rs::validate::{validate_contract, validate_repo_workflow, ValidateContractOptions, ValidateRepoWorkflowOptions};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), github_actionspec_rs::errors::AppError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Validate {
            schema,
            contract,
            actual,
        } => {
            validate_contract(ValidateContractOptions {
                schema_paths: schema,
                contract_path: contract,
                actual_path: actual,
                cwd: None,
                env: None,
            })?;
        }
        Command::Discover {
            repo,
            declarations_dir,
        } => {
            let declarations = discover_declarations(&repo, &declarations_dir)?;
            println!("{}", serde_json::to_string_pretty(&declarations)?);
        }
        Command::ValidateRepo {
            repo,
            workflow,
            actual,
            declarations_dir,
        } => {
            validate_repo_workflow(ValidateRepoWorkflowOptions {
                repo_root: repo,
                workflow,
                actual_path: actual,
                declarations_dir,
                cwd: None,
                env: None,
            })?;
        }
    }

    Ok(())
}
