use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, crate_version};
use dover::{Config, Diff, GitChange, HTML_BOILERPLATE, HTML_EPILOGUE, Overview, Treeish};
use editor_command::EditorBuilder;

#[derive(Debug, Parser)]
#[command(author, version = crate_version!(), about = "Diff OVERview - summarize git diffs of Rust code")]
struct Cli {
    #[arg(long, global = true, default_value_t = false)]
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
    /// Diff revisions or the working tree (emulates `git diff [REVISION [REVISION]]`)
    Diff {
        revision1: Option<String>,
        revision2: Option<String>,
    },
    /// Diff two files
    Files { file1: PathBuf, file2: PathBuf },
    /// Manage Dover's configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    // Overview {
    //     files: Vec<PathBuf>,
    // },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Create a config file containing all available settings
    Init,
    /// Open the config file in the configured editor
    Edit,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let output = OutputFormat::new(args.to_html);

    match args.command {
        Command::Diff {
            revision1,
            revision2,
        } => {
            let config = Config::load()?;
            run_diff(
                Command::Diff {
                    revision1,
                    revision2,
                },
                output,
                &config,
            )
        }
        Command::Files { file1, file2 } => {
            let config = Config::load()?;
            run_files(Command::Files { file1, file2 }, output, &config)
        }
        Command::Config {
            command: ConfigCommand::Init,
        } => {
            let path = Config::init()?;
            println!("Created config at {}", path.display());
            Ok(())
        }
        Command::Config {
            command: ConfigCommand::Edit,
        } => {
            let path = Config::ensure_exists()?;
            open_in_editor(&path)
        }
    }
}

fn open_in_editor(path: &std::path::Path) -> Result<()> {
    let editor = EditorBuilder::new()
        .string(git_editor_command())
        .string(non_empty_env("VISUAL"))
        .string(non_empty_env("EDITOR"))
        .string(Some(platform_default_editor()))
        .build()
        .context("Error parsing the configured editor command")?;

    let status = editor
        .open(path)
        .status()
        .with_context(|| format!("Error opening config at {}", path.display()))?;
    if !status.success() {
        bail!("Editor exited with {status}");
    }
    Ok(())
}

fn git_editor_command() -> Option<String> {
    non_empty_env("GIT_EDITOR").or_else(|| {
        let output = ProcessCommand::new("git")
            .args(["config", "--get", "core.editor"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let editor = String::from_utf8(output.stdout).ok()?;
        let editor = editor.trim();
        (!editor.is_empty()).then(|| editor.to_owned())
    })
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[cfg(target_os = "windows")]
fn platform_default_editor() -> &'static str {
    "notepad.exe"
}

#[cfg(not(target_os = "windows"))]
fn platform_default_editor() -> &'static str {
    "vi"
}

fn run_diff(command: Command, output: OutputFormat, config: &Config) -> Result<()> {
    let Command::Diff {
        revision1,
        revision2,
    } = command
    else {
        unreachable!();
    };

    let trees = revision1.map(|revision1| Treeish::new(revision1, revision2));

    let repo_changes = dover::get_changed_files(PathBuf::from("."), trees)?;

    let changes = repo_changes
        .changed_files
        .into_iter()
        .filter(|c| c.path.extension().is_some_and(|ext| ext == "rs"));

    let mut rendered = match output {
        OutputFormat::Html => HTML_BOILERPLATE.to_string(),
        OutputFormat::Plain => String::new(),
    };

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

                let overview_diff = overview1.diff_with(&overview2);
                if overview_diff.has_visible_changes(config) {
                    append_diff(&mut rendered, &overview_diff, output, config);
                }
            }
            GitChange::Added { contents } => {
                let overview1 = Overview::try_from((path.clone(), "".to_string()))
                    .context("Error getting overview")?;
                let overview2 =
                    Overview::try_from((path, contents)).context("Error getting overview")?;

                let overview_diff = overview1.diff_with(&overview2);
                if overview_diff.has_visible_changes(config) {
                    append_diff(&mut rendered, &overview_diff, output, config);
                }
            }
            GitChange::Deleted { contents } => {
                let overview1 = Overview::try_from((path.clone(), contents))
                    .context("Error getting overview")?;
                let overview2 =
                    Overview::try_from((path, "".to_string())).context("Error getting overview")?;

                let overview_diff = overview1.diff_with(&overview2);
                if overview_diff.has_visible_changes(config) {
                    append_diff(&mut rendered, &overview_diff, output, config);
                }
            }
        }
    }

    if let OutputFormat::Html = output {
        rendered.push_str(HTML_EPILOGUE);
    }
    write_output(&rendered, matches!(output, OutputFormat::Plain))?;

    Ok(())
}

fn run_files(c: Command, output: OutputFormat, config: &Config) -> Result<()> {
    let Command::Files { file1, file2 } = c else {
        unreachable!();
    };

    let overview1 = Overview::try_from(file1).context("Error getting overview for file1")?;
    let overview2 = Overview::try_from(file2).context("Error getting overview for file2")?;

    let file_diff = overview1.diff_with(&overview2);

    let rendered = match output {
        OutputFormat::Plain => file_diff.to_terminal_with_config(config),
        OutputFormat::Html => {
            let mut html = HTML_BOILERPLATE.to_string();
            html.push_str(&file_diff.to_html_with_config(config));
            html.push_str(HTML_EPILOGUE);
            html
        }
    };
    write_output(&rendered, matches!(output, OutputFormat::Plain))?;

    Ok(())
}

fn append_diff(
    rendered: &mut String,
    diff: &dover::OverviewDiff,
    output: OutputFormat,
    config: &Config,
) {
    if !rendered.is_empty() && matches!(output, OutputFormat::Plain) {
        rendered.push_str("\n\n");
    }
    match output {
        OutputFormat::Plain => rendered.push_str(&diff.to_terminal_with_config(config)),
        OutputFormat::Html => rendered.push_str(&diff.to_html_with_config(config)),
    }
}

fn write_output(output: &str, page_plain_output: bool) -> Result<()> {
    if output.is_empty() {
        return Ok(());
    }

    if page_plain_output && io::stdout().is_terminal() && write_to_pager(output)? {
        return Ok(());
    }

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{output}").context("Error writing output")
}

fn write_to_pager(output: &str) -> Result<bool> {
    let pager = env::var("PAGER").ok();
    let pager = pager.as_deref().map(str::trim).unwrap_or("less");
    if pager.is_empty() {
        return Ok(false);
    }

    let mut words = pager.split_whitespace();
    let Some(program) = words.next() else {
        return Ok(false);
    };
    let mut command = ProcessCommand::new(program);
    command.args(words);
    if PathBuf::from(program)
        .file_name()
        .is_some_and(|name| name == "less")
    {
        command.args(["-F", "-R", "-X"]);
    }

    let mut child = match command.stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(_) => return Ok(false),
    };

    let mut pager_write_error = None;
    if let Some(mut stdin) = child.stdin.take() {
        let write_result = stdin
            .write_all(output.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"));
        if let Err(error) = write_result
            && error.kind() != io::ErrorKind::BrokenPipe
        {
            pager_write_error = Some(error);
        }
    }
    child.wait().context("Error waiting for pager")?;
    if let Some(error) = pager_write_error {
        return Err(error).context("Error writing to pager");
    }
    Ok(true)
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
