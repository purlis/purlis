# Amended 2026-11-08: a link is judged when it is read

The person's ruling on #5795, which follows
[a link is one every persona of its vault may read](2026-11-08-a-link-is-one-every-persona-of-its-vault-may-read.md).
That amendment judged a link only when it was made, at the press, so a persona that came to use the
link's vault later still read through it.

A link is now also judged each time it is read. A persona's read through a link into a vault it
may not use is refused before any value is read. The vaults it may use are the ones `authorise`
opens for it. The readers judged are:

- a declared tool's run, a GitHub App's key read to mint with included;
- a run under a Run grant, as before;
- a browser fill;
- a login, declared or granted from its notice;
- a chat profile's sign-in credential;
- a runner chat's tool, read on the desktop.

A fill, a granted login and a profile's sign-in each count as a reader of their vault, though none
of them needs the vault to be one their persona may use.

So it no longer matters when the link was made or how the persona came to use its vault: by a tag,
an Allow (a refused vault's, or Always allow's), or a policy lock lifted. A link made before #5745's
press check is judged as well. The press check stays, so a link no persona of its vault may read
through is still refused up front.

It fails closed. A link that can't be read is refused. So is this machine's grants file where it
is there and can't be read, which counts as a refusal here and not as no grants. The chat hears
which key, in which of its own vaults, reads an entry of a vault its persona may not use. It never
hears the other vault's name (D-5246-8). Nothing is asked of the person, because a refusal of this
kind raises no Secret ask.
