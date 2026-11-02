# Built-in skills are `purlis-<name>` on every harness, and the prefix is reserved

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-01**, operator's ruling, ticket #4524, parent #2762. It replaces the opencode-only rule of the [2026-10-26 opencode amendment](2026-10-26-opencode-gets-the-built-in-skills-under-their-own-names.md), and §3's rule that a plane skill may not have a built-in skill's bare name.

## What holds now

- **Each built-in skill has one name.** The 12 in the bundle are `purlis-<name>`, folder and `name:` together, on every harness. No route renames one. Only purlis's own skills: a person's skills and installed skills keep their names.
- **On Claude Code the plugin namespace stays**, so a built-in skill is `/purlis:purlis-handoff`. The plugin keeps its name.
- **The index skill is named the same way.** A router's index skill (ADR 0102 §2) is `purlis-skill-index` on every route.
- **opencode is handed the bundle's own `skills/`**, beside the set's folder, which now holds only what the plane brings and the index. The reason for handing it a rewritten copy was the name. A router never hides a built-in skill, so the bundle's folder is what the set holds.

## Old names keep working

- `purlis:handoff` and `charter:handoff`, in a persona's `skills:` list, a skill's or a curation action's text or a name given to `skill_load`, read as `purlis:purlis-handoff`. Nobody's plane has to change. `Reference::parse` is the one reading.
- **A bare `handoff` is an ordinary skill name.** It means the plane's or the harness's skill of that name and never falls back to the built-in one, so a name has one meaning however many skills exist. A bare `/smart-close` is no longer purlis's smart close. purlis still hears `/purlis:smart-close` as one, though a harness that knows only the new command may not run it.

## The prefix is reserved

- **New, rename, install, share and every other change that lands a skill under a name refuse a `purlis-` name** for anything but a built-in skill, in one line.
- **A plane skill already named so is left out, never deleted**: `Reason::ReservedName`, whose text asks for a rename. Needs you lists it, as it listed a plane skill with a built-in skill's name.
- **The prefix replaces the list.** Which names are reserved no longer depends on which built-in skills exist, so a later built-in skill cannot take a name a plane already uses.
- **A plane skill named `handoff` loads**, and so does one named `skill-index`. A built-in skill still never loses, because no plane skill can have its name.
- **A repo or harness skill named `purlis-<name>` is not left out**, because the harness reads it whatever purlis says. The built-in lints warn of it. Where it has the name of a skill purlis hands, the index skill's among them, it is a `Clash`: `Outcome::BothLoad`, or on opencode, which keeps one skill per bare name, `Outcome::EitherOne`.

## What goes

The per-harness naming has no work left: `Route::names_built_ins_its_own`, `Route::built_in_name`, `Route::built_in_names`, `handed_text_as`, the harness argument of `skills::whose` and of `smart_close_typed`, and the laying out of a folder for the built-in skills alone. With the index skill's bare name free, so is the work of keeping one copy of it: `Reason::IndexName`, `Reason::HiddenByIndex`, `Route::off_by_name_spares_handed`, `has_own_index` and `Chosen::own_index`. Which built-in skills exist stops being asked for a reservation, so `Reserved` goes too. `SHIPPED` stays for the doctor's check of a project keeping its own copy of one.
