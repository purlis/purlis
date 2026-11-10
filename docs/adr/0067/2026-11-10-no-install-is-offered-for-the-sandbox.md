# Amended 2026-11-10: no install is offered for the sandbox

[Claude Code's own sandbox is gone from Linux](2026-11-10-claude-code-s-own-sandbox-is-gone-from-linux.md),
so no sandbox on any system needs a program installed. SD-30's install action
([ruled V78](2026-10-03-ruled-v78-sd-2-slice-4.md), point 3) could no longer be reached. It is
removed, and nothing takes its place.

1. **A refusal offers nothing to install.** The new-chat picker shows why the sandbox cannot be
   applied, and "Start without the sandbox" beside it, as before. It no longer has an install
   line or a "Type it in a shell" button, and the window no longer types a command into a shell
   for the sandbox.
2. **Why not point the action at what Linux still needs.** All a Linux sandbox needs now is the
   kernel's Landlock, at version 6 or later. That is a kernel setting, not a package. Turning it
   on means changing the boot loader's kernel line or upgrading the kernel, and no single
   command typed in a shell does that safely. An install button there would suggest something
   that cannot be done in one step. The refusal already says what to do in plain words
   (`landlock_way_on`), and the person can start the chat without the sandbox meanwhile. For
   someone who is not an advanced user, that is the clearer path.
3. **git's install command stays.** The first run still names the distribution's command where
   git is missing (`backend::install_command`). Only the sandbox stopped using it.

The record's "offers SD-30's prerequisite install" now means: names what is missing and what to
do about it.
