-- TODO -------------------------------------------------------------------------
- Summary stats at top
- Compare between two branches.
- Figure out sensible defaults for add/remove files
- Identify freestanding function signature pieces
- Refactor old/new pattern to use single underlying type
- struct impl - fn
- trait impl - fn
- enum impl - fn
- "fragment" formatter
- probably need to add attributes for all items
- colored output
- impl item - const
- impl item - macro
- impl item - type
- attributes - fn
- attributes - enum
- attributes - struct
- attributes - trait
- attributes - module
- From<ImplItemFn> for Function - tricky, we're lossy converting ImplItemFn to ItemFn

-- BUG --------------------------------------------------------------------------
- struct fields assume order matters, but this is only relevant for tuple structs

-- DONE ------------------------------------------------------------------------
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
