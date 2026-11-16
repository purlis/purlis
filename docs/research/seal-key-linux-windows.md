# The seal key on Linux and Windows: what can hold it to the app

Research note, 2026-11-16. It answers issue #6986 (part of map #6969): purlis seals what the
person accepted (the accepted policy text, grants and nevers; rulings Q306, Q308b, Q309, Q314)
with an HMAC key. On macOS that key is held to the purlis app. On Linux and Windows it is not.
Is there a standard way to hold it to the app there, what does each way cost the person at each
start, and what should purlis do?

## How to read this note

Each claim is tagged with where it comes from:

- **[cited]**: read this session in the primary source linked. Some kernel and man-page text
  was read from GitHub copies, because the sandbox refused `man7.org` and `kernel.org` (see
  [Open questions](#open-questions)). The man pages came from `github.com/mkerrisk/man-pages`,
  Michael Kerrisk's own mirror. It stopped in 2021, so a page there can be behind the current
  one. The kernel docs came from `github.com/torvalds/linux`. Fedora's Yama setting came from
  `github.com/sourceware-org/elfutils`, the elfutils project's mirror.
- **[cited, not re-read]**: a primary source I know of but could not open, because the sandbox
  refused the host. Check it before you rely on it.
- **[repo]**: this repository's code and docs.
- **[inference]**: my reasoning from the above. Not established until it is measured.

## The answer

There is no standard way on Linux or Windows to keep a stored secret from another program that
runs as the same user without root or admin rights. The reason is the same on both: a desktop
program installed without a package has no identity the OS checks. The Secret Service
specification "does not mandate any form of access control" [cited]. GNOME's maintainers closed
the keyring bug on this as "no bug here", because "the privilege boundary is at the user level,
not the app level" [cited]. DPAPI, DPAPI-NG, systemd-creds `--user` and the kernel's user
keyrings all scope a secret to the user, never to a program [cited]. KWallet's per-application
allow list trusts an application name that the caller supplies itself [cited]. The macOS
Keychain checks a code signature, and nothing on the Linux or Windows desktop does that for an
unpackaged app.

Persistence is the root of it. **Any secret that survives a restart and that the app can read
without the person can also be read by another program running as that user** [inference, from
the above]. The only ways round this are:

1. The person proves presence at each start (a password, PIN or biometric), and the key then
   lives only in the session host's memory.
2. The chat runs as a different OS user.

Option 1 does not make the key the app's alone. It raises the bar. A chat can no longer read a
D-Bus item in one call. It has to get code into the next purlis start and wait for the person
to approve a prompt there. A prompt that appears when the person did not start purlis is the
visible sign.

Recommendation, in two lines:

- **Linux**: keep the declared gap as the default. Offer an opt-in "ask for a password at each
  start". It keeps the key in a Secret Service collection of its own, locked with its own
  password, which purlisd unlocks at start, reads once and locks again. After that the key
  lives only in purlisd's memory, and purlisd is made non-dumpable.
- **Windows**: keep the declared gap as the default. Offer an opt-in "Windows Hello at each
  start": a Hello signature over a fixed challenge gives a key that unwraps the seal key, which
  then lives only in purlisd's memory behind a process DACL that denies the user memory access.

Neither opt-in means much while the app's binary sits where the user can write to it (an
AppImage in the home folder, or a per-user install under `%LOCALAPPDATA%`). Doctor should say so.

## The threat, stated once

The attacker is a chat. It runs as the person's own OS user, as a descendant of `purlisd`
[repo, brief], and the policy may turn its sandbox off. It has no root and no admin rights. It
can run any code, start processes, use the session D-Bus, read and write every file the user
can, and set environment variables for programs it starts. That includes the purlis binary
itself with `LD_PRELOAD` or extra arguments. It can ptrace whatever the kernel lets it. It can
send signals to `purlisd`, which runs as the same user [inference]. Its goal is a valid HMAC over
a loosened policy or a grant. To get one it must read the key, or have purlis sign for it.

Out of scope: root or admin compromise, kernel exploits, physical access.

## Candidates on Linux

### The Secret Service specification and libsecret

1. **Holds to the app?** No. Chapter 10 says: "This specification does not mandate any form of
   access control. The service may choose to allow certain applications to access a keyring,
   and others. [TODO: complete]"
   ([spec ch. 10](https://specifications.freedesktop.org/secret-service-spec/latest/ch10.html))
   [cited]. A service "may choose to allow any client application to access items or
   collections unlocked by another client application"
   ([ch. 8](https://specifications.freedesktop.org/secret-service-spec/latest/unlocking.html))
   [cited]. Encryption in transit "is not envisioned to withstand man in the middle attacks"
   ([ch. 7](https://specifications.freedesktop.org/secret-service-spec/latest/transfer-secrets.html))
   [cited].
2. **Cost per start**: nothing while the login collection is unlocked by PAM at login. Arch's
   wiki notes the login collection is unlocked whenever the user is logged in
   ([Arch wiki, GNOME/Keyring](https://wiki.archlinux.org/title/GNOME/Keyring)) [cited].
3. **Available**: yes on stock Ubuntu and Fedora desktops (GNOME Keyring or KWallet) [inference].
4. **From Rust**: purlis already uses it, through `keyring` 4.2.0, which in turn pulls in
   `secret-service` 5.2.0 [repo, `Cargo.lock`]. `secret-service` 5.2.0 has
   `create_collection`, `Collection::lock`, `unlock`, `is_locked`
   ([docs.rs](https://docs.rs/secret-service/5.2.0/secret_service/struct.Collection.html))
   [cited]. GNOME's `oo7` 0.7.1 is the other mature client
   ([docs.rs](https://docs.rs/oo7/latest/oo7/)) [cited].

### GNOME Keyring

1. **Holds to the app?** No. The ACL API existed in the old `libgnome-keyring` era: "New API
   functions for getting/setting ACL" (0.3.2), and `GNOME_KEYRING_ITEM_APPLICATION_SECRET` "for a
   single application only with strict access controls" (0.7.1)
   ([NEWS](https://gitlab.gnome.org/GNOME/gnome-keyring/-/raw/main/NEWS)) [cited]. Today's
   daemon serves the Secret Service, which has no ACL. I found no NEWS line naming the removal.
   The wiki page that describes the security philosophy (`wiki.gnome.org/Projects/GnomeKeyring`)
   was refused [cited, not re-read]. The maintainers' current position is on record in
   CVE-2018-19358's issue
   ([gnome-keyring#5](https://gitlab.gnome.org/GNOME/gnome-keyring/-/issues/5)) [cited]:
   - "programs running as the user can read the user's keyring. That's not surprising at all
     … The privilege boundary is at the user level, not the app level." (Ray Strode, 2018)
   - "if a program can just impersonate any other application trivially, then locking things
     down at the dbus layer doesn't even help … a program could just toss `LD_PRELOAD` into
     `~/.bash_profile` and get full access to `gnome-keyring-daemon`."
   - "If you have access to the user's session bus, then you are a trusted/unsandboxed process
     with full permissions to do absolutely anything on the bus. This is fundamental to the
     Linux desktop security model" (Michael Catanzaro, 2023, closing it as "no bug here").
2. **Cost per start**: nothing for the login collection. A collection with its own password
   asks for that password through the daemon's prompt when it is unlocked [inference; measure].
3. **Available**: the default on Ubuntu and Fedora Workstation (GNOME) [inference].
4. **From Rust**: as above.

### KWallet

1. **Holds to the app?** No. In KDE Frameworks 5, `kwalletd`'s `isAuthorizedApp(appid, …)`
   checks `appid` against `_implicitAllowMap`. `appid` is a string argument of the D-Bus call
   `open(wallet, wId, appid)`, so the caller supplies it
   ([kwalletd.cpp at v5.116.0](https://github.com/KDE/kwallet/blob/v5.116.0/src/runtime/kwalletd/kwalletd.cpp))
   [cited]. The allow list is read from `kwalletrc` in the user's own config [cited, same file].
   A same-user process can therefore claim any name, and it can edit the list. In today's
   `master` the daemon is `ksecretd`. Its `open` path still has the comment "isAuthorizedApp
   might show a dialog", but no such function is left in the file
   ([ksecretd.cpp](https://github.com/KDE/kwallet/blob/master/src/runtime/ksecretd/ksecretd.cpp))
   [cited]. Arch's wiki still recommends "Prompt when an application accesses a wallet"
   ([Arch wiki, KDE Wallet](https://wiki.archlinux.org/title/KDE_Wallet)) [cited]. That prompt
   names a self-declared app, so it is advice for people, not a boundary [inference].
2. **Cost per start**: nothing, or a click-through prompt if the person turned that on.
3. **Available**: the default on Fedora KDE and Kubuntu, not on stock Ubuntu or Fedora
   Workstation [inference].
4. **From Rust**: through the Secret Service, as above.

### The Secret portal (`org.freedesktop.portal.Secret`)

1. **Holds to the app?** No, not against an unsandboxed caller. `RetrieveSecret` returns "a
   master secret … unique per application … In a typical backend implementation, it is stored
   in the user's keyring, under the application ID as a key"
   ([interface XML](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.Secret.xml))
   [cited]. GNOME Keyring's backend stores it as an ordinary item with the attribute `app_id`,
   labelled "Application key for %s"
   ([gkd-secret-portal.c](https://gitlab.gnome.org/GNOME/gnome-keyring/-/blob/main/daemon/dbus/gkd-secret-portal.c))
   [cited]. So any unsandboxed process can read it straight from the Secret Service
   [inference]. For a program outside a sandbox the app ID is either missing or claimed by the
   program itself through `org.freedesktop.host.portal.Registry.Register(app_id, …)`
   ([interface XML](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.host.portal.Registry.xml))
   [cited]. `oo7`'s own docs say why the portal exists: so a *sandboxed* app's secrets are not
   exposed "to other applications that can talk to the org.freedesktop.Secrets service"
   ([docs.rs](https://docs.rs/oo7/latest/oo7/)) [cited]. It protects the app from sandboxed
   peers. It does not protect it from unsandboxed ones.
2. **Cost per start**: nothing.
3. **Available**: needs `xdg-desktop-portal` and a backend that implements Secret, which GNOME
   Keyring does [cited, NEWS "dbus: Implement secret portal backend"].
4. **From Rust**: `oo7` [cited]. Not useful here.

### polkit

1. **Holds to the app?** No. polkit answers yes or no, and it gives no key material. A rule sees
   the subject's `pid`, `user`, `groups`, `seat`, `session`, `local` and `active`, and nothing
   about which program is calling
   ([polkit(8)](https://www.freedesktop.org/software/polkit/docs/latest/polkit.8.html))
   [cited]. `auth_self` means "Authentication by the owner of the session that the client
   originates from is required"; `auth_self_keep` keeps that "for a brief period (e.g. five
   minutes)" [cited, same page]. To hold anything back, a privileged mechanism (a root-owned
   system D-Bus service) must keep the key in a file only root can read and hand it out after
   `auth_self` [inference]. It hands the key to whichever process asks and passes the prompt.
   A chat can ask too, and the person sees the same password dialog. Bitwarden's desktop app
   uses polkit for its Linux unlock and says so plainly: polkit "only returns a yes/no
   authorization decision and no key material", and a key it persists in the Secret Service is
   readable by "any un-sandboxed process running as the user"
   ([bitwarden/clients biometric/src/linux.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/linux.rs))
   [cited].
2. **Cost per start**: the user's login password (or fingerprint, where PAM is set up for it)
   [inference].
3. **Available**: polkit is on both distributions. A new action file and system service must be
   installed by root, so a `.deb` or `.rpm` can ship it and an AppImage cannot [inference].
4. **From Rust**: `zbus_polkit` 6.0.0, "Status: Stable"
   ([docs.rs](https://docs.rs/zbus_polkit/6.0.0/zbus_polkit/)) [cited], on `zbus` 5, which is
   already in the lockfile [repo]. The root helper would be a new privileged program for purlis
   to write and audit [inference]. That is a large cost for a prompt the user-level option below
   gives without root.

### Linux kernel keyrings (`keyctl`)

1. **Holds to the app?** Only as well as process memory does, and only until the process ends.
   - Rights are the union of the user, group or other set and, "if the key is possessed", the
     possessor set. "Granting permissions to the key possessor while denying them to the key
     owner and group allows the prevention of access to keys on the basis of UID and GID
     matches" ([keyrings(7)](https://man7.org/linux/man-pages/man7/keyrings.7.html), read from
     the GitHub mirror) [cited].
   - A key in purlisd's *process* keyring with possessor-only rights cannot be read with
     `keyctl` by another process of the same user. A forked child "initially has no process
     keyring", and the keyring "is cleared on execve(2)"
     ([process-keyring(7)](https://man7.org/linux/man-pages/man7/process-keyring.7.html))
     [cited]. So chats do not inherit it.
   - The *session* keyring "is inherited across clone(2), fork(2), and vfork(2)" and "preserved
     across execve(2)"
     ([session-keyring(7)](https://man7.org/linux/man-pages/man7/session-keyring.7.html))
     [cited]. A key there is possessed by every chat.
   - The *user* keyring "is shared by all processes with that UID"
     ([user-keyring(7)](https://man7.org/linux/man-pages/man7/user-keyring.7.html)) [cited].
   - The *persistent* keyring is per UID, any process of that UID can link it with
     `keyctl_get_persistent`, and it lasts only "until its expiration timer triggers"
     ([persistent-keyring(7)](https://man7.org/linux/man-pages/man7/persistent-keyring.7.html))
     [cited]. Nothing in kernel keyrings survives a reboot, since they live in kernel memory
     [inference].

   A same-user process that can ptrace purlisd reads the key whichever way it is kept
   [inference]. So the process keyring adds little over a key in purlisd's own memory.
2. **Cost per start**: nothing, but the key still has to come from somewhere at each start.
3. **Available**: on every stock kernel [inference].
4. **From Rust**: `linux-keyutils` 0.2.5 (pure Rust) or `keyutils` 0.4.0 (bindings to the C
   library) ([docs.rs](https://docs.rs/linux-keyutils/latest/linux_keyutils/)) [cited, versions
   only]. Both are young.

### systemd-creds

1. **Holds to the app?** No. `--user` makes a credential "user-scoped": "the selected user's
   numeric UID and username, as well as the system's machine-id(5) are incorporated into the
   encryption key". Such a credential "may only be decrypted from the specified user's context"
   ([systemd-creds(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-creds.html))
   [cited]. In the `io.systemd.Credentials` Varlink service, `Decrypt` skips polkit when
   `scope == user && uid == peer_uid` ("Relax security requirements if peer wants to encrypt
   credentials for themselves")
   ([src/creds/creds.c](https://github.com/systemd/systemd/blob/main/src/creds/creds.c))
   [cited]. Any process of that UID can decrypt it. The TPM2 and host keys tie a credential to
   the machine and its boot state, not to a program [cited, same man page: "can only be
   decrypted again by the … TPM2 chip"; host key "only accessible to the root user"].
2. **Cost per start**: nothing.
3. **Available**: `--user` was "Added in version 256" [cited]. My understanding is that Ubuntu
   24.04 ships systemd 255 and Fedora 41 and later ship 256 or newer [inference; check].
4. **From Rust**: run `systemd-creds`, or speak Varlink to `io.systemd.Credentials`. There is no
   mature crate for it [inference]. Not useful here.

### TPM2 sealing on Linux

1. **Holds to the app?** No. A PCR policy ties a secret to the machine and its boot chain, not
   to a program [inference, from systemd-creds(1)'s description of `--tpm2-pcrs`]. An
   `authValue` (PIN) ties it to a person, and the TPM's dictionary-attack lockout throttles
   guessing [inference]. A PIN the app stores is readable by the same user [inference].
2. **Cost per start**: nothing with a PCR policy only; a PIN with an `authValue`.
3. **Available**: tpm2-tss's udev rule makes `/dev/tpmrm*` mode `0660`, group `tss`
   ([tpm-udev.rules](https://github.com/tpm2-software/tpm2-tss/blob/master/dist/tpm-udev.rules))
   [cited]. A desktop user is not in `tss` by default, so an unprivileged app cannot open the
   TPM on stock Ubuntu or Fedora without the person changing groups as root [inference; measure
   on both].
4. **From Rust**: `tss-esapi` 7.7.0 ([docs.rs](https://docs.rs/tss-esapi/latest/tss_esapi/))
   [cited, version only], which binds the C TSS. That is a heavy dependency for a PIN prompt.

## Candidates on Windows

### DPAPI (`CryptProtectData`)

1. **Holds to the app?** No. "Typically, only a user with logon credentials that match those of
   the user who encrypted the data can decrypt the data"
   ([CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata))
   [cited]. Every process of that user qualifies.
   - `CRYPTPROTECT_UI_FORBIDDEN` "is used for remote situations where presenting a user
     interface (UI) is not an option": a call that would show UI fails instead [cited]. It adds
     no protection.
   - `pOptionalEntropy` must be passed again to decrypt [cited]. The app would have to keep it
     somewhere it can read without the person, and the same user can read it there too
     [inference].
   - `CRYPTPROTECT_PROMPT_ON_UNPROTECT` is deprecated: "will be removed in April 2027 … There is
     no replacement for the interactive prompt behavior itself"
     ([CRYPTPROTECT_PROMPTSTRUCT](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/ns-dpapi-cryptprotect_promptstruct))
     [cited]. Do not build on it.
2. **Cost per start**: nothing.
3. **Available**: every Windows edition [inference].
4. **From Rust**: the `windows` crate (0.62.2, already in the lockfile [repo]), feature
   `Win32_Security_Cryptography`
   ([docs.rs features](https://docs.rs/crate/windows/latest/features)) [cited]. This is what
   Credential Manager's user scope already gives purlis through `keyring` [inference].

### DPAPI-NG (`NCryptProtectSecret`)

1. **Holds to the app?** No. A protection descriptor names `SID`, `SDDL`, `LOCAL=user` or
   `LOCAL=machine`, `WEBCREDENTIALS` or `CERTIFICATE`
   ([Protection descriptors](https://learn.microsoft.com/en-us/windows/win32/seccng/protection-descriptors))
   [cited]. None of these names a program. `LOCAL=user` is DPAPI's user scope again
   [inference].
2. **Cost per start**: nothing (`LOCAL=user`), or whatever the certificate's key asks for.
3. **Available**: Windows 8 and later [cited, not re-read].
4. **From Rust**: `windows`, `Win32_Security_Cryptography` [inference: same feature]. Nothing
   gained over DPAPI.

### Windows Hello (`Windows.Security.Credentials.KeyCredentialManager`)

1. **Holds to the app?** Not on its own, but it is the one Windows API here that yields
   key-bound output only after the person proves presence.
   - `OpenAsync` "Retrieves a key credential for the current user and application"
     ([OpenAsync](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.keycredentialmanager.openasync))
     [cited]. The pages read do not say what "application" means for an unpackaged Win32 app
     (see open questions).
   - `RequestSignAsync` "Prompts the user to cryptographcally sign data using their key
     credential"
     ([RequestSignAsync](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.keycredential.requestsignasync))
     [cited]. The keys are RSA 2048, and signatures are "PKCS #1 RSA PSS with SHA256"
     ([KeyCredentialManager](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.keycredentialmanager))
     [cited].
   - Bitwarden's desktop app signs a per-user challenge, which "produces a deterministic
     signature which is hashed to obtain a key", and encrypts the vault key with it. Its threat
     note: "the challenge is known to all userspace processes. Therefore, to circumvent the
     security measure, the attacker would need to create a fake Windows-Hello prompt, and get
     the user to confirm it"
     ([bitwarden/clients biometric/src/windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs))
     [cited]. It also notes Hello prompts need a focus workaround ("Ugly hack: We need to focus
     the window via window focusing APIs") [cited, same file].

   So against our threat: a chat cannot get the key without a Hello confirmation from the
   person. It can raise such a prompt itself, or get code into the next purlis start
   [inference].
2. **Cost per start**: one Hello confirmation: face, fingerprint or the Hello PIN.
3. **Available**: Windows 11 Home included, once the person has set up Hello. A PIN is the
   minimum, and Microsoft-account setup offers it by default [cited, not re-read]. Some local
   accounts have no Hello set up. `KeyCredentialManager.IsSupportedAsync` says which
   [inference].
4. **From Rust**: `windows` 0.62.2, feature `Security_Credentials`
   ([docs.rs features](https://docs.rs/crate/windows/latest/features)) [cited]. Bitwarden ships
   this from Rust with the `windows` crate [cited, same file]. Its signature step uses one
   `unsafe` byte view (`as_mut_bytes`) [cited, same file]. purlis would need either a safe
   route to the buffer's bytes or a sixth audited block, and a sixth block is a new ruling
   [repo, CLAUDE.md].

### TPM keys through the Microsoft Platform Crypto Provider (NCrypt)

1. **Holds to the app?** No. The provider keeps private keys in the TPM so "they cannot be
   extracted"
   ([CNG key storage providers](https://learn.microsoft.com/en-us/windows/win32/seccertenroll/cng-key-storage-providers))
   [cited]. Being non-extractable is not the same as being usable by one program only: a
   per-user key can be opened by name by any process of that user [inference].
   `NCRYPT_UI_POLICY` with `NCRYPT_UI_PROTECT_KEY_FLAG` will "Display the strong key user
   interface as needed"
   ([NCRYPT_UI_POLICY](https://learn.microsoft.com/en-us/windows/win32/api/ncrypt/ns-ncrypt-ncrypt_ui_policy))
   [cited]. That is a consent dialog, not an app check. `NCRYPT_PIN_PROPERTY` sets a PIN, and
   "Microsoft KSPs will cache this value so that the user is only prompted once per process"
   ([Key storage property identifiers](https://learn.microsoft.com/en-us/windows/win32/seccng/key-storage-property-identifiers))
   [cited].
2. **Cost per start**: a PIN or consent dialog, or nothing.
3. **Available**: Windows 11 requires TPM 2.0 [cited, not re-read].
4. **From Rust**: `windows`, `Win32_Security_Cryptography`. Hello (above) is the same kind of
   TPM-backed key with a better prompt [inference].

### AppContainer

1. **Protects purlis's secret?** No. It confines the process inside the container. It does not
   guard a secret from processes outside. A Package SID "is derived from a string name
   (moniker)", and an app that launches its own AppContainer picks the moniker; an existing one
   is found again with `DeriveAppContainerSidFromAppContainerName`
   ([Implementing an AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer))
   [cited]. Any same-user process can derive the same SID and start a process in the same
   container [inference]. AppContainers "run with a Low Integrity Level" [cited, same page], so
   ordinary medium-integrity processes of that user outrank them [inference]. "Credential
   isolation" there means the *contained* app "cannot impersonate the user"
   ([AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation))
   [cited].
2. **Cost per start**: nothing.
3. **Available**: Windows 8 and later [cited].
4. **From Rust**: `windows`, `Win32_Security_Isolation` [cited, features list]. The right use
   would be to contain the *chat*, which is sandboxing. The policy can switch sandboxing off,
   so it does not answer this question.

## The candidates side by side

| Candidate | Same-user unsandboxed process kept out? | Per-start cost | Ubuntu 24.04+ / Fedora 40+ / Windows 11 | Rust |
| --- | --- | --- | --- | --- |
| Secret Service (login collection) | No | nothing | yes / yes / — | `keyring`, `secret-service` (in tree) |
| Secret Service, own locked collection | Only while locked; open for the moment it is read | its password | yes / yes / — | `secret-service` 5.2.0 |
| KWallet allow list | No (self-declared name) | nothing or a click | KDE only | via Secret Service |
| Secret portal | No (plain keyring item) | nothing | yes / yes / — | `oo7` |
| polkit + root helper | No identity; a password prompt gates it | login password | `.deb`/`.rpm` only | `zbus_polkit` + a new root program |
| Kernel process keyring | Same as process memory; lost at exit | nothing | yes / yes / — | `linux-keyutils` |
| systemd-creds `--user` | No (UID only) | nothing | 256+ only / 41+ / — | the CLI or Varlink |
| TPM2 (Linux) | No (machine or PIN) | PIN or nothing | `tss` group needed | `tss-esapi` |
| DPAPI / DPAPI-NG | No (user scope) | nothing | — / — / yes | `windows` (in tree) |
| Windows Hello | No identity; a Hello prompt gates it | face, finger or PIN | — / — / yes, once Hello is set up | `windows` (in tree) |
| PCP TPM key | No (per user) | PIN, consent or nothing | — / — / yes | `windows` |
| AppContainer | No (confines the inside) | nothing | — / — / yes | `windows` |

## What is achievable: raising the bar

### The persistence problem

The seal must still check after a restart: accepted policy and grants outlive purlisd. Checking
needs the key. If purlisd can get the key at start without the person, it uses some store or
derivation that depends only on the user's identity. Then a same-user process can use the same
store or derivation. It can run the purlis binary itself to do it, if it has to. Only an
identity the OS checks for the program would break that, and no such identity exists for an
unpackaged app on the Linux or Windows desktop (all candidates above). On macOS the Keychain
checks the code signature of the program that created the item, which is why purlis holds it
there by having the app's own binary create it [repo, `crates/purlis-core/src/secrets/keyhold.rs`
module doc]. [inference, from the cited candidates]

So the choice on Linux and Windows is one of these: accept the gap; ask for the person's
presence once per purlisd start; or move chats to another OS user.

### A key held only in purlisd's memory

The shape is the same on both OSes. The seal key is stored wrapped. At each start purlisd
unwraps it with something only the person's presence yields, then keeps it only in its own
memory. Every seal and check goes through purlisd. The window and the `purlis` command ask
purlisd, and never read the key [inference].

There is one condition, and it is easy to miss. **purlisd must never seal on a chat's say-so.**
A chat can connect to purlisd's socket as the same user. If an "accept" request on that socket
gets a seal, the key never needs to leave memory for a chat to forge one. The seal is then only
as strong as purlisd's check that an acceptance came from the person's own act in the window
[inference]. This note did not check how purlisd tells those apart today (see open questions).

#### Linux: dumpable and Yama

- `ptrace`'s access check denies access "if the target process 'dumpable' attribute has a value
  other than 1 … and the caller does not have the `CAP_SYS_PTRACE` capability"
  ([ptrace(2), "Ptrace access mode checking", step 4](https://man7.org/linux/man-pages/man2/ptrace.2.html))
  [cited]. Reading `/proc/[pid]/mem` is governed by a `PTRACE_MODE_ATTACH_FSCREDS` check
  ([proc(5)](https://man7.org/linux/man-pages/man5/proc.5.html)) [cited]. So
  `prctl(PR_SET_DUMPABLE, 0)` in purlisd keeps every same-user process without
  `CAP_SYS_PTRACE` out of its memory. That includes its own descendants, and it holds whatever
  Yama is set to [inference from the two].
- The attribute is reset when IDs change or a set-ID or capability program is executed
  ([prctl(2)](https://man7.org/linux/man-pages/man2/prctl.2.html)) [cited]. purlisd sets it
  first thing in `main`, and the setting covers only that process image [inference].
- Yama mode 1: "a process must have a predefined relationship with the inferior … By default,
  this relationship is that of only its descendants"
  ([Yama.rst](https://github.com/torvalds/linux/blob/master/Documentation/admin-guide/LSM/Yama.rst))
  [cited]. A chat is purlisd's descendant, so under mode 1 it cannot attach to purlisd, but it
  can attach to any purlis binary it starts itself [inference]. Mode 0 allows attaching to "any
  other process running under the same uid, as long as it is dumpable" [cited, same page].
- Defaults: Arch sets 1 ([Arch wiki, Security](https://wiki.archlinux.org/title/Security))
  [cited]. Ubuntu sets 1 (`wiki.ubuntu.com/Security/Features`, refused) [cited, not re-read].
  Fedora sets 0: elfutils ships `10-default-yama-scope.conf` with
  `kernel.yama.ptrace_scope = 0`, and `elfutils-libs` `Requires: default-yama-scope`
  ([config/10-default-yama-scope.conf](https://github.com/sourceware-org/elfutils/blob/master/config/10-default-yama-scope.conf),
  [elfutils.spec.in](https://github.com/sourceware-org/elfutils/blob/master/config/elfutils.spec.in))
  [cited]. That stock Fedora installs elfutils-libs is my understanding, and it should be
  checked [inference]. So on Fedora only `PR_SET_DUMPABLE` stands between a chat and purlisd's
  memory.
- `memfd_secret(2)` removes pages "from the kernel page tables", so even `/proc/pid/mem` cannot
  reach them ([memfd_secret(2)](https://man7.org/linux/man-pages/man2/memfd_secret.2.html))
  [cited]. Bitwarden uses it for the same purpose [cited, linux.rs]. A tracer that already
  controls the process can make the process read the pages for it [inference]. With dumpable
  off, it adds little.
- Cost to purlis: a non-dumpable purlisd blocks the crash companion's ptrace too. purlis sets
  the companion as ptracer today (`handler.set_ptracer(Some(companion.id()))`,
  `crates/crash-handler/src/attach.rs`) [repo]. That is a Yama exception, and dumpable 0 still
  denies it [inference from ptrace(2) step 4]. The crash path would have to set dumpable back
  to 1 before it signals the companion [inference; check against ADR 0083].
- From Rust: `rustix::process::set_dumpable_behavior`
  ([docs.rs](https://docs.rs/rustix/latest/rustix/process/fn.set_dumpable_behavior.html))
  [cited], with `rustix` 1.1.6 already in the lockfile [repo]. No `unsafe` is needed.

#### Windows: the process DACL

- "If you specify NULL, the process gets a default security descriptor. The ACLs in the default
  security descriptor for a process come from the primary or impersonation token of the
  creator", and a process can change its own with `SetSecurityInfo`
  ([Process security and access rights](https://learn.microsoft.com/en-us/windows/win32/procthread/process-security-and-access-rights))
  [cited]. A same-user process normally gets `PROCESS_VM_READ` from that default [inference].
- purlisd can replace its DACL with one that denies the user `PROCESS_VM_READ`,
  `PROCESS_VM_WRITE`, `PROCESS_VM_OPERATION`, `PROCESS_CREATE_THREAD` and `PROCESS_DUP_HANDLE`.
  The owner keeps implicit `READ_CONTROL` and `WRITE_DAC`, so another process of that user could
  restore the DACL. An ACE for OWNER RIGHTS (`S-1-3-4`) prevents that: "the system ignores the
  implicit READ_CONTROL and WRITE_DAC permissions for the object owner"
  ([Special identity groups](https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/understand-special-identities-groups))
  [cited].
- "If the caller has enabled the SeDebugPrivilege privilege, the requested access is granted
  regardless of the contents of the security descriptor"
  ([OpenProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess))
  [cited]. An elevated admin gets past the DACL, which is out of scope here.
- From Rust: `windows`, features `Win32_Security_Authorization` and `Win32_System_Threading`
  [cited, features list].

### What still gets through

This is why the opt-ins raise the bar and do not close the gap [inference throughout]:

- **The next start.** A chat can kill purlisd, since they run as the same user, and start a new
  one as its own child. It can trace that child from its first instruction (`PTRACE_TRACEME`,
  `DEBUG_PROCESS`) or preload a library into it (`LD_PRELOAD`). Both happen before purlisd can
  set dumpable or a DACL. When the person then confirms the prompt, the chat's copy gets the
  key. purlisd can check `TracerPid` in `/proc/self/status` and refuse a set `LD_PRELOAD`, but
  each such check is one more thing to get round. The visible sign is a prompt the person did
  not cause.
- **The binary itself.** Where the app sits in a folder the user can write to, the chat can
  replace it. Examples: an AppImage in `~/`, `~/.local/bin`, or a per-user install under
  `%LOCALAPPDATA%`. Every start after that is the chat's. Only a system-wide install (`.deb`,
  `.rpm`, Program Files) puts the binary out of the user's reach.
- **The prompt itself.** On Windows a chat can call `RequestSignAsync` on the same credential and
  get a real Hello prompt. Bitwarden's note assumes exactly this. On Linux a chat can call
  `Unlock` on the seal collection and get the real keyring prompt. Everything rests on the person
  not confirming a prompt they did not expect.
- **The socket**, as above: purlisd must not be a signing oracle.

### Running chats as a different OS user

If a chat ran as its own OS user, the OS's user boundary would do the work. A `0600` file, the
person's own Secret Service, DPAPI user scope or the Credential Manager would each hold the key,
and Yama, dumpable and process DACLs would no longer matter for this purpose. It needs a
privileged step to create and switch users (root on Linux, admin on Windows), it changes file
ownership in every worktree, and it changes how chats reach the person's tools and credentials.
That is a different product decision from this note. It is out of scope here, but it is the
only setup in which the fixed rule holds on Linux and Windows without the person's presence at
each start [inference].

## What declaring the gap means

Ruling Q309 point 4 already declares it: on Linux and Windows the seal stops hand edits and
sandboxed chats only. It is declared in Settings and doctor, and chats still start [brief].
This note confirms the gap is real and not a purlis shortcut: no standard store on either OS
offers more [cited, above].

What the seal still stops:

- A hand edit of the policy, grant or never files. Editing them by hand does not produce a
  valid HMAC.
- A sandboxed chat. purlis's sandbox can keep it off the session D-Bus and the Credential
  Manager, and so away from the key [inference; holds only if the sandbox profile denies the
  session bus socket and keyring access, which should be tested].
- Mistakes. A tool that rewrites a file by accident gets no valid seal.

What it does not stop: an unsandboxed chat that reads the key from the Secret Service or
Credential Manager. Any program run as that user can do the same.

What the person should be told, in the window's words (draft, following `docs/ui-copy.md`'s
rules [repo]):

- Settings, by the sandbox switch (Linux, Windows): "Without the sandbox, a chat can read the
  key that seals your policy, and so could loosen the policy itself. On this system no program
  can keep a key from another program run by the same user."
- Doctor line: "Seal key: held for this user, not for purlis alone (Secret Service)". On
  Windows: "(Credential Manager)". Then: "Unsandboxed chats could forge an acceptance." The way
  out is a link to the setting below if it ships, and otherwise to this explanation.
- If the opt-in ships, doctor adds a line when the app's own binary is writable by the user:
  "purlis is installed where your user can change it, so asking at start protects less. Install
  it system-wide for the full effect."

## Recommendation per OS

**macOS.** Keep it as it is (`keyhold.rs`) [repo].

**Linux.**

- **Default: the declared gap.** Keep the Secret Service login collection through `keyring`
  4.2.0 (`Held::NoRule`) [repo].
  - Per-start cost: nothing.
  - Residual risk: any unsandboxed same-user process can forge a seal.
- **Opt-in: "Ask for a password each time purlis starts".**
  - On turning it on, create a Secret Service collection of its own (for example "purlis
    seal"). The keyring daemon asks the person for that collection's password. Move the seal key
    there.
  - At each purlisd start, call `Unlock`, read the key, call `Lock`, and keep the key only in
    memory. purlisd sets `PR_SET_DUMPABLE(0)` first thing in `main`, and its crash path sets it
    back to 1.
  - Everything uses standard tools: the user's own keyring daemon and its own prompt, the
    `secret-service` crate already in the tree, and `rustix`. It needs no root, no new daemon
    and no own crypto. It works for an AppImage, but protects less there.
  - Per-start cost: one password.
  - Residual risk: the moment between `Unlock` and `Lock`; a prompt a chat raises; the next-start
    and binary-replacement paths above.
  - Before building: measure that GNOME Keyring and KWallet each prompt for a non-default
    collection's own password, and that `Lock` takes effect at once.
- Do not build polkit, a root helper, TPM2, systemd-creds or kernel keyrings for this. Each one
  costs more and none adds an app identity.

**Windows.**

- **Default: the declared gap.** Keep Credential Manager through `keyring` [repo].
  - Per-start cost: nothing.
  - Residual risk: as on Linux.
- **Opt-in: "Confirm with Windows Hello each time purlis starts".**
  - On turning it on, create a `KeyCredential` and sign a fixed, per-install challenge. Hash
    the signature into a wrapping key, and store the seal key wrapped (in Credential Manager or
    a file; the wrapped form is not secret).
  - At each purlisd start, sign again, unwrap, and keep the key only in memory. purlisd's
    process DACL denies the user memory and thread access, with an OWNER RIGHTS ACE.
  - Per-start cost: one Hello confirmation (face, fingerprint or PIN), which is cheaper than a
    password.
  - Residual risk: a Hello prompt a chat raises; the next-start and binary-replacement paths
    (the default per-user install is user-writable); an elevated admin with `SeDebugPrivilege`.
  - Offer it only where `IsSupportedAsync` says Hello is set up.
  - Before building: confirm that the signature really is deterministic (see open questions),
    and that the bytes can be read without a new `unsafe` block.
  - Build this before the Linux opt-in, because its per-start cost is lower.

**Both opt-ins stay opt-in**: each costs every start, and neither closes the gap. The real fix
is running chats as another OS user, which needs a decision of its own.

## Open questions

- **Is a Windows Hello signature deterministic?** The key type is documented as RSA-PSS with
  SHA-256 [cited], and PSS normally uses a random salt [inference]. Bitwarden's code calls the
  signature deterministic and derives a key from it [cited]. Measure on more than one machine and
  TPM before relying on it. If it is not deterministic, Hello gives only a yes or no, and the
  Windows opt-in loses its key material.
- **What "current user and application" means for an unpackaged Win32 app** in
  `KeyCredentialManager.OpenAsync` [cited]. If it is per user only, another process can open
  purlis's credential by name. It still needs a Hello confirmation, so the analysis does not
  change, but the doc should say which.
- **How purlisd tells the person's acceptance from a chat's request** on its socket. The in-memory
  design is worth nothing if a chat can ask for a seal. Not checked in this note.
- **Do purlis's Linux and Windows sandbox profiles deny the session bus and the Credential
  Manager?** If not, "the seal stops sandboxed chats" is not true either. Needs a test.
- **Ubuntu 24.04's and Fedora's systemd versions, and whether stock Fedora installs
  `elfutils-libs`.** Read from memory, not from a source this session.
- **A keyring daemon's prompt for a non-default collection**, and whether `Lock` is immediate, on
  both GNOME Keyring and KWallet (`ksecretd`). Measure.
- **Refused hosts.** The sandbox refused these, so claims that rest on them are tagged
  [cited, not re-read] or were read from a mirror: `man7.org`, `www.kernel.org`,
  `docs.kernel.org`, `git.kernel.org`, `flatpak.github.io`, `api.kde.org`, `invent.kde.org`,
  `wiki.gnome.org`, `wiki.ubuntu.com`, `git.launchpad.net`, `manpages.ubuntu.com`,
  `documentation.ubuntu.com`, `src.fedoraproject.org`, `docs.fedoraproject.org`,
  `fedoraproject.org`, `salsa.debian.org`, `sourceware.org`. The man pages came from the 2021
  `mkerrisk/man-pages` mirror, and newer pages may differ.
