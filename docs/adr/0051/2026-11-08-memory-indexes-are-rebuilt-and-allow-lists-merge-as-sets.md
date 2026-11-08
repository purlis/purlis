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

**A list every machine adds to is a set (D4).** `[sandbox] hosts` was one long line that every
Always allow appended to, so any two machines that each allowed a host conflicted. The keys whose
lists are sets are named in one place, `purlis_core::tomlmerge::SETS`: `[sandbox] hosts`,
`[sandbox] browser-sites`, a persona's `[sandbox.personas.<name>] hosts` and
`[dispatch.grants] <persona>`. Their writers write them one item per line, sorted and once each,
keeping every other line and comment of the file, so two machines that each add one usually change
different lines. Where git's merge still stops on `charter.toml`, the save merges it on the parsed
TOML: each set comes out as both sides' items less any either side removed since the two parted,
so a revoke on one machine holds against the other
(`hosts_both_machines_allowed_merge_as_a_set_and_a_removal_holds`). *Rejected:* a file of allows
per machine, because a removal has to hold on every machine. A plane's existing one-line lists are
rewritten the first time a writer or a merge touches them.

**The rest of `charter.toml` merges key by key (#5824 F6).** In the same merge, a key only one
side changed (added, removed or given a new value) takes that side, as that side wrote it; a
table both sides changed is merged key by key inside it; values are compared as TOML reads them,
so spacing alone is no change. A key both sides changed, each its own way, is the person's: the
save stops with nothing changed, and names the file and the key, as `charter.toml (sandbox.mode)`
(`a_key_both_machines_changed_in_charter_toml_is_a_persons_conflict_named_by_its_key`). purlis
never writes a conflict marker into the TOML. How Resolve shows that key to the person is lane C's
(F7); until then it is git's own stopped merge, as for any file a person wrote.

A chat's sandbox holds `charter.toml`, so a merge that changes it is brought in by the window,
not by a chat's save, as any move that would write a held file is.
