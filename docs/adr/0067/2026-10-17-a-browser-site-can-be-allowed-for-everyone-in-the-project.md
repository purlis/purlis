# Amended (2026-10-17, the operator's ruling on #2443): a browser site can be allowed for everyone in the project

Under the ruling of 2026-10-17 on #2018 (a browser site's Allow is for the browser alone, with the
same scopes as host grants) and the operator's ruling of 2026-10-17 on #2443 (the committed key
is `[sandbox].browser-sites`, a list beside `hosts` in the same entry shape; a site a teammate adds
is shown once in a Notice, per site, and recorded per machine as `hosts_seen` is), item 3 of the
amendment *a browser's new sites are judged by its driver* gains its third scope.

1. **Everyone in the project.** A browser's site ask offers *Allow for everyone in this project*
   beside this chat and this project on this machine, unless an administrator's policy leaves
   that scope out. The Allow writes the site to `[sandbox].browser-sites` in the committed file,
   so it travels with the project, and records it as seen on the machine that wrote it.
2. **For browsers alone.** The browser's judge and its reach proxy read the committed sites, at
   the project's layer of the browser's lane. A chat's own commands never do: they still need an
   Allow of their own for that host, and `[sandbox].hosts` never holds a browser site.
3. **Shown once before it applies here.** A site a teammate's change adds reaches no browser on
   a machine until the person there read the Notice of the project's browser sites, which names
   each site added and each taken out; Got it records each one it named (`browser_sites_seen` in
   this machine's sandbox-grants file). Each site is recorded on its own, so one added later is
   told alone, and one taken out is forgotten, so putting it back is told again. Where policy
   leaves out everyone in the project, or never allows a site's host, nothing of it is told and
   none of it reaches a browser.
4. **A row of its own in Granted.** Each committed site is listed under Everyone in the project,
   as the browsers' alone, with who committed it; one not shown on this machine yet is listed as
   waiting. Remove takes it out of the committed file and ends the browsers of every chat of the
   project on this machine, as removing a browser site kept on this machine does.
