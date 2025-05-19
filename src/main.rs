use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dover::{Diff, GitChange, Overview, Treeish};

#[derive(Debug, Parser)]
#[command(author, version, about = "Diff OVERview")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Diff two commits (emulates `git diff [SHA-1 [SHA-1]]`)
    Diff {
        /// a test thing
        commit1: Option<String>,
        commit2: Option<String>,
    },
    /// Diff two files
    Files { file1: PathBuf, file2: PathBuf },
    // Overview {
    //     files: Vec<PathBuf>,
    // },
}

fn main() -> Result<()> {
    let args = Cli::parse();
    match args.command {
        Command::Diff { commit1, commit2 } => run_diff(Command::Diff { commit1, commit2 }),
        Command::Files { file1, file2 } => run_files(Command::Files { file1, file2 }),
        // Command::Overview { files } => run_overview(Command::Overview { files }),
    }
}

fn run_diff(command: Command) -> Result<()> {
    let Command::Diff { commit1, commit2 } = command else {
        unreachable!();
    };

    let trees = commit1.map(|c1| {
        let treeish = Treeish::new(c1, commit2);
        treeish
    });

    let repo_changes = dover::get_changed_files(PathBuf::from("."), trees)?;

    let changes = repo_changes
        .changed_files
        .into_iter()
        .filter(|c| c.path.extension().map_or(false, |ext| ext == "rs"));

    for changed_file in changes {
        let path = changed_file.path;
        match changed_file.change_type {
            GitChange::Modified {
                before_contents,
                after_contents,
            } => {
                let overview1 = Overview::try_from((path.clone(), before_contents))
                    .context("Error getting overview")?;
                let overview2 =
                    Overview::try_from((path, after_contents)).context("Error getting overview")?;
                println!("{}", overview1.diff_with(&overview2));
            }
            GitChange::Added { contents } => {
                let overview1 = Overview::try_from((path.clone(), "".to_string()))
                    .context("Error getting overview")?;
                let overview2 =
                    Overview::try_from((path, contents)).context("Error getting overview")?;
                println!("{}", overview1.diff_with(&overview2));
            }
            GitChange::Deleted { contents } => {
                let overview1 = Overview::try_from((path.clone(), contents))
                    .context("Error getting overview")?;
                let overview2 =
                    Overview::try_from((path, "".to_string())).context("Error getting overview")?;
                println!("{}", overview1.diff_with(&overview2));
            }
        }
    }

    Ok(())
}

fn run_files(c: Command) -> Result<()> {
    let Command::Files { file1, file2 } = c else {
        unreachable!();
    };

    let overview1 = Overview::try_from(file1).context("Error getting overview for file1")?;
    let overview2 = Overview::try_from(file2).context("Error getting overview for file2")?;
    println!("{}", overview1.diff_with(&overview2));

    Ok(())
}

#[allow(unused)]
fn run_overview(c: Command) -> Result<()> {
    // let Command::Overview { files } = c else {
    //     unreachable!();
    // };

    // for file in files {
    //     let overview = Overview::try_from(file).context("Error getting overview")?;
    //     println!("{overview}");
    // }

    // Ok(())
    todo!()
}
