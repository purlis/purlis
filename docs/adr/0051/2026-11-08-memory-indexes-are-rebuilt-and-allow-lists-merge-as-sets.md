# Noted 2026-11-08 (#5824 lane B): memory indexes are rebuilt, and allow-lists merge as sets

Decided in implementation under the person's standing instruction of 2026-11-08 (take the
recommended path), to be confirmed. It extends the note of 2026-11-05 on bringing a moved target
in by a merge.

**A memory store's `MEMORY.md` is generated, and rebuilt like the todos index.** The plane's
`.gitattributes` gives `personas/*/memory/MEMORY.md` and `workspaces/*/memory/MEMORY.md`
`merge=union`, which the forge ignores, so two machines that each remembered something made a
request that could not merge. A union merge done here stitched duplicate or out-of-order lines.
Now, where both sides changed a store (a persona's, `_shared`'s or a workspace's), its index is
written from the memory files of the merged tree: a memory either side forgot or compacted away
stays gone, one both sides added is listed once, and a line keeps the title the side that changed
it wrote. A workspace's memory files start with the time they were written, so its index is in the
order of their names. A persona's are named by their subject, so its index keeps this machine's
order, with the other machine's new lines after it
(`a_memory_index_both_sides_added_to_is_rebuilt_not_stitched`). The union rules stay, for git run
by hand.
