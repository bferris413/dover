-- TODO -------------------------------------------------------------------------
- Summary stats at top
- Compare between two branches.
- Figure out sensible defaults for add/remove files
- Refactor old/new pattern to use single underlying type
- impl item - const
- impl item - macro
- impl item - type
- attributes - fn
- attributes - enum
- attributes - struct
- attributes - trait
- attributes - module
- probably need to add attributes for all items
- From<ImplItemFn> for Function - tricky, we're lossy converting ImplItemFn to ItemFn
- "fragment" formatter
- remove irrelevant details from impls (like unmodified functions)
- remove irrelevant details from structs
- remove irrelevant details from enums
- introduce .toml config for user config output
- remove { .. } from Function impl items
- enum variant diffs are too coarse (e.g. stuff like Fields aren't stored)

-- BUG --------------------------------------------------------------------------
- struct fields assume order matters, but this is only relevant for tuple structs

-- DONE ------------------------------------------------------------------------
- done - colored output - term
- done - Identify freestanding function signature pieces
- done - struct impl - fn
- done - trait impl - fn
- done - enum impl - fn
- done - use proc-macro2 for the initial parse (this gives us line info that syn::parse doesn't give)
- done - Compare between two commits.
- done - Freestanding functions
- done - traits
- done - enum declaration
- done - Generics in structs
  * done - type constraints
  * Defaults (?)
- done - Generics in structs (lifetime params)
- done - Visualization (structs)
- Generics in structs (type params)
  * done - Inline types
  * done - Where clauses
- done - struct field

- done - Tuple structs
  * vis
  * fields
  * field vis
- done - Identify struct pieces
  * vis
  * fields
    * named
    * unnamed (tuples)
- done - Identify relevant bits of imports that we want to diff.
  * done - ItemUse.tree
  * done - Condense to single logical import per line
  * done - A single logical import is like syn::Thing and syn::other::Thing.
  * done - Remove duplicates
- done - Create Import struct to capture only these bits.
- done - Compare Imports between two files.
- done - Compare Imports between working tree and HEAD
