use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dover::{Diff, GitChange, HTML_BOILERPLATE, Html, Overview, Treeish};

#[derive(Debug, Parser)]
#[command(author, version, about = "Diff OVERview")]
struct Cli {
    #[arg(long, default_value_t = false)]
    to_html: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Copy, Clone)]
enum OutputFormat {
    Html,
    Plain,
}
impl OutputFormat {
    fn new(to_html: bool) -> Self {
        if to_html {
            OutputFormat::Html
        } else {
            OutputFormat::Plain
        }
    }
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
    let output = OutputFormat::new(args.to_html);

    match args.command {
        Command::Diff { commit1, commit2 } => run_diff(Command::Diff { commit1, commit2 }, output),
        Command::Files { file1, file2 } => run_files(Command::Files { file1, file2 }, output),
        // Command::Overview { files } => run_overview(Command::Overview { files }, &output),
    }
}

fn run_diff(command: Command, output: OutputFormat) -> Result<()> {
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

    let mut html = HTML_BOILERPLATE.to_string();

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

                match output {
                    OutputFormat::Plain => println!("{}", overview1.diff_with(&overview2)),
                    OutputFormat::Html => html.push_str(&overview1.diff_with(&overview2).to_html()),
                }
            }
            GitChange::Added { contents } => {
                let overview1 = Overview::try_from((path.clone(), "".to_string()))
                    .context("Error getting overview")?;
                let overview2 =
                    Overview::try_from((path, contents)).context("Error getting overview")?;

                match output {
                    OutputFormat::Plain => println!("{}", overview1.diff_with(&overview2)),
                    OutputFormat::Html => html.push_str(&overview1.diff_with(&overview2).to_html()),
                }
            }
            GitChange::Deleted { contents } => {
                let overview1 = Overview::try_from((path.clone(), contents))
                    .context("Error getting overview")?;
                let overview2 =
                    Overview::try_from((path, "".to_string())).context("Error getting overview")?;

                match output {
                    OutputFormat::Plain => println!("{}", overview1.diff_with(&overview2)),
                    OutputFormat::Html => html.push_str(&overview1.diff_with(&overview2).to_html()),
                }
            }
        }
    }

    if let OutputFormat::Html = output {
        html.push_str("</body></html>");
        println!("{}", html);
    }

    Ok(())
}

fn run_files(c: Command, output: OutputFormat) -> Result<()> {
    let Command::Files { file1, file2 } = c else {
        unreachable!();
    };

    let overview1 = Overview::try_from(file1).context("Error getting overview for file1")?;
    let overview2 = Overview::try_from(file2).context("Error getting overview for file2")?;

    let file_diff = overview1.diff_with(&overview2);
    match output {
        OutputFormat::Plain => println!("{}", file_diff),
        OutputFormat::Html => println!("{}", file_diff.to_html()),
    }

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
