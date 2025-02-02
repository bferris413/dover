use std::path::PathBuf;

use clap::Parser;
use dover::Diff;

#[derive(Parser)]
#[command(author, version, about = "Diff OVERview")]
struct Cli {
    #[arg(long)]
    file1: PathBuf,

    #[arg(long)]
    file2: PathBuf,
}

fn main() {
    let args = Cli::parse();

    let overview1 = dover::get_overview(args.file1).expect("Error getting overview for {file1}");
    let overview2 = dover::get_overview(args.file2).expect("Error getting overview for {file2}");

    println!("{overview1}");
    println!("{overview2}");

    println!("{}", overview1.diff_with(&overview2));
}
