# Amended 2026-11-05: git is refused a Run grant

The person's ruling on #5265's security review. The reviewed list that gets more than Allow once
still starts with curl alone, and `git` is on the refused list, so it gets no Run grant at all, not
even Allow once: an alias, a hook, `core.sshCommand` or a credential helper in a config file the
chat wrote runs whatever it says, with the value in its environment, and none of that is in the
command the person reads. A chat's `git` with a token is declared as a tool of its persona's, or it
does not run. `screen`, `tmux` and `m4` were refused with it, and a program a link leads to is judged
by the name it resolves to, so `git` under another name is refused too.

Under a grant, a run also starts without the chat's hosts, ports, region and certificate
authorities (`PGHOST`, `MYSQL_HOST`, `REDIS_HOST`, `AWS_REGION`, `SSL_CERT_FILE`, `CURL_CA_BUNDLE`
and the rest that a declared tool's run takes from the chat): the notice shows neither, so where
the run connects and whom it trusts is said in the command, or is the program's default. A
certificate authority set only in the chat's environment does not reach a granted run.

No key a browser login or secret-fill line of any persona holds, or a `mint` line marks, is offered
to a program, through a link either way and in any case. It is judged when the chat asks, when the
person presses, and once they have, since a teammate's line can come in with a pull while the
notice is up. Once Always allow has written its tool line, that line is a declared tool like any
other: a login line added later for the same key doesn't stop it, as it never stopped a tool a
person declared by hand. Removing the tool, or the grant in the Allowed list, does.
