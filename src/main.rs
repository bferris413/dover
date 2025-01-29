use clap::Parser;

#[derive(Parser)]
#[command(author, version, about = "Diff OVERview")]
struct Cli {
    #[arg(long)]
    file: String,
}

fn main() {
    let args = Cli::parse();
    let overview = dover::get_overview(&args.file).expect("Error getting overview");
    println!("{overview}");
}
