# Amended 2026-11-02: an outcome eval stages, and its judge reads, files that are not text

Ticket #4230 (SK-55b), the follow-up the amendment of 2026-10-31 rejected for then. It changes what
that amendment says of a file's form and nothing of its guards.

**An eval's input file is read as bytes**, at the same 1 MiB and through no link, in the same read
as the skill's copy, so the consent covers those bytes. Each file goes into the digest after its
length, as a file that is not text may hold any separator. So a consent or an opt-in given to a
grade with outcome evals is asked for once more. A file that does not read, past the bound included, is left out
and named, as before: no eval runs on part of a file.

**A plan says which files it cannot show.** Purlis checks a text file for characters no reader would
see. It cannot do that for a file that is not text, and that includes a text file with one bad byte
or a NUL, by the rule the skill review reads with. So the plan names such a file as not checked,
beside the eval, before the person ticks it, and says a judge may not be able to read it. The check
reads a file's bytes as they are, so text encoded inside a file that passes as text, such as a 7-bit
PDF's streams, is not read.

**What a chat left reaches its judge as bytes**, at the same bound: at most 32 files, 8 folders deep,
no dot-name, each read pinned. A chat's instructions file is still withheld and listed as withheld,
whatever its bytes are. A file past the bound, or one that is not a plain file, is not copied, and
`files.txt` lists it as not copied, so its judge can tell it from a file the chat never made. An
expectation that needs it fails on the burden of proof.

**Every guard on a judge stands.** It reads inside its own folder alone, with `--tools Read,Grep`,
and its answer is taken whole or not at all. A file that is not text gives it nothing else to read
with. A harness whose Read cannot open such a file leaves the expectation unproved, and it fails.

## Rejected

- **A larger bound for files that are not text.** It would mean a second limit, a budget across the
  32 files, and a change to a ruling #3235 holds. It can be raised on its own if evals need it.
- **Treating a file that is not text as `hides`.** That line says a character is hidden, which
  nobody found. The plan says what is true: this file was not checked.
