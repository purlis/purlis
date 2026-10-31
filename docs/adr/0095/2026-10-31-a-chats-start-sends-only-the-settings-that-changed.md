# Amended 2026-10-31: a chat's start sends only the settings that changed

**Built by #4088**, under spec #2041, after the person asked that a profile's settings stop
crossing whole at every chat's start. It extends the 2026-10-30 amendment, whose checks, limits and
all-or-none stand. Everything here is **decided in implementation, to be confirmed** (D-4088-1 to
D-4088-7 in the pull request).

1. **A runner says what it holds** (session protocol 1.60, `settings_held`, the paired desktop's
   alone, on its own link, as `keep_profile` is): the path and SHA-256 of each settings file of a
   profile it keeps for that desktop, in pages within a control frame. They are read off its disk
   as a desktop reads its own, at that moment. Nothing is remembered, since a chat may be able to
   write in a settings folder. A profile kept for another desktop, or for none, holds nothing, so a
   runner's own settings are never told.
2. **The desktop sends the difference.** Its files are read and stripped as before, and only those
   whose digest differs are staged. `keep_profile` carries `settings_changed`: the paths to remove,
   and the digest of all the settings as the desktop has them. Files left behind over the limit are
   chosen as before, and so leave the runner as before.
3. **What a runner said decides only what is sent, never what is kept.** At the keep the runner
   reads what it holds again, puts the staged files in place, takes out what is named, runs the
   same whole-file check any keep runs, and refuses unless the result comes to the digest the keep
   names. A file changed or planted since, or one its list lied about, ends there.
4. **A refusal sends them all.** On any refusal of a keep of what changed, the desktop sends all the
   settings on the same link, which replaces whatever is held, as before this amendment. So does a
   list that could not be a runner's: a path an import would not write, one twice, a digest that
   looks like none, pages that do not go forward, more than a runner holds, or one with no file in
   common. A runner that stops answering is asked nothing more.
5. **Nothing is written where nothing differs.** Where the result is what the settings folder
   already holds and it holds nothing else, the keep moves nothing. Under each listed name only
   private folders leading to those files, and each file private, of its size, with one name, may
   be there, reached through no link. Anything else, such as a hidden file, a skill's
   `node_modules`, a link or a second name for a file, which a read of the settings passes over, has
   the keep replace them all from what it just checked.
6. **A runner before 1.60 is sent them all**, as it was.

**What this does not defend against:** a runner whose own program is changed can ignore any keep,
as it could when every file was sent. The boundary here is the runner's program, against what
its disk holds.
