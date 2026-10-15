# Amended (2026-10-14, qw374; amends section 5): a sandbox is compiled from a whole list, or not at all — to be confirmed

**To be confirmed by the operator.** This is a rule on the sandbox's boundary, decided in
implementation.

Section 5's denials name where each vault keeps its secrets, and the vault registry is what
says where that is. **A chat's sandbox is compiled only from a list that is whole.** Every entry
of the registry, in the committed half and in this machine's, is judged, under whatever name it
is written. No entry is passed over, and an entry keeps the files it names, whatever kind it
says.

- **Whole, and the chat starts.** A project with no vault registry, or no state folder yet,
  registers no vault: that is absent, not unreadable. A registry that registers nothing. Vaults
  in the keyring, in 1Password, by reference, or in a plain file that says where its file is.
  And three states that say something and stop nothing:
  - **A `plain-file` vault with no file said on this machine yet**, as on a teammate's fresh
    clone: nothing of it is here to keep. The vault's view says it is not set up on this
    machine.
  - **An entry that names no kind of vault**, which is what this machine's half keeps of a
    vault the project no longer registers. It names nothing to keep. Doctor says it is left
    over, and the vault's view removes it.
  - **A vault of a kind this purlis does not know**, as one a newer purlis registered. The
    files its entry names are kept from the chat as a vault's file is, the chat starts, and
    Doctor says to update purlis.
- **Not whole, and no chat starts.** A half of the registry that is there and does not read: not
  JSON, not an object, a link, or not a plain file. A `vaults` that is not an object. A
  `plain-file` vault that says where its file is in a way purlis cannot use. A vault's file
  with more than one name, in a project that holds more than purlis looks through for the
  others.
- **Every harness, on every system, the same.** The refusal comes before any harness's compiler
  is asked (`sandbox::compile_checked`, `NotStarted::KeptUntold`), so no Claude Code settings
  and no wrap's profile are made from a short list. A new chat, a resumed one, a relaunch, a
  restored chat and a dispatched one all start through it. A chat that is already running keeps
  the sandbox it started under and is not stopped.
- **The person is told in purlis's own sentence**, where every refused start is said: why, with
  the path of the file to put right, that nothing was started, and that Doctor says what is
  wrong. The new-chat picker says it before anything starts. `purlis doctor` has a
  `sandbox kept files` row whenever the list is not whole, whether or not the project runs its
  chats sandboxed, and whenever a whole list has one of the three states above to say.
- **A vault's file is kept under the other names purlis finds for it.** Each one found is
  denied as the file is, in the one list, so each harness's sandbox and the window's file reads
  hold it alike.
- **The window reads by the same list** (ADR 0038, amended the same day): the plane root's
  files and, in a project that runs its chats sandboxed, a branch's folder. While the list is
  not whole, the one file of the project that still opens in the window is the half of the
  registry to put right.

Decided here, and ruled by the operator, 2026-10-15 (a vault of an unknown kind starts, with a Notice):

- **A vault of a kind this purlis does not know does not stop a start.** A registry written by
  a newer purlis does not stop an older one: the files the entry names are kept, and Doctor
  asks for the update. The stricter choice is to refuse every sandboxed start while such an
  entry is registered, which stops every person on an older purlis until they update or the
  entry is removed.
- **An entry of no kind, and a `plain-file` vault with no file said yet, do not stop a start.**
- **The sentence names the repair and not the opt-out.** Section 7's opt-out is unchanged and is
  still the person's to choose in the picker.
