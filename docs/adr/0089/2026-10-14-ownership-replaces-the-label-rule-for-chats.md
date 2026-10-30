# Amended 2026-10-14: ownership replaces the label rule for chats, browsers and vault values

**Ruled by the operator on 2026-10-14**, in round 1 of the grill on the browser program (the
session host spec, #1974; the browsers spec, #2008). The text of §2, §6 and §7 above is left as
accepted, and this note amends them. Where it disagrees with them, this note holds.

1. **A device the person owns runs chats and browsers in one session host.** This machine, or a
   runner marked owned, may run chats and browsers side by side in its one session host, with no
   role label between them. It may receive a vault value only with a grant (§7). A runner that is
   not marked owned keeps exactly one role label (§2's table) and never receives a vault value,
   granted or not.
2. **§6, browser logins.** A browser on a device the person owns may be signed in from a vault.
   purlis performs the login as the browser's driver, and the value goes from the vault to the
   session host and the browser, never to the chat or its tools. A browser on a runner not marked
   owned is never signed in from a vault.
3. **§7, who receives a value.** "A `browser` runner never gets a value" now reads: a runner not
   marked owned never gets a value. An owned device gets one only through a grant, by §7's other
   rules.
4. **To be confirmed:** where "owned" is set for a runner. §4 holds it per provider, as the
   operator's setting, with not owned as the default; whether a single runner may also be marked
   owned, and where that mark is kept and shown, is left to the runner placement spec (#2041),
   and §4 stands until then. **Built for cluster pools by #4071** (the person's ruling of
   2026-10-30): the mark is per pool, the person's alone, kept as `owned` in `runners.json`
   (amendment of 2026-10-26, item 3).
