-- TODO -------------------------------------------------------------------------
- Summary stats at top
- Compare between two branches.
- Figure out sensible defaults for add/remove files
- Refactor old/new pattern to use single underlying type
- impl item - const
- impl item - macro
- impl item - type
- trait item - const
- trait item - macro
- trait item - type
- attributes - fn
- attributes - enum
- attributes - struct
- attributes - trait
- attributes - module
- From<ImplItemFn> for Function - tricky, we're lossy converting ImplItemFn to ItemFn
- From<TraitItemFn> for Function - tricky, we're lossy converting TraitItemFn to ItemFn
- introduce .toml config for user config output
- associate impl blocks with their respective enum/struct if within same file
- HTML output doesn't do spacing between members correctly (should mimic stdout)
- We need to pull files from git instead of the filesystem, I think this existed previously but got obliterated somehow.
- Function diffs: old doesn't have doc comment, new does (but it's not marked as new)
- html - diff items (function signatures) have a leading space (?)
- html - added functions (at least standalone and impl) don't have spaces between
- html - horiz width should have a default min with horiz scroll on overflow (per column overflow)
- html - filenames are printed even if there's no diff

-- BUG --------------------------------------------------------------------------

-- DONE ------------------------------------------------------------------------
- done - Code should be escaped before returning HTML output
- done - enum variants don't elide whitespace/irrelevant members like other 'container' types
- done - struct fields assume order matters, but this is only relevant for tuple structs
- done - remove irrelevant details from structs
- done - remove irrelevant details from traits
- done - remove irrelevant details from enums
- done - remove irrelevant details from impls
- done - "collection" diffs need an ellipsis or something to indicate some irrelevant output was omitted
- done - "collection" diffs include whitespace from the original, but we need to collapse it in some instances
- done - remove irrelevant details from impls (like unmodified functions)
- done - remove irrelevant details from enums
- done - remove { .. } from Function impl items
- done - "fragment" formatter (we just use the original source now, no formatting)
- done - trait diffs are too coarse
- done - enum variant diffs are too coarse (e.g. stuff like Fields aren't stored)
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
