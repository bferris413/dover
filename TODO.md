- Summary stats at top
- Compare Imports between two commits.
- Compare Imports between two branches.
- Figure out sensible defaults for add/remove files
- Identify freestanding function signature pieces
  * visibility
  * args
  * const
  * name
  * return type
  * lifetime
  * generics
  * (basically all of `sig`)
- Identify trait pieces
- Refactor old/new pattern to use single underlying type
- enum declaration
- enum impl
- struct impl
--------------------------------------------------------------------------------
- done Generics in structs
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
