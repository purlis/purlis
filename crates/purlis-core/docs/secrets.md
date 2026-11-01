# Secrets and the vault directory

A **vault** is a named store of secrets, and purlis hands a secret to a command without the
value ever passing through an agent's conversation. Every message an agent reads and every
tool result it gets can end up in a transcript — saved, logged, reviewed, or fed back into a
later prompt — so the transcript is not a safe place for a value, whatever holds it on disk.

## The commands

```bash
purlis vault add devops --persona devops                          # a keyring vault, local by default
purlis secret set devops API_TOKEN --stdin                         # value on stdin, never argv
purlis secret list devops                                          # key names, never values
purlis secret exec devops --env TOKEN=API_TOKEN -- curl --variable %TOKEN --expand-header 'Authorization: Bearer {{TOKEN}}' …
purlis secret exec devops --file KUBECONFIG=PROD_KUBECONFIG -- kubectl get pods
purlis persona secret exec --env TOKEN=API_TOKEN -- some-cli       # the active persona's vault
```

The command runs with no shell between: a `$TOKEN` written on the line is expanded by your own
shell before purlis hands anything over, so it is empty there, and a value written into the
command's arguments is visible to other processes. The command reads the value itself: from its
environment or a file, as `kubectl` reads `KUBECONFIG`, or, for curl 8.3.0 or later, through
curl's own `--variable %TOKEN` and an `--expand-` option (`--expand-header`, `--expand-user`).

- **`secret exec <vault> -- <command>`** resolves each value inside purlis and hands it to
  the command in its **environment** (`--env NAME=key`), in a temp file made **0600** whose
  path is the variable (`--file VAR=key`), or in a 0600 dotenv file (`--dotenv VAR=NAME:key`,
  repeats sharing a `VAR` merge into one file). The temp files are removed when the command
  ends, when purlis fails, and when purlis is stopped by any terminating signal it can
  catch; SIGKILL and a crash leave them in the temp directory. By default the command's output
  is captured and every value this call resolved is replaced by `***` before it is printed.
  That is a net against an accidental echo, not a boundary: a command that transforms a value
  (`base64`, a JSON re-encode) prints it unrecognised. `--stream` runs a long-lived command
  with its stdio attached and still removes its files; `--exec` replaces purlis with the
  command and cannot take `--file` or `--dotenv`. Neither captures, so neither redacts.
