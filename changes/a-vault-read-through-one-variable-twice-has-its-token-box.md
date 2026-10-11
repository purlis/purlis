### Fixed

- **A vault whose bindings all read one variable has its token box.** The vault's tab counted
  bindings, so a vault that reads one token variable for two targets said it needed two tokens
  and drew no box. It now counts variables, as storing the token does (#1528).
