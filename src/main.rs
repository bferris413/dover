use clap::{Parser, Subcommand};
use std::fs;
use syn::{File, Item, ItemFn, ItemUse};

#[derive(Parser)]
#[command(author, version, about = "Diff OVERview")]
struct Cli {
    #[arg(long)]
    file: String,
}

fn main() {
    let args = Cli::parse();
    let overview = get_overview(&args.file);
    dbg!(overview);
}

/// Get an overview of a given Rust file.
fn get_overview(path: &str) -> Overview {
    let contents = fs::read_to_string(path).expect("Something went wrong reading the file");
    let file: File = syn::parse_file(&contents).expect("Couldn't parse file {path}");

    let mut imports = Vec::new();
    let mut functions = Vec::new();

    for item in file.items {
        match item {
            Item::Use(item_use @ ItemUse { .. }) => {
                imports.push(item_use);
            }
            Item::Fn(item_fn @ ItemFn { .. }) => {
                functions.push(item_fn);
            }
            _ => {}
        }
    }

    let overview = Overview { imports, functions };

    overview
}

#[derive(Debug)]
struct Overview {
    imports: Vec<ItemUse>,
    functions: Vec<ItemFn>,
}
