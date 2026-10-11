# Uninstall purlis

This page says where purlis keeps its state on a machine, and how to remove it. Each step names
what it removes. Nothing here deletes a project or a repo of yours: those are left for you to
keep or delete. No single command does all of this yet; `purlis reset` is planned (#610).

purlis was called charter until 2026-10, so most places below may also exist under the old
name. Each step names both.

## What an uninstall leaves alone

- **Your projects.** A project is a git repo of yours, with its workspaces, personas, memory and
  settings, and its clones of your repos. Each project also has a state folder, `.purlis/`
  (`.charter/` in a project made before the rename), which git never commits. It holds this
  machine's own records for that project, among them its vaults' registry for this machine and
  the keys index of each keyring vault, which names the keys and never holds a value. It goes
  when you delete the project's folder, and not before.
- **Your harnesses and forge tools.** Claude Code, Codex, opencode, `gh` and `glab`, and their
  logins, are yours and stay. Step 3 takes back what purlis wrote into their configuration.
- **1Password.** No step here touches it, or the items your 1Password vaults read.
- **`/etc/purlis/policy.json`**, where an administrator has written one. purlis only reads it.

## 1. Quit purlis

Quitting ends every chat the app runs. A chat that has not saved its work loses what it held in
its session, so finish or close the chats you care about first.

## 2. Take your credentials out of the keyring

Do this before you delete the app, and before you delete any project, because it reads the
project's keys index to know which items are its own.

A **keyring vault** keeps each secret as one item in the operating system's credential store:
the login Keychain on macOS, the Secret Service on Linux. purlis names every item it makes with
a service that starts with `purlis/` (`charter/` for one made before the rename). Two kinds of
item are purlis's:

- **A vault's secret**: service `purlis/<vault>/<8 hex digits>`, account the key's name. Each
  keyring vault's keys index, `.purlis/vaults/<vault>.keys.json` in its project, names its
  service and its keys.
- **A vault's token kept in the keyring**, such as a 1Password service-account token you gave
  a vault's tab: service `purlis/@identity/<id>`, account the variable it stands in for.

In each project, run, from inside the project's folder:

```sh
purlis vault list                     # each vault, and its provider
purlis secret list <vault>            # a keyring vault's keys
purlis secret rm <vault> <key>        # removes that key's keyring item and its line in the index
```

`purlis secret rm` on a keyring vault removes exactly one item from the keyring and that key's
line from the vault's keys index, and nothing else. Run it on keyring vaults only: on a
1Password vault it removes that key's field from the item in 1Password. `purlis vault remove`
takes a vault out of the project's registry and deletes no item, so remove the keys first.

What `secret rm` cannot reach, such as a `purlis/@identity/…` token or an item left by a
project you have already deleted, you remove by its service: in **Keychain Access** on macOS
(search for `purlis/` and `charter/`), or in your desktop's keyring manager on Linux, such as
**Passwords and Keys**. Delete only items whose service starts with `purlis/` or `charter/`.
macOS may ask for your login password, because purlis's items are held to the app.

## 3. Take back what `purlis plugin install` wrote

If you ran `purlis plugin install` for the `claude` or `codex` you start in a terminal, take it
back with the app's own command, before you delete the app:

```sh
# macOS
/Applications/purlis.app/Contents/MacOS/purlis plugin uninstall --dry-run   # prints each change
/Applications/purlis.app/Contents/MacOS/purlis plugin uninstall
# Linux, from the .deb
purlis plugin uninstall --dry-run
purlis plugin uninstall
```

It takes back what `install` wrote, and nothing else: the plugin's registration in your Claude
Code user `settings.json` and the copy of the plugin it kept in purlis's config folder, purlis's
Bash guard in `~/.codex/config.toml` with Codex's record that you trusted it, and purlis's file
in opencode's plugin folder. [Install](../crates/purlis-core/docs/install.md#chats-you-start-in-a-terminal)
says what each install wrote. An app installed as `charter.app`, which an update kept under that
name, has the command at `/Applications/charter.app/Contents/MacOS/purlis`.

Chats the app started need nothing here: the app armed each one for that chat alone.

## 4. Remove the app

**macOS.** Drag `purlis.app` (or `charter.app`) from Applications to the Bin. That removes the
app, its `purlis` and `charter` commands and the plugin it carries.

If you ran **Install `purlis` command in PATH**, two links are left in `/usr/local/bin`,
`purlis` and `charter`, each pointing at the command inside the app. Check that each still
points into `purlis.app` or `charter.app`, then remove the two links (`sudo` is needed when the
folder is not yours):

```sh
ls -l /usr/local/bin/purlis /usr/local/bin/charter
sudo rm /usr/local/bin/purlis /usr/local/bin/charter
```

**Linux, `.deb`.** `sudo apt remove purlis` removes the package's files: the app,
`/usr/bin/purlis`, `/usr/bin/charter` and the plugin it carries.

**Linux, AppImage.** Delete the `.AppImage` file. It installed nothing else.

## 5. Remove this machine's purlis folders

These folders are purlis's alone. Deleting one removes exactly what is listed under it, and
makes purlis, if you install it again, start as it does on a new machine.

**The config folder**: `purlis/` in `~/.config` (`charter/` before the rename), or in the
folder `$PURLIS_CONFIG_HOME`, else `$XDG_CONFIG_HOME`, names. It holds:

- `machine.json`: the projects this machine has opened and the ones you approved, which were
  open in which window, your pins, the update channel this machine follows, this device's
  random id, and the week it last fetched the update list;
- the record of the extensions you turned off on this machine, beside it;
- `purlisd/` (`charterd/` before the rename): the session host's socket and the credential
  file of each of its local clients, which mean nothing without the app;
- `plugin/`: the plugin copy `purlis plugin install` kept, if step 3 left it;
- the forges' cached responses, kept to ask them again cheaply;
- `local-plane/`, only on a machine where an older purlis made the first-run project there and
  no newer one has launched since, which moves it to the data folder: that project, the same as
  the data folder's `local-project/` below. Copy out what you want from it first.

Deleting it makes purlis forget every project and every approval on this machine. No other
project loses anything.

**The data folder**: `~/Library/Application Support/purlis/` on macOS and
`~/.local/share/purlis/` on Linux (`charter/` in the same place before the rename), or the
folder `$PURLIS_DATA_HOME` names, else `purlis/` in `$XDG_DATA_HOME`. It holds:

- `local-project/`: **the project the first run made for you**, with its workspaces, their
  clones of your repos, its memory and its records. It is a project like any other, kept on this
  machine only. **Copy out anything you want from it first**, or push it to a remote of your
  own: deleting the data folder deletes it, and nothing else holds a copy;
- `events/`: the log of what chats on this machine did, which the app reads back;
- `cache-homes/`: the package caches sandboxed chats used, one per project, which a package
  manager fetches again when it needs them;
- the record of what the sandbox blocked, and the Inbox's record of updates.

**The app's own folders**, named for its identifier, `dev.purlis.app` (`dev.charter.app` before
the rename):

- `~/Library/Application Support/dev.purlis.app/` on macOS, `~/.local/share/dev.purlis.app/` on
  Linux: the shims a shell tab finds first on its `PATH` and the git hooks a chat's commits go
  through, both written again at the app's next launch, and, on Linux, the app's log and its
  crash log in `logs/`;
- `~/Library/Logs/dev.purlis.app/` on macOS: the app's log and its crash log;
- the web view's own storage and cache, where the system keeps them for the app:
  `~/Library/WebKit/dev.purlis.app/` and `~/Library/Caches/dev.purlis.app/` on macOS,
  `~/.cache/dev.purlis.app/` on Linux. purlis keeps nothing of its own there.

None of these holds a project, a memory or a credential.

## Installing again

A new install starts as on a new machine. A project you kept opens again with **Open an existing
project**, and asks once whether you approve it, as any project does on a new machine.
