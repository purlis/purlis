# Amended 2026-10-30: a profile's settings reach a runner in batches

**Built by #4081**, under spec #2041, after a person's every chat on a Docker pool was refused for
a profile with a few skills. It extends item 1 of the 2026-10-29 amendment, which stands except
for how much crosses and how. Everything here is **decided in implementation, to be confirmed**
(D-4081-1 to D-4081-7 in the pull request).

1. **Settings cross in batches** (session protocol 1.56, `stage_profile_settings`, the paired
   desktop's alone, on its own link, as `keep_profile` is). Each batch fits a control frame, and
   a file larger than one crosses in pieces. A `keep_profile` naming how many files were staged
   keeps them.
2. **All or none.** Batches are held in memory, on that link alone, so a link that ends or a batch
   refused leaves nothing. The runner checks the whole files as it checks files one `keep_profile`
   carries, then writes them into a new folder of their own under the folder of the profiles
   purlis makes, which no chat may write and which is on the same volume, never the settings
   folder itself, which a chat may be able to write. Only then does it move each listed name aside
   and the new one in, through descriptors, putting every move back if one fails.
3. **One overall limit: 128 MiB and 8192 files.** Past it the chat still starts. The desktop
   leaves behind whole skills and files, reading nothing of a file past the limit, and the chat's
   tab says what and how to bring it. The exception is a harness's own settings file, which says
   what a chat may do: one over 512 KiB, or that does not parse, still refuses the start, in
   words. Nothing that was refused before is sent: what is read, and what is removed from it, is
   what item 1 says. A skill's `node_modules`, `__pycache__` and `venv` do not go to a runner; an
   import on the same machine still copies them.
4. **A runner before 1.56 keeps today's refusal, in words**: it is sent what one `keep_profile`
   carries.
