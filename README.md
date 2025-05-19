# dover
dover (**d**iff **over**view) is a CLI tool for summarizing git diffs of Rust code. dover diffs provide a semantic, high-level overview of the changes tracked by git.

#### Note
While usable, this project is very prototype-y, both in terms of output format and implementation. Updates are made on a semi-dialy basis, but it's still in the early stages. See the roadmap for planned features and changes.

# Installation
If you're building from source or using cargo, lou'll need a recent version of Rust, which you can get [here](https://www.rust-lang.org/learn/get-started).
## Using cargo
```sh
cargo install dover
```

## Building from source

1. Clone the repository
```sh
git clone git@github.com:bferris413/dover.git
```
2. Build the application
```sh
cd dover && cargo build --release
```

# Usage
The main entry point is the `diff` subcommand, meant to emulate the behavior of `git diff`:
```sh
# like git diff [c1 [c2]]
dover diff [c1 [c2]]
```
`c1` and `c2` must be valid commit SHAs, but support is planned for branch names, refnames like `HEAD`, and selections like `HEAD~2`.

# FAQ
#### Q: What's the intended use case?
A: dover works nicely when you're looking for a high-level overview of the changes between two commits, or when you want to summarize a set of changes before getting into the nitty-gritty of a full diff (like when reviewing a large PR).

#### Q: Does dover replace git?
A: No, dover doesn't do any versioning or source control. It's an optional supplement to reading large git diffs.

#### Q: How does it work?
A: dover has two major dependencies: [git2](https://github.com/rust-lang/git2-rs) for reading git repositories and [syn](https://github.com/dtolnay/syn) for parsing Rust source files. For each added, modified, or removed file in a git diff, dover uses syn to parse the source and collect an opinionated subset (the "overview") of the AST. The overview ASTs are then compared and the resulting diff is printed.

#### Q: Why don't you include \<this language feature\> in diffs?
A: I probably just haven't gotten to that feature yet. As of `3397ad3a5a80c32b5ab9d29d955af5b1e77163b3`, dover supports structs, enums, traits, function signatures, `use` statements, and `impl` blocks. See the roadmap for missing-but-planned items.

#### Q: Rust only?
A: Correct, no other languages are planned, for a couple reasons:
* This project is for me: my driver at work is Rust,
* This project is about exploring the idea of a "diff overview" rather than providing a general purpose implementation.

If you're interested in implementing the idea and want multi-language support, consider using [tree-sitter](https://github.com/tree-sitter/tree-sitter) for the parser.
