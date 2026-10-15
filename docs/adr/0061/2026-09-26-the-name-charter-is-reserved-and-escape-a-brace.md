# Amendment, 2026-09-26: the name `charter` is reserved, and `{{` `}}` escape a brace

Two gaps this decision left open, closed the day it was accepted.

**A persona named `charter` could pass its own action off as a built-in.** Its ids read
`charter/<id>`, the built-ins' namespace, so a file with an id no built-in uses looked like one
in every menu. The id and label checks above cannot see it, because nothing about the file
clashes; the persona's name does. So `purlis` is a reserved persona name, beside the leading
`_` purlis already keeps for its own namespaces (`personas::RESERVED`, one function,
`reserved_refusal`, asked by all three readers):

- `purlis persona create purlis` is refused with a sentence saying why.
- A persona that already has the name is not made unusable: it still loads, runs and can be
  removed, because the reservation is not part of the name grammar every command checks
  (`shape_refusal`). `purlis persona lint` reports it as an error.
- `curation::parse` refuses every file under `personas/charter/curation/`, so `resolve` leaves
  each out with the usual warning naming the file, and `purlis persona curation add purlis`
  writes nothing.

The comparison ignores case, though the persona alphabet is lowercase and already refuses
`purlis`.

**A literal `{word}` could not be typed.** Now `{{` is a literal `{` and `}}` a literal `}`, the
convention of Rust's `format!` and Python's `str.format`, read in the same single pass as the
variables: nothing is expanded twice, and a substituted value is still never scanned, so a
value holding `{{` is typed as it is. `{{word}}` types `{word}` and is not a variable, so lint
accepts it; `{{{subject.name}}}` types the name in braces. A lone `}` stays text, as it was,
rather than becoming an error the way it is in `format!`, so no prompt that parsed before stops
parsing. What moves: `{{subject.name}}` used to type the name in braces and now types
`{subject.name}`, and `}}` used to type two braces and now types one. Curation actions shipped
in no release before this, so no plane holds a prompt that relied on either.
