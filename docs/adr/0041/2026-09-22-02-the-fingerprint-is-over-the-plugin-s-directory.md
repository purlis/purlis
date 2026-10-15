# Amendment, 2026-09-22: the fingerprint is over the plugin's DIRECTORY, not over what it declares

**The gap is in this record's own wording**, which is why the amendment is here rather than in a
code comment. Decision 3 says the fingerprint *"has to be a hash of the executable and of every
file it declares, checked at each launch"*. `crates/purlis-core/src/extension.rs` implemented
exactly that. **So a plugin could add or change an UNDECLARED sibling and the fingerprint said
unchanged** — a `.dylib` beside the program, a script it `source`s, a config it reads. None of
those is declared; none of them was hashed.

purlis#150's author followed the record rather than widening it unasked, which was right.
purlis#152 is where the widening got decided, and the operator ruled on 2026-09-22.

**Why it is not a small thing.** It is harmless while stage 1 has no executor and nothing reads
an undeclared file. It stops being harmless the moment stage 2 starts a program, because a
program loads what it likes — at which point *"approved code" stops meaning what the dialog says
it means*. And the dialog was unusually explicit: *"charter has read this extension's files and
will ask again if any of them change."* An undeclared sibling makes that sentence false, and a
consent surface that over-promises is the failure this record's first amendment is entirely
about, arriving through a second door.

## The ruling

**Hash the plugin's whole directory — contents and the set of paths — with ONE manifest-declared
state directory excluded, which is the only place a plugin may write.**

The reasoning is the constraint the rest of this section is designed against, and it is the same
sentence as the first amendment's, pointed the other way: without the carve-out, any plugin that
keeps a cache or a log beside itself re-prompts the operator at every launch, and **a consent
dialog people click through is worse than no dialog at all.**

**What was rejected, and why, so that re-opening any of it has to argue:**

- **hashing the directory with no exclusions** — correct and unusable. A plugin with a cache
  beside it asks at every launch, and the prompt becomes a reflex;
- **refusing to run a plugin whose directory holds anything undeclared** — strictest, and it
  breaks on `node_modules`, `.git`, `README`, `__pycache__`. It reads to the operator as
  *purlis refuses my plugin over a file I didn't write*, which is a tool telling its owner the
  filesystem is wrong;
- **keeping the declared list and rewording the dialog** — cheapest, and it moves the problem
  onto the reader. The record already knows what that costs: decision 3's own *"treating silence
  as a yes is the one state this record exists to keep out"* is the same instinct, and a sentence
  carefully worded around a hole is the polite version of silence.

## What the carve-out has to satisfy, because a carve-out is where this goes wrong

**A state directory that can hold a `.dylib` or a script the program loads has given the whole
property back.** So the exclusion is narrow by construction and the narrowness is enforced, not
described:

1. **One path segment**, directly inside the plugin's directory, so the hole is visible in a
   directory listing rather than buried at `build/tmp/state`.
2. **Named in the manifest, which is itself hashed**, so the carve-out cannot appear, move or
   widen without the operator being asked again.
3. **Nothing the manifest declares may live inside it** — a theme or a program declared under it
   is refused when the manifest is read, so purlis never opens a byte in there.
4. **purlis refuses to load a plugin whose state directory holds a symlink or a file with an
   executable bit.** Metadata only, nothing read, so a cache of ten thousand files is checked
   without being hashed and without prompting anybody.

**And the limit of (4), which belongs in the record and not in a later apology.** No filesystem
predicate makes a file un-loadable as code: `dlopen` does not need the executable bit on Linux,
and `source` needs it nowhere. What the check closes is the careless case and the conventional
one — a helper binary cached beside a log. What it leaves open is *a program the operator
approved choosing to read its own state as code*, which is the same class as a program that
fetches a string and evaluates it, and which no fingerprint anywhere closes. This record already
refuses to pretend otherwise about the fingerprint; it refuses to pretend here too, and the
consent surface carries the sentence rather than leaving it in a doc comment.

## What a symlink is to the hash, decided rather than left to the walk

**A symlink inside the plugin is hashed as a link and is never followed.** Its own target string
goes into the digest. So re-pointing it asks again, a link out of the tree reads nothing out
there, and a loop cannot hang the walk because nothing is walked *through*.

Refusing links outright was the alternative and is the wrong one for the reason the second
rejected option above is wrong: `node_modules/.bin/` is a tree of them. What this leaves honest
is that the *target* of a link out of the plugin is not fingerprinted — it is not part of the
plugin, the link that names it is, and purlis does not claim about files it was never pointed
at.

## The bound, which the declared list did not need and a directory does

A declared list is bounded by the manifest; a directory is bounded by whatever is on the disk, so
**the walk is attacker-influenced input and gets its own limits**: 4096 entries and 64 MiB per
plugin, each refused with a sentence that says what to do rather than a number on its own.
Directories count as entries, which bounds the depth without a second limit to keep in step.

## The cost, which this record left as an open number

purlis#150's point 6 left this unmeasured and the operator asked for it. Measured on an
M-series machine, release build, warm page cache, mean of five re-hashes
(`what_the_re_hash_costs_at_launch` in `crates/purlis-core/src/extension/tests.rs`, so it can be
re-run rather than believed):

| plugin | re-hash at launch |
| --- | --- |
| 4 files / 16 KiB — a theme plugin as one ships | **0.23 ms** |
| 100 files / 1 MiB | **4.4 ms** |
| 1,000 files / 50 MiB | **125–140 ms** |
| 4,064 files / 63 MiB — at the bound | **222–228 ms** |

On a machine under load the bound case reached **1.3 s**. It runs on `spawn_blocking` and the
window is drawn before it, so what a person feels at the bound is the theme repaint arriving
late, not a launch that waits. **The consequence in this record's own list is therefore now
measured for the fingerprint and still open for the subprocess**: gate item 8 is about starting a
process and nothing here touches it.

## What this changes in the record above

- Decision 3's *"a hash of the executable and of every file it declares"* is amended to **a hash
  of every path below the plugin's directory, contents and names alike, except one
  manifest-declared state directory**. The rest of decision 3 — show what it contributes, ask
  once per human per machine, re-ask when it changes, every unreadable state means ask, and it is
  not a boundary — stands word for word.
- The consequence *"this one reads an executable"* is amended to **this one reads a directory**,
  with the numbers above.
- The first amendment's obligation on the consent surface now has a second clause: the prompt
  says a plugin runs with the operator's own access and that the list is a declaration rather
  than a limit, **and it says which one directory purlis did not read**, when there is one.

## What is still unmet

**Items 2, 3, 4, 5, 6 and 8 stand exactly as written**, and nothing in this amendment closes any
of them. In particular this is not a sandbox, it is not a boundary, and it does not make a
plugin's code safe to run — it makes the fingerprint mean what the dialog already claimed it
meant. The implementation is purlis#152.
