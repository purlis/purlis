---
name: purlis-browser
description: When a purlis chat must log in to a website with a vault login, or needs several logged-in browsers, drive a real browser where purlis fills the password unseen. Use when asked to log in, sign in, or test behind auth. Not for an API's credential (purlis-secrets) or theming (purlis-theme).
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
`browser_close` ends one and everything it held. While the person has taken a browser over it stays
open: the answer says so, and you close it once they hand it back.

## Files: downloads and uploads

A download lands where no chat reaches it; each answer says new or changed downloads by name
(`d1`), size and origin, and `browser_downloads` lists them all. `browser_save_download` saves a
finished one into your working folder as a new file, once the person has said yes for its
origin. `browser_upload` puts files of your working folder (paths relative to it) into a page's
file input. Only files you may read are taken, and no link out of the folder is followed.

## Credentials: never typed, never printed

**In a chat purlis started, log in with `browser_login`, and call it first.** Don't ask for the
vault or list its keys beforehand: that raises a second notice. If your persona has no login for
the site, call `browser_login` with a short new `login` name, `site` (the login page's https
address), `vault` and `key`, plus `user_key` if the page asks for a user name. Give the site's own
address even where it signs in on another host: the page is followed to its login form, and the
person is shown both hosts. The person sees one notice and confirms the site, which they may change. If a
login your persona declares is on the wrong site, call `browser_login` again with its name, its
`vault` and keys, and the right `site`: the person is asked whether to replace the recorded one.
Your call waits for their answer; on Allow it goes on to log in. Fill in only what you know, never
guess a key name, and if they keep it blocked, tell them what needs the login and don't suggest it
again.

A project may also declare a login once, in its committed settings, and the person allows it on
their machine with the persona's hosts:

```toml
[sandbox.personas.qa]
tools = { shop-login = { vault = "qa", browser = "https://shop.example.test/login",
                         user = "username", key = "password" } }
```

Then call the `browser_login` tool with `login = "shop-login"` (and `browser` to pick one of
your browsers). purlis goes to the declared page in your browser, fills the login from the vault
only while the page is on the login's origin, submits, waits for the password field to go, and
hands the browser back logged in. You never see the login: it is kept out of every snapshot and
screenshot, and the fields purlis filled are blanked. From then on that browser reaches the
login's site alone; call `browser_new` for any other site. Where it signs in on another host,
the browser keeps reaching that host too, so the site's own redirects and token refreshes work,
but you may read or act on that host's pages only while it signs in: until the browser is back
on the login's page, for two minutes at most, and only on the page the login ran on and the
windows it opens. Another tab stays kept off that host. A step that host asks for after the password is
yours or the person's to take in that time. Outside it, a call on that host's page is refused, and
where the page asks to sign in again, call `browser_login` again. To start an OAuth or OpenID
Connect sign-in with your own request, give `start` the authorize address, its whole query as
you write it: it must be on the login's site or the host it signs in on, and any `redirect_uri`
must lead back to the login's site. The login goes into the browser
you name, or the one you're using. A browser that holds it, on a page of its site that asks for no sign-in, answers "already logged
in" and nothing is filled. Where the page asks, or you're elsewhere, purlis looks on a page of its
own, and may fill again. Every login that fills starts clean on its own site: what that site
stored in the browser goes first, and other sites are left as they are. A login kept for later
reuse opens in a browser of its own, and so does one whose form doesn't appear on the clean load:
the answer says why and which browser it left, and you close that one with `browser_close`
if nothing in it is needed. A browser you opened with `browser_new` and never used is closed for you,
and the answer says so. A project's browsers have a limit: the refusal lists yours, with what each
holds, so you can close the right one. If the login is refused, tell the person the sentence: it names what to declare or
allow. A page with a captcha or a second factor does not log in this way; ask the person, or hand
them the browser with `ask_operator` pointed at it.

Running the login's name as a command does nothing: it says to use `browser_login`.

**Any other secret goes in with `browser_fill_secret`**: an API key into a dashboard's settings,
a token into a deploy form. Take a snapshot, then call it with `target` (the field's reference,
`e12`), `vault` and `key` (the value's name, never the value). The person is asked on a notice
that names the key and the exact site the field is on; your call waits for their answer. Once
they allowed it for the chat or always, a one-line field (password, text, email, number, tel,
url) on that site takes it without asking; a multi-line text box or an editable area asks them
each time. purlis fills only where the field's own frame is still on that site when it fills, so
a field in a frame from another site is refused. A key the vault doesn't hold is asked of them
too, and your call waits for it. The value never reaches you: every snapshot, find, wait and screenshot of that
browser leaves it out, and from then on that browser copies, cuts and drags nothing for you. If they
keep it blocked, tell them what needs it and don't ask again. While the page you filled stays
open, the browser reaches only that site: an address for another is refused, saying so. Nor do
you press any key in that page, or type or choose anything in it, so type the other fields (a user name) before you fill, and send the form by clicking its button,
not with Enter. The form may go on to another site only by a redirect from that site's own
server: to the site that began the way to this sign-in (the address you opened, or what it
redirected from) it goes by itself, and any other asks the person once, naming the site. A redirect
whose address holds the value is refused, and a form that posts straight
to another site is refused unless the person approved the key for that site too; the answer says
how. Once that page loads another page, an error page, or you close it, the browser reaches and
takes keys as before. A fill is refused while a window on that site opened the page or was opened
by it, so open the form with `browser_open`, in a page of its own. Any other window related that
way closes when the page is done, and the browser reaches as before once it has. The hold can't
stop a script on the approved site using that site's own storage or workers: it can keep what it
reads there and send it from that site later. It works the
same on a pool's runner.

**A 2FA code comes from its seed**: name the key that holds the seed (an `otpauth://totp/`
address or the base32 secret) and set `code` to true. purlis works the current code out from
the seed as it fills, and only the code goes into the page; the seed never does. A seed asked for
without `code` is refused. So is any value that looks like one, unless the person marked its key
as no 2FA seed in the vault's tab, and a chat can't make that mark. Pass on what the refusal says.

**In the person's own terminal**, `purlis secret exec --dotenv` resolves vault keys into one 0600
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
  chat, or the bridge in the person's terminal.
- Never commit a session directory, a storage-state file or a trace. Each one holds live
  cookies or authenticated traffic, which is the credential in another form.
