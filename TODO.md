# TODO

* Fix Miri bugs.
* `walk_*` try to find a non-visited property of node to create stored pointer from to avoid repeated
  creation of `&mut other_field` refs (looks like cannot use first field `span`)
  * Move span to be last field of structs, so then can always use it?
  * Or 2nd field, so in deterministic position for all structs, for fast lookup?
  * Or are the Miri fails due to `Span` being `Copy`?
    (I'm assuming they're because it's 1st field, but could be wrong)
  * Check how `repr(Rust)` orders fields.
* Replace `push_stack` + `pop_stack` calls in `walk_*` with `replace_stack` + `retag_stack`.
* Check lifetimes - don't allow `Ancestor` refs to live longer than visitor.
  * Write compile fail tests for this.
* `Ancestor` returned by `.parent()` etc contain actual references, so remove `.span()` method calls.
* Implement `Debug` on `Ancestor` properly.
