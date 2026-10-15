# Amendment, 2026-10-13: on the plane root's tab, Files is the whole project's (issue 1916)

The operator's rulings of 2026-10-13: at the plane root, Explorer shows the whole plane's files
(G3); everything in it may be listed and opened except what purlis keeps private, the same
exclusions a chat's sandbox applies to reads (F1); and a workspace's clone shows as one row that
jumps to that workspace's tab (F2). This replaces the third point of the amendment above, which
left the tab with a heading and a sentence.

- **The plane root is a place the window reads, named by no names.** The core's branch type
  gains it (`files::Branch::plane_root`), read by the same machinery as a workspace's repo or
  branch: the bounded reader finds the folder and answers what git would, the tree is one folder
  at a time, and each open folder is watched. No workspace, repo or piece name spells it, so
  every command that takes those names still reads a branch and nothing else.
- **What purlis keeps to itself is neither listed nor opened** (F1), decided in one place
  (`files/kept.rs`). The window's exclusion list and the sandbox's read denials are one list
  (`sandbox::kept_from_reading`): the vaults and their keys, each chat's hook spool, every
  dispatch's record, what waits to be told to a chat, and the person's credentials on this
  machine. A path is judged by its spelling in any case, and by what it is, so another name, a
  link or a second name of a kept file reaches nothing. Each refusal is said in purlis's own
  sentence, the same whether or not the path is there.
- **F1 is read as: what of purlis's own state is hidden is what a chat cannot read.** The rest
  of the state folder (session records, the registries that name the vaults, the app's other
  records) is listed and opens under the rule for every other file, as a chat may read it.
- **A list that cannot be told whole lists nothing.** Where the vault registry does not read, or
  a vault in it does not say where it keeps its file, the plane root's tree and every open are
  refused with a sentence that says so and names Doctor.
- **A workspace's clone is one row** (F2): the workspace's mark, the folder's name and
  _workspace <name>_; pressing it selects that workspace's tab. Nothing at or below it is a
  folder or a file of the plane root. A repository nested anywhere else is drawn as a nested
  repository is in any branch: its folder opens, and nothing in it does. A workspace's branch
  folders, under its `.worktrees`, are such repositories here: they are not rows that lead to
  the workspace.
- **The window's alone.** The two commands that read it, `plane_tree` and `plane_file`, are on
  the list no link is served (ADR 0052, ADR 0068), so no chat gains a way to read the project's
  own files through the UI RPC. The folder watch is served on a link for a branch's folders, and
  refused there for a folder of the plane root (`ui::names_the_plane_root`).

Decided in this change, beyond the three rulings, and to be confirmed:

- **The same rule as a branch for what opens**: a file the project's git tracks, or does not
  ignore. What it ignores is hidden until _Show ignored files_, then dimmed, and never opens.
- **A project whose folder is not a git repository of its own lists nothing**, and says so: there
  is no list of what opens to go by.
- **Nothing is compared at the plane root.** No change marks, no _Changed only_, no cockpit: it
  has no base, and the reader refuses those questions for it.
- **Its rows are read, and nothing more.** No menu, no drag onto a chat, no picks, and its file
  tab has no _Open in your editor_, _Copy path_ or _Add to a chat's context_: each names a
  branch. Search and the file palette do not look in it. Each is a follow-up in issue 1916.
- **The folder watch takes the plane root's folders in the one command it has** (`files_watch`,
  the window's set of watched folders), instead of a second watch of its own. It hands back
  nothing a folder holds, and a link's call that names the plane root is refused.
