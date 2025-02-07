use std::path::PathBuf;

use clap::{Parser, Subcommand};
use dover::{Diff, GitChange, Overview};

#[derive(Parser)]
#[command(author, version, about = "Diff OVERview")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Files { file1: PathBuf, file2: PathBuf },
    Overview { files: Vec<PathBuf> },
}

fn main() {
    let args = Cli::parse();
    if args.command.is_some() {
        run_command(args.command.unwrap());
        return;
    }

    let changes = dover::get_changed_files(PathBuf::from("."))
        .unwrap()
        .into_iter()
        .filter(|c| c.path.extension().map_or(false, |ext| ext == "rs"));

    for change in changes {
        let path = change.path;
        match change.change_type {
            GitChange::Modified {
                before_contents,
                after_contents,
            } => {
                let overview1 = Overview::try_from((path.clone(), before_contents))
                    .expect("Error getting overview");
                let overview2 =
                    Overview::try_from((path, after_contents)).expect("Error getting overview");
                println!("{}", overview1.diff_with(&overview2));
            }
            GitChange::Added { contents } => {
                let overview1 = Overview::try_from((path.clone(), "".to_string()))
                    .expect("Error getting overview");
                let overview2 =
                    Overview::try_from((path, contents)).expect("Error getting overview");
                println!("{}", overview1.diff_with(&overview2));
            }
            GitChange::Deleted { contents } => {
                let overview1 =
                    Overview::try_from((path.clone(), contents)).expect("Error getting overview");
                let overview2 =
                    Overview::try_from((path, "".to_string())).expect("Error getting overview");
                println!("{}", overview1.diff_with(&overview2));
            }
        }
    }
}

fn run_command(c: Command) {
    match c {
        Command::Files { file1, file2 } => {
            let overview1 = Overview::try_from(file1).expect("Error getting overview for file1");
            let overview2 = Overview::try_from(file2).expect("Error getting overview for file2");

            println!("{}", overview1.diff_with(&overview2));
        }
        Command::Overview { files } => {
            for file in files {
                let overview = Overview::try_from(file).expect("Error getting overview");
                println!("{overview}");
            }
        }
    }
}
