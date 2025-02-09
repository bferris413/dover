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
- Tuple structs
  * done - vis
  * fields
  * field vis

- struct field
- Identify enum pieces
--------------------------------------------------------------------------------
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
