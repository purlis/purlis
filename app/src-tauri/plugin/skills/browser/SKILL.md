---
name: browser
description: Drive a real browser for a task that needs one — a web login, an end-to-end check, verifying a UI, reaching something behind auth — with credentials pulled from a vault so purlis hands the password to the browser instead of you typing it into the conversation. Use when browser automation needs to authenticate, or when several workers each need their own logged-in session.
---

# Driving a browser with vault credentials

**In a chat purlis started, use purlis's own browser tools.** They drive a real browser the
person can watch and take over: `browser_open` and `browser_navigate`, `browser_snapshot` and
`browser_find` for a page as a tree of elements with references, actions on those references
(`browser_click`, `browser_type`, `browser_press`, …), and `browser_screenshot`. No tool runs
script. The first browser tool asks the person what the browser may reach.

## One browser per worker

Each browser has its own cookies, storage and pages, so workers that must be logged in as
different users each use one of their own: `browser_new` opens another, and the tools use it
from then on (pass `browser` to pick one by the name a tool answered with, `b1`, `b2`).
`browser_close` ends one and everything it held.

## Files: downloads and uploads

A download lands where no chat reaches it; each answer says new or changed downloads by name
(`d1`), size and origin, and `browser_downloads` lists them all. `browser_save_download` saves a
finished one into your working folder as a new file, once the person has said yes for its
origin. `browser_upload` puts files of your working folder (paths relative to it) into a page's
file input. Only files you may read are taken, and no link out of the folder is followed.

## Credentials: never typed, never printed

**In a chat purlis started, log in with your persona's browser login, through purlis's own
browser tools.** A project declares a login once, in its committed settings, and the user allows
it on their machine with the persona's hosts:

```toml
[sandbox.personas.qa]
tools = { shop-login = { vault = "qa", browser = "https://shop.example.test/login",
                         user = "username", key = "password" } }
```

Then call the `browser_login` tool with `login = "shop-login"` (and `browser` to pick one of
your browsers). purlis goes to the declared page in your browser, fills the login from the vault
only while the page is on the declared origin, submits, waits for the password field to go, and
hands the browser back logged in. You never see the login: it is kept out of every snapshot and
screenshot, and the fields purlis filled are blanked. From then on that browser reaches the
login's site alone; call `browser_new` for any other site. You choose which declared login, and
nothing else. If the login is refused, tell the user the sentence: it names what to declare or
allow. A page with a captcha or a second factor, or one that first sends the browser to a
sign-in page on another site, does not log in this way; ask the user, or hand them the browser
with `ask_operator` pointed at it.

Running the login's name as a command does nothing: it says to use `browser_login`.

**In the user's own terminal**, `purlis secret exec --dotenv` resolves vault keys into one 0600
temp file and points `PLAYWRIGHT_MCP_SECRETS_FILE` at it. You then refer to a secret **by
name**. Playwright substitutes the value and scrubs it from the output it captures.

That is a net, not a boundary. A step that transforms or forwards the value is not scrubbed:
a screenshot, an `eval`, a POST. The credential goes wherever you send it.

Four rules for the terminal flow:

- **Keep Playwright's output out of the project.** `playwright-cli` writes its snapshots, logs
  and traces under `.playwright-cli/` in the folder it runs from, and a trace of a logged-in
  run holds that run's cookies. A project's autosave commits what the project does not ignore.
  So the flow makes a private folder outside the project with `mktemp -d`, stops if it could
  not, and points `PLAYWRIGHT_MCP_OUTPUT_DIR` at it before `open`. `playwright-cli` reads that
  variable when the session starts, as it reads the secrets file. When the flow exits, however
  it exits, it closes the session and removes the folder. Do the run's work inside the flow.

- **Open the session inside the bridge.** `playwright-cli` reads
  `PLAYWRIGHT_MCP_SECRETS_FILE` once, when the session starts. Setting it later, on a `fill`
  against a session that is already open, fails silently: the literal string `PASS` is typed
  into the field. Wrap the whole flow in one `purlis secret exec`, from `open` through the
  last `fill`.
- **Wait for each field before filling it.** `open` returns before an identity provider's
  redirect lands. A `fill` that runs too early fails with "does not match any elements",
  which looks like a wrong selector.
- **`not open` is not a cue to re-open.** A second `open` with the same `-s=<name>` starts a
  new browser and orphans the logged-in one. The dotenv file is gone once `purlis secret
  exec` exits, so the new browser cannot be filled either. Re-run the whole flow instead.

```bash
purlis secret exec <vault> \
  --dotenv PLAYWRIGHT_MCP_SECRETS_FILE=USER:<user-key> \
  --dotenv PLAYWRIGHT_MCP_SECRETS_FILE=PASS:<pass-key> \
  -- bash -c '
    P="npx @playwright/cli@<version> -s=owner"
    OUT=$(mktemp -d) || exit 1
    cleanup() { $P close >/dev/null 2>&1; rm -rf -- "$OUT"; }
    trap cleanup EXIT
    export PLAYWRIGHT_MCP_OUTPUT_DIR="$OUT"
    wait_for() {
      for _ in $(seq 1 15); do
        $P eval "() => !!document.querySelector(\"$1\")" 2>/dev/null | grep -q true && return 0
        sleep 2
      done
      echo "TIMEOUT waiting for $1" >&2; return 1
    }
    $P open https://example.test/
    wait_for "#username" || exit 1
    $P fill "#username" USER
    wait_for "#password" || exit 1
    $P fill "#password" PASS
    $P click "button[type=submit]"
    $P snapshot
  '
```

To read the **active persona's** vault instead of naming one, use `purlis persona secret
exec`. A different fixture account is a different pair of keys, not a different mechanism.

Reading a token *out of* a logged-in session through the vault (a `browser://` reference)
is not in this version yet. Do not stand in for it with `TOKEN=$(… cookie-get …)`: that is
command substitution into the transcript, with nothing to redact it.

## Hard rules

- **Never read a filled secret back.** Do not evaluate `el.value`, take a screenshot of a
  filled password field, or dump the dotenv file.
- Never type a credential in directly, even "just once to test". Use `browser_login` in a
  chat, or the bridge in the user's terminal.
- Never commit a session directory, a storage-state file or a trace. Each one holds live
  cookies or authenticated traffic, which is the credential in another form.
