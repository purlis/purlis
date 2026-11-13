# Amended (2026-11-12, #5242): an Allow once whose fill never went in carries its retry

Builds on the amendment of 2026-11-04 on secret fill's grants.

*The case:* the person presses Allow once on a fill's notice, and the vault holds no such key, so
the fill goes to the Secret ask (ADR 0109). The chat's call ends before they add it, because the
harness gave up on the call or the wait ran out. The Allow once was used up at the notice, so the
chat's retry of the same fill asked the person the same question again. Each retry asked for the
Allow again before the key, which could repeat without end.

*Decided:* the person's Allow on that notice stands for the retry of exactly that fill. It is
kept only where the call ended or stopped waiting while the key was asked for. It is not kept
where the key was declined, refused or added under another name, nor where any check refused
the fill. It is bound to:

- the chat, by its number and id;
- the runner the browser runs on, or none;
- the digest of the notice the person pressed on (`FillWait::digest`): browser, vault, key,
  origin, page, label, warnings, persona and every mark the notice drew.

A retry whose notice would be drawn differently in any of these is asked about as before. The
retry takes the kept Allow as it starts, so one call alone uses it. The retry passes every check
a fresh press would, as the project reads at that moment: the fill is judged again and policy
must still let Allow once be. The kept Allow lives in memory and goes when the chat closes.

*Why it is no wider:* it covers one fill the person said yes to, which never went in. It is
still one fill of one value into one field on one origin. A Run grant's Allow once is held the
same way (ADR 0112): it is used as its run starts, not at the gate.
