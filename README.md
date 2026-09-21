# dover
dover (**d**iff **over**view) is a CLI tool for summarizing git diffs of Rust code. dover diffs provide a semantic, high-level overview of the changes tracked by git.

#### Note
While usable, this project is very prototype-y, both in terms of output format and implementation. Updates are made regularly, but it's still in the early stages. See [the roadmap](#-todo) for planned features and changes.

# Installation
If you're building from source or using cargo, you'll need a recent version of Rust, which you can get [here](https://www.rust-lang.org/learn/get-started).
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
dover diff                       # index vs. working tree
dover diff main                  # main vs. index and working tree
dover diff main feature/demo     # two branches
dover diff HEAD~2 HEAD           # any tree-ish revisions
```
The arguments can be any Git revisions that resolve to trees, including branch names,
tags, commit SHAs, `HEAD`, and relative expressions such as `HEAD~2`. With one revision,
dover compares it to the current index and working tree. With two revisions, it compares
the two committed trees.

See `dover --help` for all supported commands.

# Configuration

Dover reads `dover.toml` from the platform-specific configuration directory. If the file does not
exist, all supported elements are shown by default. Create a complete config containing every
available setting with:

```sh
dover config init
```

Open the config in your preferred editor with:

```sh
dover config edit
```

The editor is selected using Git's precedence: `GIT_EDITOR`, Git's `core.editor` setting, `VISUAL`,
then `EDITOR`. Dover falls back to `vi` on macOS and Linux or Notepad on Windows. The command creates
a default config before opening it if necessary.

Each element can be shown or hidden independently:

```toml
[uses]
show = true

[structs]
show = true

[enums]
show = true

[traits]
show = true

[functions]
show = true

[impls]
show = true
```

Missing sections and missing `show` settings default to `true`. Dover reports unknown settings as
configuration errors so that misspellings do not silently change behavior.

# ✅ TODO
- [X] structs
- [X] enums
- [X] traits
  - [X] signatures
  - [X] functions
  - [ ] `const`
  - [ ] macro
  - [ ] type declarations
- [X] functions - standalone
- [ ] modules
- [X] `use` statements
- [X] `impl` blocks
  - [X] functions
  - [ ] `const`
  - [ ] macro
  - [ ] type declarations
- [ ] attributes
- [X] user config (`dover.toml`)

# FAQ
#### Q: What's the intended use case?
A: dover works nicely when you're looking for a high-level overview of the changes between two commits, or when you want to summarize a set of changes before getting into the nitty-gritty of a full diff (like when reviewing a large PR).

#### Q: Does dover replace git?
A: No, dover doesn't do any versioning or source control. It's an optional supplement to reading large git diffs.

#### Q: How does it work?
A: dover has two major dependencies: [git2](https://github.com/rust-lang/git2-rs) for reading git repositories and [syn](https://github.com/dtolnay/syn) for parsing Rust source files. For each added, modified, or removed file in a git diff, dover uses syn to parse the source and collect an opinionated subset (the "overview") of the AST. The overview ASTs are then compared and the resulting diff is printed.

#### Q: Why don't you include \<this language feature\> in diffs?
A: I probably just haven't gotten to that feature yet. As of `3397ad3a5a80c32b5ab9d29d955af5b1e77163b3`, dover supports most parts of structs, enums, traits, function signatures, `use` statements, and `impl` blocks. See [the roadmap](#-todo) for missing-but-planned items.

#### Q: Rust only?
A: Correct, no other languages are planned, for a couple reasons:
* This project is for me: my daily driver at work (and home) is Rust,
* This project is about exploring the idea of a "diff overview" rather than providing a general purpose implementation.

If you're interested in implementing the idea and want multi-language support, consider using [tree-sitter](https://github.com/tree-sitter/tree-sitter) for the parser.

#### Q: Don't semantic diffing tools already exist?
A: They sure do. While tools like [difftastic](https://github.com/Wilfred/difftastic), [SemanticDiff](https://semanticdiff.com/), and [DiffLens](https://www.difflens.com/) will provide a semantic diff of your code, dover's goal is to explore the idea of providing an opinionated, configurable _overview_ of changes so you can get a glimpse of what's coming before diving into the full diff.
