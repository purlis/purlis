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

A fill and a granted login each count as a reader of their vault, though neither needs the vault
to be one its persona may use. A link to another entry of the same vault reaches no new vault, so
it is not judged, as the press does not judge it.

A run's keys are all judged before anything is read or asked of the person. A key out of reach is
never offered on the run's notice, and the run is refused before the person is asked to fill
another key it lacks.

A link is judged again once its value is read, so a link written between the first judgement and
the read is seen, and what was read goes unused.

So it no longer matters when the link was made or how the persona came to use its vault: by a tag,
an Allow (a refused vault's, or Always allow's), or a policy lock lifted. A link made before #5745's
press check is judged as well. The press check stays, so a link no persona of its vault may read
through is still refused up front.

It fails closed. A link that can't be read is refused. So is this machine's grants file where it
is there and can't be read, an empty file included, which counts as a refusal here and not as no
grants. Why it can't be read goes to purlis's log, since the file holds other vaults' names. The chat hears
which key, in which of its own vaults, reads an entry of a vault its persona may not use. It never
hears the other vault's name (D-5246-8). Nothing is asked of the person, because a refusal of this
kind raises no Secret ask.