- **In a sandboxed chat, the app runs it** (#1407). A sandboxed chat may not read any vault
  (ADR 0067 §5 class 1), so `secret exec` there reads nothing itself. It asks the app that
  started the chat, over the chat's hook channel, and the app reads that line only from a
  process inside that chat. One from a process outside it (tmux, `nohup` after its shell ended,
  `docker exec`) is refused with a sentence saying so, and nothing runs. The app checks that the vault registry tags the vault with the
  chat's persona: the persona the app started the chat as. A persona's own `vault:` line does
  not count, because a chat can edit that file. It then resolves the values and runs the
  command in a sandbox built from what the chat's own sandbox was compiled to when the chat
  started: the same denials (its harness's own among them), the chat's folder and a temp
  directory of its own as the only places it writes, and the network only through a proxy
  carrying the chat's hosts. That sandbox is applied as the chat's own is: Seatbelt on macOS,
  and on Linux purlis's own sandbox, started from the purlis program the chat's start checked
  and held, which is never one a chat may write (#2342). Where the chat's start held none, or
  the kernel lacks the Landlock it needs, the run is refused and nothing runs. A `--file` or `--dotenv` file is kept in the vaults folder, which
  every chat is denied, and only that command is given back a read of it. The command leads a
  process group of its own: everything it started that stayed in that group is killed when it
  ends or when the chat goes away. A process that leaves the group on purpose, such as a daemon,
  is not. A chat may
  have four such runs at once. Output streams back with each value's literal text masked, even
  under `--stream` and `--exec`, then the exit status. stdin is passed through when it is not a
  terminal.

  **A host the chat's hosts do not list.** The command's proxy refuses it, and the command
  reports that in its own words, which often name no host (kubectl says "Forbidden"). So the
  proxy's own record of what it refused is what counts: the chat's tab shows the same sandbox
  block Notice as for the chat's own command, naming that host and port whole, with **Allow
  for this chat**, **Always allow** and **Keep blocked**. Before the exit status, purlis says on
  stderr that its sandbox refused that host and that it can be allowed on the tab. A grant
  reaches the next run once the chat is started again with it, which Allow does once the
  chat's turn has ended. The host is never read from what the command printed. A refused host
  that carries a value from the vault is never named or offered: purlis says only that one was
  withheld. Another host refused within the minute gets its own Notice.

  **A database host, through a tunnel** (#1667). A database client opens its own connection
  and never asks the proxy, so it would fail at the name lookup. When an `--env` value names
  one place a client connects to (a database URL such as `postgres://…`, a libpq
  `host=… port=…` string, or `host:port`) and the chat's hosts list that exact host and port,
  purlis opens a port on the loopback interface that carries every connection to exactly that
  host and port, and the command gets the value pointed at it (`127.0.0.1:<port>`), masked as
  the value is. The run's sandbox lets it connect there. A host listed without a port is not
  enough: that is HTTPS's, through the proxy. A host it may not reach is refused as the proxy
  refuses one, with the same Notice. Every tunnelled connection is in the network record.
  A value a `--file` holds alone, and each value of a `--dotenv` file, is pointed the same
  way, in the file itself; every other line stays as it was, and a note names the entry and
  the file's variable (`DATABASE_URL in APP_ENV`).

  **A certificate checked by name** (#1708) is kept where the client has a way to keep it:
  libpq's `sslmode=verify-full` (in the value, or `PGSSLMODE` beside one that names no mode)
  keeps `host` and connects through an added `hostaddr`, and `PGHOST` takes `PGHOSTADDR`; a
  SQL Server URL (`sqlserver://`, `mssql://`, `ms://`) is given `hostNameInCertificate`, the
  host's own name, unless it names one. Where the client has no such way, the value is handed
  as it is, with a note naming the variable, and no tunnel is opened or Block raised: libpq
  values run by `usql` (Go's lib/pq reads no `hostaddr`), `pg://` and `jdbc:postgresql://` with
  `verify-full`, MySQL's `VERIFY_IDENTITY` or `tls=true`, MongoDB with `tls` or `ssl` on (unless
  `tlsAllowInvalidHostnames` or `tlsInsecure` is on), every `rediss://` and `amqps://`,
  CockroachDB URLs with `verify-full`, and a `postgresql+<driver>://` URL with `verify-full`
  whose driver is not libpq's (`+asyncpg`, `+pg8000`; `+psycopg2` and `+psycopg` keep the name
  through `hostaddr`).

  **Host and port variables.** A host alone in `PGHOST`, `MYSQL_HOST` or `REDIS_HOST` is one
  place with its port beside it in `PGPORT`, `MYSQL_TCP_PORT` or `REDIS_PORT` (5432, 3306 or
  6379 where none is set), and both are pointed at the tunnel. `PGHOST` is left as it is where
  `PGHOSTADDR` is set already.

  **ssh, and git over ssh** (#1667), need no vault: every sandboxed chat's ssh leaves through
  its SOCKS port, which carries only the hosts and ports the chat may reach, so a host it may
  not reach is a Block with Allow. A Codex or opencode chat, and a brokered run, find the
  route's `ssh` first on their `PATH` and in `GIT_SSH_COMMAND`. A Claude Code chat's route is
  written under `.purlis/app/ssh/`, which no chat writes, and the file Claude Code reads before
  each command (`CLAUDE_ENV_FILE`) puts it first on that command's `PATH`, so a bare `ssh` takes
  it with the command unchanged. The route names no key and no agent: signing is your ssh's own,
  through `SSH_AUTH_SOCK`, which a brokered run is handed unchanged.

  **Which vaults a chat may use.** One the vault registry tags for the persona the chat was
  opened as, or one you allowed for that persona on this machine. A vault that is neither is
  refused with a sentence naming the ways forward, and the chat's tab shows a notice with
  **Allow {persona} to use this vault** and **Keep blocked**. Allow is kept on this machine
  beside the project, never in the committed registry; it is recorded, listed in Settings ›
  Sandbox › Granted and revoked there, and the next run reads it, so the chat does not restart.
  The chat is then told exactly what now runs: `secret list` of that vault, and the persona's
  declared tools with it where it declares one; reading a value directly stays refused. One
  Allow answers every chat opened as that persona that was refused the same vault, and each is
  told the same; a press where the persona may use the vault already says so and puts the
  notice away.
  A policy's `"vault-grants": false` forbids it. The other way forward is a dispatch to the
  persona the vault is tagged for. A chat's persona is fixed for its life, so nothing run in
  the chat changes which vaults it may use. A vault name the project does not register is
  refused as that.

  **Which commands it runs: only the persona's declared tools** (D-PE-ruling-2, the operator's
  ruling of 2026-10-12). The command must be one of the chat's persona's tools (below), armed
  as the chat started (allowed on this machine, its program found where no chat writes), asked
  for with exactly that tool's vault, key and hand-over. It runs as the tool's own program, by
  its absolute path, from a `PATH` without the project, the chat's folders and the temporary
  folders, and without the chat's variables that make a program load code (the dynamic
  loader's, and `NODE_OPTIONS`, `PYTHONPATH` and their kind). Anything else, a shell or an interpreter among them, is refused with a sentence that
  names the tool to declare, and nothing runs. A credentialed MCP server of the persona that you
  approved on this machine (`purlis persona approve-mcp`) runs as approved: only the exact line
  purlis wraps from the entry you approved, never a server's own arguments written in that shape.
  Its environment values come from your approval too (#3441): the run starts with each variable
  the entry's `env` writes out, at the value its line shows, never the chat's value for it. A
  `${VAR}` value is not on the line, so the run starts without it; write the value out, or name a
  credential under `secrets` so it comes from the vault.
  Such a server's start is marked as one, and is asked for once more when its first run fails
  before the server answered (see personas.md). **Your own terminal,
  and a chat the app did not start sandboxed, are not limited**: `secret exec` there runs any
  command, as it always has.

  **What it keeps from the chat, and what it does not.** It keeps out the vault's storage and its
  provider's session, every vault the chat's persona may not use, every key no tool is handed,
  and the credential file. It does not keep a tool's own value out: a tool's arguments are the
  chat's choice, so a tool that can be told to run something else, or to send its credential
  somewhere, hands that value over. Masking matches a value's literal text only. Prefer `--file` to `--env` for a command that can read
  its credential from a file: an environment variable is readable by the same user through `ps
  eww` while the command runs.
- **A sandboxed chat never reads the keyring through the command** (#1638). Where the app
  does not take a sandboxed chat's `secret exec` (it is closed, or does not answer), a vault
  whose values come from the keyring (a `keyring` vault, or a 1Password vault whose token is
  kept there) is refused with a sentence saying so, and nothing runs: read in the chat, the
  Keychain would ask you on the chat's behalf. `secret get`, `cp` and every other read of such
  a vault in a sandboxed chat are refused the same way, since the app runs only `exec`. The
  refusal names what to run instead: `secret exec` through the app on macOS and Linux, and a
  terminal outside the chat on other systems. Any other vault runs
  as before.
- **A sandboxed chat lists a vault's key names through the app, and reads no value itself**
  (qw385). `secret list <vault>` (and `persona secret list`) in a sandboxed chat is answered
  by the app, which authorises the vault exactly as it does a brokered `secret exec` (the
  vault is tagged for the chat's persona, or you allowed it) and answers the names alone. A
  vault the persona may not use raises the same notice, whose Allow lets the same command run.
  An app that does not answer in time, or is older than the command, is said so; the vault is
  never read in the chat instead.
  `secret get` and `secret cp` (and their `persona secret` forms) are refused in a sandboxed
  chat for every provider, whatever was allowed; the refusal names `secret list` for the key
  names and `secret exec` through a declared tool for a value.
- **The rest of the vault verbs are yours, and a sandboxed chat is told so before anything is
  read** (#1969). `secret set`, `secret rm`, `vault add` and `vault remove` change a vault or
  the registry, and the app takes no write for a chat: in a sandboxed chat each is refused
  before the registry, the vault or standard input is read, with a sentence naming your two ways to do it, the vault's tab (or New vault…
  and Delete vault… in the palette) and the same command in your own terminal. `secret audit`
  and `vault verify` print no value, but read each key's age from the vault's storage and
  resolve every value, which a sandboxed chat does not; they are refused the same way, naming
  `secret list` and `vault list` for what the chat can see, the Vaults panel for each vault's
  health, and your terminal for the full check. `persona secret set`, `rm` and `audit` are
  refused the same way before the persona's vault is looked up, naming the same `persona
  secret` command. Outside a sandboxed chat every verb runs as before.
- **A persona's tools: a plain `kubectl` that runs through `secret exec`** (#1407). The
  project's `charter.toml` can name the programs a persona's chats run with a vault's value:

  ```toml
  [sandbox.personas.devops]
  tools = { kubectl = { vault = "devops", as = "KUBECONFIG file", key = "PROD_KUBECONFIG" } }
  ```

  `as` is `"<VARIABLE> file"` (`--file`, preferred) or `"<VARIABLE> env"` (`--env`). A tool
  that needs several values from its vault, such as a user name and a password, names each
  under `values` instead (T-1, ruled by the operator on 2026-10-14). The program must read
  each value from its variable or file itself:

  ```toml
  [sandbox.personas.cloud]
  tools = { aws = { vault = "cloud", values = {
      AWS_ACCESS_KEY_ID = { key = "aws-access-key-id", as = "env" },
      AWS_SECRET_ACCESS_KEY = { key = "aws-secret-access-key", as = "env" } } } }
  ```

  Its script runs `purlis secret exec cloud --env AWS_ACCESS_KEY_ID=aws-access-key-id --env
  AWS_SECRET_ACCESS_KEY=aws-secret-access-key -- /usr/local/bin/aws`, so `aws s3 ls` in the chat
  signs its requests with both, and a chat's own `secret exec` of it runs only with exactly that
  set. A tool is written one way or the other, never both; it takes at most 16 values, each in a
  variable of its own.

  **curl reads no credential from a variable by itself**, and a chat's `$VAR` on the command
  line is expanded by the chat's own shell, where it is unset. Check `curl --version`; with
  8.3.0 or later, hand the values as `env` and have curl expand them itself:

  ```toml
  [sandbox.personas.logs]
  tools = { curl = { vault = "elastic-logs-master", values = {
      KIBANA_USER = { key = "kibana-username", as = "env" },
      KIBANA_PASS = { key = "kibana-password", as = "env" } } } }
  ```

  ```bash
  curl --variable %KIBANA_USER --variable %KIBANA_PASS \
       --expand-user '{{KIBANA_USER}}:{{KIBANA_PASS}}' https://kibana.example.test/api/status
  ```

  The single quotes keep `{{…}}` away from the chat's shell; curl fills them from its own
  environment, and stops with an error where a variable is not set.

  **A general-purpose program can show the values it receives in the chat's output** (T-4,
  ruled by the operator on 2026-10-14). curl, wget, httpie, base64, tee and the like fetch,
  send, copy, encode or write content as their arguments say, and the chat writes the
  arguments. purlis still runs such a program as a tool, and its Allow says so: the Notice,
  Settings › Sandbox and the Granted list add one sentence beside its line, "curl can show the
  values it is handed in the chat's output; allow it only for a persona you trust with them."
  Allow one only for a persona you trust with its values. The sentence follows a short,
  reviewed list of program names, matched on the tool's name and on the file name of the
  program it finds; it is not part of what the Allow is bound to, so an Allow you made before
  it stands. The list is not exhaustive, and a tool without the sentence is not thereby a safe
  one: any program can show what it is handed if its arguments ask it to.

  As a sandboxed chat running as `devops` starts, the app writes one small script per tool under
  `.purlis/app/tools/devops/bin/`, which no chat may write, and puts that folder first on the
  chat's `PATH`. A Claude Code chat is also handed `CLAUDE_ENV_FILE`, which puts it first again
  before each command, after your shell's own start files. So `kubectl get pods`, in a
  compound command or a script too, runs
  `purlis secret exec devops --file KUBECONFIG=PROD_KUBECONFIG -- /opt/homebrew/bin/kubectl get pods`,
  brokered by the app as above. The real program is the one the app finds on its own search
  path, outside the project, the temp folders and every folder the chat may write, followed
  through its links; none found there means no script for it. The vault must still be tagged
  for the persona. **A persona's tools wait for the same Allow on each machine as its hosts**
  (D-PE-ruling-1): the project view's Notice for the persona shows its hosts and tools
  together, and one Allow (there, or in Settings › Sandbox) is of exactly what it showed. Until
  then, and after any change to a tool or a host, a chat as the persona gets no script.
  A tool named after a shell, an interpreter, `env`, `sudo`, `xargs` or another program that
  runs what it is told, or after purlis itself, is refused with a sentence, and so is a
  variable that steers how programs are found or loaded (`PATH`, `LD_*`, `DYLD_*`). A tool is
  refused where the program it runs, as found and as its links resolve, is a shell, an
  interpreter or another program that runs what it is told, with the same sentence, on the
  Notice and Settings › Sandbox and as a chat starts, and no Allow is of it. A tool runs the
  program its links resolve to, and every link on the way to it is in a folder no chat writes.
  It starts that program under the name the tool was declared as (its `argv[0]`), so a program
  that does what its name says (`unxz` or `xzcat` linked to `xz`, `gunzip` or `zcat`, a
  `git-*` link) does what the declared name says (#1977). In the run's sandbox the app makes a
  link of that name, for that run alone, beside the credential files no chat reads, to the
  resolved program, and runs the link, so the program sees a path that ends in the declared
  name. A program that finds its own files from that path without resolving links (a script
  whose `#!` line names its interpreter and that reads `$(dirname "$0")/../lib`) looks beside
  the link and not beside itself; declare such a tool under its program's own file name. The
  script itself, run where no app answers, can only name the program when its shell has
  `exec -a` (bash and zsh do, dash does not); otherwise the program sees its own name. The
  name is handed to `secret exec` as `--argv0`, an internal flag of the scripts that does
  nothing off macOS and Linux. An administrator's policy can forbid every persona's tools (`"persona-tools": false`): no chat
  gets the scripts, and a save that adds one is refused.
- **A browser login: a persona's tool that logs a browser in to one page** (D-PE-browser-1,
  approved by the operator on 2026-10-12). A sandboxed chat runs only declared tools, and a
  shell or a `--dotenv` is never one, so the `purlis-browser` skill's terminal flow does not run in a
  chat. Declare the login instead, beside the persona's other tools:

  ```toml
  [sandbox.personas.qa]
  tools = { shop-login = { vault = "qa", browser = "https://shop.example.test/login",
                           user = "username", key = "password" } }
  ```

  The chat calls the `browser_login` tool with `login = "shop-login"`, and purlis performs the
  login in the chat's own browser (#2016): the app reads the user name and password from the
  vault for exactly the login the chat's start armed (for a chat started without the sandbox,
  the login its persona declares and this machine allowed, read at the call, #2560), and hands
  them to the session host, whose
  browser goes to the declared page, fills them only while the page's document is on that
  page's origin, submits, waits for the password field to go, and hands the browser back. A
  login that does not go through has what it filled cleared, and the chat is told why. The chat
  never gets the values: they are kept out of every snapshot, find, wait and screenshot of every
  page of that browser for as long as it runs, and each field purlis filled is blanked, so a
  show-password toggle shows nothing. From the first fill the browser reaches the login's
  origin alone; a chat that needs another site opens another browser. No file of the session is
  written. The chat chooses which declared login and nothing else; run as a command, the tool
  runs nothing and names `browser_login`. The login waits for the same Allow as the persona's
  hosts and tools, its digest covering the page and both keys, and policy's `persona-tools`
  forbids it with the rest. A page with a captcha or a second factor does not log in this way,
  and neither does a page that first sends the browser to a sign-in service on another origin:
  the login fails closed (declare that service's page instead).
- **`secret list <vault>`** prints the key names. In a sandboxed chat the app answers it, for
  a vault the chat's persona may use.
- **`secret get <vault> <key>`** prints a size band and a keyed fingerprint —
  `devops/API_TOKEN: present · 16–31 bytes · fp:9c41a0b7e5d2` — never the value. The
  fingerprint is `HMAC-SHA256` under a 32-byte key kept 0600 at `.charter/fingerprint.key`, so
  it compares values within this plane and cannot be checked against a guess without the key.
  `--reveal` prints the value only to a terminal, and refuses any other stdout unless `--force`.
- **`secret cp <vault> <key> <dest>`** writes the value to a new regular file at 0600 and prints
  only the path. A symlink, a device, a FIFO, a directory, an existing file (without `--force`)
  and purlis's own stdin, stdout or stderr under any name are refused before the value is
  read. After that it is an ordinary file: no guard knows purlis put a credential there.
- **`secret set`**, **`secret rm`** and **`secret audit`** (secrets older than `--days`, for a
  keyring or plain-file vault) write and inspect; `set` refuses an empty value unless `--allow-empty`.
  In a sandboxed chat all three are yours, and refused there with the way that works.
- **`persona secret <verb> [--persona <name>]`** runs the same verb on the vault of the active
  persona: its `vault:` field, else the vault tagged with it. `vault: none` says the persona
  holds no credentials.
- **`vault add | list | verify | remove`** manage the registry. `vault list` shows each vault's
  provider, persona, scope and health, never a value. In a sandboxed chat the app answers it,
  because the chat's sandbox denies it every provider's own files: each vault's provider and
  persona, and whether this chat may use it, with no scope or health column and no provider
  asked; `vault verify` resolves every reference
  for real and exits non-zero when one does not resolve. `vault add`, `vault remove` and
  `vault verify` are yours in a sandboxed chat, and refused there with the way that works.

`exec`, `cp` and `get --reveal` each record one event in the session trace naming the vault,
the keys and the command — never a value.

### Providers

- **`keyring`**, the default for `vault add` — the operating system's own credential store:
  the login Keychain on macOS, the Secret Service on Linux (ADR 0047). Each secret is one item,
  service `charter/<vault>/<8 hex>` (random per vault, made at its first write) and account
  `<key>`. A keyring cannot be asked what it holds, so the key names live in a keys index,
  `.charter/vaults/<vault>.keys.json` (0600, gitignored with the rest of `.charter/`), with each
  key's size band and when it was last written — never a value. `list`, `vault list` and
  `audit` read only the index, so they never make the Keychain ask you anything; `get`, `exec`,
  `cp` and `vault verify` read the item. On macOS every item is held to purlis's app: the app's
  own binary writes it (the `purlis` command hands the write to the app beside it), so only the
  app reads it without asking. Any other program — `security find-generic-password -w`, a
  script, a program a chat runs, and the `purlis` command too — makes the Keychain ask you
  first. Answer each ask as it comes. "Always Allow" for the `purlis` command lets any chat
  that runs the command read the item without asking; resolving secrets in purlis's own
  service will take the command out of the way (#1180). An item written before is written
  again, held, the next time purlis reads it (one the `purlis` command made, the next time
  the command reads it), and the vault's next read through the command says so once. A
  purlis update is a new binary, so with an ad-hoc signed build the first read after an
  update asks again. A 1Password vault's kept token is read once per `secret exec`, however
  many values it hands on, not once per value (#1638), and the app reads it once per run of
  the app, however many chats and tabs ask, keeping it in the app's memory only (#1654).
  `vault list`, `persona list` and `persona show`, and the app's Vaults panel never read it:
  their status says the token is kept in the keyring and that `vault verify` reads it
  (#1180, #1654).
- **`plain-file`** (`--provider plain-file`) — a JSON object of key → value at 0600,
  `.charter/vaults/<vault>.json` by default. It is **plaintext on disk**. Inside a plane that is a git repository, `vault add`
  refuses a `--file` git would commit, and `secret set` checks again before it writes, because
  the registry can be edited by hand or arrive in a commit. A file git already ignores, and
  one outside the plane, is accepted. `vault add` also refuses a file another registered vault
  already uses.
- **`reference`** — the file holds URIs, not values, resolved at read time:
  `op://<vault>/<item>/<field>` through `op read`, `vault://<path>#<FIELD>` through
  `vault kv get`. A reference file is safe to commit. `browser://` references are recognised
  and refused: the browser lane is not in this version yet (#996).
- **`1password`** — purlis keeps the vault in one 1Password item (`charter-<vault>`, or
  `--op-item`), each secret a concealed field of it, read and written through the `op` CLI; a
  value reaches `op` on stdin, never in its arguments. When `op` signs in with a
  service-account token, purlis tells it not to read the 1Password app's own settings, which
  macOS keeps in that app's container: such a token never uses that app, and the read made
  macOS ask whether purlis may "access data from other apps" on every run (#1654). Without a
  token `op` signs in through the 1Password app and reads its settings as before.

**Where purlis looks for `op` and `vault`.** In the `PATH` of the process that reads the vault,
then in the directories installers use under your home (`~/.local/bin` first), then in
Homebrew's and the system's. For a sandboxed chat that process is the app, so a program your
shell finds is found however the app was started, and nothing the chat sets changes where it is
looked for.

**A program where a chat may write is never run as a provider.** That is the project, any
folder you let chats write, the project's cache home, a harness's own folders, the temp
directories, and, for a read the app makes for one chat, that chat's own folder and what its
sandbox lets it write. The file is judged by where it really is, so a link to such a file is
refused too, and purlis runs the file itself, not the link. A copy further along the search
that no chat can write is used. With none, purlis refuses, names the file it passed over, and
stores no pin for it. The rule is the one a harness's program is held to.

**A read with no chat behind it is held to what every chat of the project may write.** Reveal
and Copy in the window, storing a token, and `purlis doctor` are not made for one chat, so
they have no chat's own grants to go by. They refuse a program in the project, in the folders
you let every chat write, in the project's cache home, in a harness's own folders and in the
temp directories. A folder that only one chat was let write is not on that list: a program
there is refused for that chat's own reads, and not for these. So keep the programs your
vaults run where installers put them (`~/.local/bin`, Homebrew's, the system's), and in no
folder you grant a chat. Then every read, whoever asks, judges the same file.

A refusal names every directory searched. A program installed somewhere else is found once a
link to it is in `~/.local/bin`. `purlis doctor` has a row for each such program your vaults
use (`op for vaults`): green where it is found however purlis is started, a warning where only
this `PATH` finds it, where it is missing, and where the only one is where a chat may write.

**Setting up a 1Password vault asks how it signs in, tests it, and keeps the token in the
keyring.** In the app, *New vault* with 1Password chosen is a short set-up:

1. **How purlis signs in.** A *service-account token* (the choice for a vault agents use):
   paste it once into the password box. Or *the 1Password app on this machine*, through its
   command-line integration and its own unlock: choose the account from the ones `op` lists,
   or type its sign-in address (`my.1password.com`, a regional one such as
   `acme.1password.eu`, a company's own). The account is pinned for this machine only. A
   1Password Connect server is not offered: the provider does not support one yet (#1542).
2. **Where the items live.** The 1Password vault, chosen from the ones that sign-in can see
   (typed where it may not list them), and the item purlis keeps this vault's secrets in
   (`charter-<vault>` unless you name another).
3. **Test.** purlis signs in with what you gave and reads that vault's item *names*, never a
   value. Nothing is registered yet. A test that does not pass says why in purlis's own words,
   as one of four kinds: the provider's program is missing, no network or a rate limit, a
   refused sign-in, or something else. What `op` printed is never shown, since it can hold
   what it was given. *Create vault* is offered once a test passed; *Create anyway* is offered
   beside the reason when one did not.
4. **Create** registers the vault and writes its keyring record in one step. If either part
   fails, neither is left: there is never a registered vault whose token is nowhere, and never
   a token nothing refers to.

A vault made this way declares `"token": "keyring"` in the registry and binds **no variable**:
there is nothing to export, in any shell, on this machine. The token is one item in the system
keyring (the macOS Keychain, the Secret Service on Linux) and is nowhere else: not in the
registry files, not in purlis's environment or a chat's, not in a log, an error, a command's
answer or the page. The record that points at the item is in `.charter/vaults.json`, this
machine's half, pinned to the 1Password vault, the item, the account and the `op` found when it
was stored, exactly as a moved token's is. `"token": "keyring"` itself names no secret and may
be committed with `--share`; on a teammate's machine the vault then says it has no token there
until they give theirs.

**From a terminal, the same set-up:**

```bash
purlis vault add team --provider 1password --op-vault Engineering --token-stdin
```

`--token-stdin` asks for the token at a prompt that does not show it, or reads it from standard
input when that is a pipe. The token is never an argument and never a variable named on the
command line. The command runs the same test, and a test that does not pass registers nothing;
then the same one-step create.

On a 1Password vault this machine's half of the registry declares, `--token-stdin` changes its
token and nothing else: `purlis vault add team --provider 1password --token-stdin` keeps the
vault's 1Password vault, item, account and persona, and converts a vault bound to a variable.
A setting given beside it that differs from the vault's is refused, not taken; changing those
is a registration again, with `--force`.

**A vault only the committed `vaults.json` declares is refused there, before anything is
read.** Its record would pin settings a commit chose, and a terminal shows them to nobody.
Give its token in its tab in the app, which shows those settings first. A missing token's
refusal and `purlis doctor` print the terminal command only for a vault this machine declares,
and point to the tab for the others.

**It is refused inside a chat, or a shell the app started, before anything is read**: a chat is
never the one supplying a vault's token. A chat of any project this machine has opened counts,
not only one of the project the command names. The projects are those the machine store
remembers, read both where the command's environment points and under your account's own
home, so a redirected environment does not hide them; of each, only the chats' processes are
read, from a record of any version. On macOS a process a record names counts only if it
started before the record was last written, so a number an app that crashed left behind does not
make your own terminal a chat; and the app that started such a chat counts too, so a process
below it is refused whether or not its own project writes a record (#1542). It is refused too
where purlis cannot tell (one of those
records, or a machine store, cannot be read, or this process's parent or session cannot). The
refusal says how to clear the doubt where there is a way (delete the record named, which the app
writes again, or forget that project under Settings, This machine), and the token can always be
given in the vault's tab instead.

In the app, a token given to the set-up is held only for the window that began it, and goes
when the set-up ends, when that window closes or reloads, and at the latest fifteen minutes
after it was given.

**One token for several vaults.** After a token is given, the set-up lists the *other* vaults
bound to the same identity, each with a tick box and the settings that would be pinned for it
(its 1Password vault, item and account) and which half of the registry names it. A vault the
committed `vaults.json` names starts unticked. Only the vaults you tick are given the token,
each under its own keyring item and its own record, and only while each is still as it was
shown: one whose settings changed in between is skipped and named.

**Changing how a vault signs in.** A 1Password vault's tab has *Change how this vault signs
in*, which opens the same set-up for that vault. This is also how a vault bound to an
environment variable is converted: give the token, test, store, and the variable's binding is
replaced by the keyring record in this machine's half. The next read uses it; nothing is
restarted and nothing is exported. A vault the committed half binds is converted on this
machine alone, and `vaults.json` is left as it is.

**After a conversion, remove the old export.** When no vault reads the variable any more, the
tab (and `vault add --token-stdin`) names it and asks for its `export` line to be taken out of
your shell's startup files. Until it is, every shell started from them, and every program
started from such a shell, still carries the token. (A chat is not one of those: it starts from
an allowlisted environment.) An export is the whole machine's, so a variable a vault of this
project or of any other project this machine opened still reads is never named; where another
project could not be checked, the sentence says it speaks for this project alone. Only a name a
shell can export is ever named.

**An identity read from an environment variable is still supported**, for a machine with no
keyring and for CI, and is no longer what the app sets up:

A vault may declare the identity it is read through — `--env OP_SERVICE_ACCOUNT_TOKEN=<VAR>`
or `--token-env <VAR>` — as NAMES only. If `<VAR>` is unset, purlis refuses rather than read
the vault as whoever the ambient token belongs to, and `secret exec` never hands one vault's
identity variables to a command run for another. The refusal says the ways out in order: put
the token in the keyring from the vault's tab in the app, or export `<VAR>` where purlis runs.

**The provider's program is given only the `OP_*` variables a vault declares** (#1542). purlis
runs `op` with its own environment less every `OP_` variable, of any case, and then adds what
the vault binds. A token, a session (`OP_SESSION_*`), a Connect server or a settings folder
(`OP_CONFIG_DIR`) that your shell exports does not reach it, so a vault with no token of its
own signs in through the 1Password app and never as whoever an exported token belongs to. A
vault that needs one of them declares it, as a name: `--env OP_CONFIG_DIR=<VAR>`.

**The token itself belongs in the keyring, not in a shell.** Open the vault's tab in the app.
It has a box to paste the token into, and it draws that box whether or not the vault could be
read: a vault whose token is nowhere shows the refusal and the box under it, and lists its
secrets by itself once the token is stored. This is the case of an app opened from the Dock,
which never sees what a shell exports. Where the identity variable is set in the app's
environment, the tab also offers **Move the token from purlis's environment**. Either way the
token goes into the system keyring, under an item of its own with a random name (service
`purlis/@identity/<id>`, account the variable's name, `OP_TEAM_TOKEN`), and the vault's
identity is marked as kept there in `.charter/vaults.json`, this machine's half of the
registry. From then on every `purlis secret` command, in a chat or in a plain terminal, reads
that variable from the keyring first and the environment second, so a terminal that exports
nothing still runs `purlis secret exec`. The mark is honoured only in this machine's half: a
committed `vaults.json` cannot tell purlis to hand a keyring item to `op`. `vault list`, the tab
and `purlis doctor` say where each identity is from the mark, without reading the keyring.

**A token is stored per vault: put it in from each vault's tab.** Storing a token marks the
one vault whose tab you are in, however many vaults are read through the same variable; no
vault is given a token you did not put in from its own tab. After a store the tab names the
other vaults read through that variable that still have none, each a link to its tab. Storing
a token again replaces it: the item the old one was kept under is deleted from the keyring
once the new one is in place.

**When a kept token does not read the vault**, the tab says the provider's own reason and
keeps the box. Most reasons are not the token's: a provider's program that is missing or has
moved (storing the token again pins the one found now), no network or a rate limit (read
again), and a refused sign-in, which is the one case the tab says to replace the token for.
A vault read through several variables has no box, since a box stores one token: start
purlis from a shell that exports them and move them from the tab.

**A chat starts from an allowlisted environment, not the app's whole one.** Whatever the app
inherited — from a terminal it was started in, `launchctl setenv`, a login item — reaches a chat
only if it is on the keep-list: what any program needs (`PATH`, `HOME`, `USER`, `LOGNAME`,
`SHELL`, `LANG`/`LC_*`, `TMPDIR`, `SSH_AUTH_SOCK`, `XDG_*`, the proxy variables, purlis's own
`CHARTER_*`), the variables the chat's harness declares for itself (`CLAUDE_*`, `CODEX_*`,
`OPENCODE_*`), and the names you add for the plane in `charter.local.toml`:

```toml
[chat_env]
pass = ["JAVA_HOME", "GO*"]   # a name, or a prefix ending in *
```

Cloud, forge and model-provider credentials (`GITHUB_TOKEN`, `GH_TOKEN`, `AWS_*`,
`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `NPM_TOKEN` and the like), and any name holding `KEY`,
`TOKEN`, `SECRET` or `PASSWORD`, are held back even when a built-in or harness prefix would admit
them. One passes only when you list its exact name there; a prefix you write does not bring a
credential with it. The table is read from `charter.local.toml` only, because a committed file
would let a teammate's push decide what of your machine's environment every chat gets.

Listing a credential there hands it to every chat in the plane, and so to the model's shell.
`purlis secret exec` is the way to give one command a credential.

**No chat the app starts carries an `OP_*` variable, or an identity variable a vault declares**
— not one the app inherited, not one a harness profile's `env` declares, and not one listed in
`[chat_env]`. The extension programs the app runs start from an empty environment too.

### Where the registry lives

`.charter/vaults.json` is this machine's half and `vaults.json` at the plane root is the
committed half; `vault add --share` writes the committed one. They are merged field by field,
this machine's winning, and a 1Password `--account` pin always stays local. A registration
holds names and paths, never a value.

A vault name is letters, digits, `.`, `_` and `-`, starts with a letter or digit, and never
holds `..`; a name that is not one is refused when it is registered and ignored when a
registry is read. A 1Password vault, item or account that starts with `-` is refused, since
`op` would read it as an option.

### The limits, said plainly

- **`--reveal` "to a terminal" means any terminal, including one the agent reads.** A
  pseudo-terminal wrapper (`script`, `unbuffer`, a `pty` module) is a terminal to purlis, and
  whatever it relays reaches the conversation. The Bash guard refuses the flag behind `script`
  and `unbuffer` when purlis is named in the same command; it cannot see every way to make a
  pty.
- **Output masking is best effort.** It replaces the exact text of each value this call
  resolved, and nothing else. A short value — a four-digit PIN, `true` — also matches ordinary
  output and is masked there, while the same value transformed by the command, split across
  writes that the command reorders, or printed in another encoding passes through.
- **A `--dotenv` file is written for dotenv parsers**, the `dotenv` package's rules. It is not
  shell syntax: do not `source` it, where a value's quoting would be read by the shell.
- **A persona is a label, not an access boundary.** `persona secret` picks a vault by the
  active persona's `vault:` field, but any chat can name any vault with `secret` directly, or
  pass `--persona`. Personas decide which vault is the default, not who may read it.
- **Put the token in the Keychain, do not leave it in a shell.** The vault's tab has a box to
  paste the token straight into the keyring; it never touches purlis's own environment. After
  that no chat the app starts is given the vault's identity variable (`OP_*` matched case
  insensitively, and every source name a vault declares, `VAULT_TOKEN` included), so
  `echo $OP_TEAM_TOKEN` there prints nothing. purlis itself reads the token from the keyring for
  every `purlis secret` a chat runs, so an agent can still use the vault — it cannot print the
  token.
- **A move from purlis's own environment leaves the token in the app's process.** The tab also
  offers to move the token an app launched from an exporting shell already carries. That works,
  but a same-user process can read another's environment block (`ps -Eww`, `/proc/<pid>/environ`),
  so until you quit and relaunch purlis from a shell that does not export it — and delete the
  export from your shell's startup files — a chat can still read it from purlis. The tab warns
  while the app's environment still holds one. Pasting into the box avoids this; prefer it.
- **purlis only runs the `op` it pinned when the token was stored.** A keyring-held identity is
  handed to the absolute `op` purlis found at store time, whose code-signing team is
  pinned too; a chat that drops its own `op` on `$PATH` cannot receive the token, and purlis
  refuses rather than fall back to `$PATH`. Re-store the token if you move or reinstall `op`.
  The record that pins it is a file, so every read holds the program to more than the record
  says (#1542): it is named `op`, it is nowhere a chat may write, and on macOS it carries
  1Password's own signature, which `codesign` verifies against the file. A token is not stored
  beside an `op` that fails these, nor where no `op` is found at all: it would be pinned to
  none, and every read refused until it was given again. purlis says so and names every
  directory it searched (#1517).
- **On macOS the two purlis programs are asked for separately.** The token item is written by
  the app, so the first `purlis secret` in a chat that reads it makes the Keychain ask whether
  `purlis` may, and "Always Allow" adds it (ADR 0047). On Linux any process in your session can
  read an unlocked Secret Service collection.
- **Errors never repeat a stored entry.** A reference that does not resolve is named by its
  key, not by what the file holds under it, because that may be a value.
- **The app's Copy puts a value on the system clipboard for up to a minute.** While it is there,
  **any process running as you can read it** — that is what a clipboard is. purlis marks the
  copy so clipboard-history apps skip it and clears it after 60 seconds, but only if the clipboard
  still holds that value; anything you copy in the meantime is left alone. The clear runs in the
  app, so **an app that crashes or is force-quit within the minute leaves the value on the
  clipboard** — it is cleared at a graceful quit, not a kill. And where Apple's **Universal
  Clipboard** is on, macOS may sync the copy to your other Apple devices, which purlis cannot
  reach to clear; turn it off for a machine that copies secrets.

`purlis doctor`'s `vaults` row says, for each vault, whether its credentials are where it says
they are, without reading a value or running a provider's program (#994): a keyring vault's
indexed items are asked for by their attributes, which never makes the keyring ask you, a
plain-file vault's file is looked for and never opened, and a 1Password or reference vault's
program is looked up, its items left to `purlis vault verify`. Its `credential paths` row warns
of a plain-file vault's file that other accounts can read or write, or that is reached through a
link. Its `op for vaults` and `vault for vaults` rows say where each provider's program is found. Its `vault tokens` row says,
for each vault read with a token (through a variable, or kept in the keyring alone), whether the token is marked as kept in the
system keyring, in this environment only, or nowhere. It warns for the last two. It says so
from this machine's record and reads no keyring item, so "marked as kept" is not a read that
succeeded. Run inside a chat it prints no such row: a chat is given no identity variable, so
from there an exported token cannot be told from a missing one.

## Where a vault lives

A plane keeps its state under `.charter/`, and `purlis init` gitignores the whole of
`/.charter/`, which is what keeps a vault out of a commit. Inside it the guards treat these
entries as secret, because their **content** is the secret:

- `.charter/vaults/` — the vault directory and every file under it;
- `.charter/browser…`, `.charter/active-…` and `.charter/fingerprint…` — a browser profile,
  the active-persona marker and the fingerprint key;
- `.charter/` itself, named as a whole.

`.charter/vaults.json` — the registry, which holds provider settings and file paths, never a
value — is an ordinary file and is not refused.

A file in `.charter/vaults/` is plaintext on disk. The guards below are about the
conversation, not about encryption at rest: anyone who can read your account's files can
read that directory.

## What the guards refuse

All of these answer in `charter hook pretooluse`, which the app's plugin runs before each
tool call. [hooks.md](hooks.md) has the whole list and its limits.

- **A reader pointed at a vault path.** `cat`, `less`, `more`, `head`, `tail`, `bat`, `nl`,
  `tac`, `xxd`, `od`, `strings`, `grep`, `rg`, `ag`, `awk` and `sed`, with a guarded path
  as an operand or an input redirection (`< <vault> tee`), behind any wrapper (`env`,
  `sudo`, `purlis secret exec … --`, `{ …; }`, `if …; then …; fi`), after a relocation however it is spelled (`cd`,
  `pushd`, `env -C`, `sudo --chdir`), and on any line of a multi-line command. A wrapper that
  opens a file itself (`xargs -a <vault>`) is a read of that file.
- **The harness's own file tools.** `Read` or `Grep` on a guarded path is refused on the same
  predicate as the shell route — the two do not differ on any spelling.
- **A walk that reaches the vault directory.** `grep -rn TOKEN .` from the plane root names
  no vault file and prints every one of them. The guard resolves the operand against the
  shell's directory and asks whether the walk would reach `.charter/`'s guarded entries; it
  fires only when they exist and hold something. The denial names the fix —
  `grep -rn --exclude-dir=.charter …` or `rg --glob '!.charter' …`. A `Grep` with no path
  walks the directory it stands in and is judged the same way.
- **`--reveal`** on a purlis invocation it can recognise.
- **A write into `.charter/`** with the `Write` or `Edit` tool, inside a plane — that
  directory decides which commands run without a prompt.

Path spellings are folded before they are compared: `.charter//vaults/`,
`.charter/./vaults/` and `.CHARTER/vaults/` are the same file to the guard, as they are to
the filesystem on macOS. `\` is not folded, because on POSIX it is an ordinary filename
character.

**A denial is the guard working, not a bug.** Every denial says so, and says there is no
switch that lifts it; see [hooks.md](hooks.md) → *When a guard is wrong*.

## Where the guard stops

It matches a **known program name** against a **path spelled in the command line, before
any shell runs**. Everything outside that sentence gets through, and it is stated here so
you do not have to discover it:

- *The name.* A program that is not on the reader list runs: an interpreter
  (`python3 -c "print(open('.charter/vaults/db.json').read())"`), `base64`, `cp`, `jq`,
  `cut`, `dd`, `git show`, and a shell string (`sh -c 'cat …'`), which reaches the guard as
  one opaque argument.
- *The path.* A different path holding the same bytes — a symlink, a copy, a vault file kept
  outside `.charter/` — is an ordinary file to every guard.
- *The walk.* Only programs known to walk directories are asked where they go, so
  `find . -type f -exec cat {} +`, `tar cf - .` and an interpreter reach the same files.
- *The shell.* Every expansion is a read the guard does not see: a glob inside the vault
  path (`cat .charter/vault?/db.json`), a variable (`V=…; cat $V`), a quoted
  `"$(cat …)"`, brace or tilde expansion. A glob only escapes when it falls inside the
  guarded part of the path, so `cat .charter/vaults/*.json` is still refused.

It is a guard against mistakes, not against someone deliberately spelling around it.

## A credential in committed text

A plane commits and pushes its memory, so a secret written into one is disclosed the moment
it lands. Two checks look for a credential's **shape** — a JWT, an AWS access key, a
`token: <value>` assignment — and name the kind, never the matched text:

- after a memory or ref file is written, the chat is told to remove it;
- `purlis save` refuses to commit a staged memory or ref file that matches.

A vault **reference** such as `token: "vault:forge/gh"` names where a secret lives and is not
a credential; both checks let it through.

Separately, a forge command that publishes prose (`gh issue create --body "…"`) and purlis's
own text-taking commands (`persona remember`, `workspace remember|note|todo|vision`) refuse a
live command substitution in their text, because inside double quotes a backtick or `$(…)`
runs and can carry your whole environment into a public page. A process substitution (`<(…)`,
`>(…)`, zsh's `=(…)`) is refused on those lines too. Write such text with
`--body-file -` and a **quoted** heredoc (`<<'BODY'`). [hooks.md](hooks.md) has the scope.
