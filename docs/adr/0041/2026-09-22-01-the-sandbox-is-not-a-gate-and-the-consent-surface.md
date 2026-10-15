# Amendment, 2026-09-22: the sandbox is not a gate, and the consent surface carries what that costs

**The operator ruled on 2026-09-22 to ship the subprocess runtime with no OS sandbox.** He was
shown the honesty paragraph under decision 1 first — that a subprocess runs as his user, with his
filesystem, and can read `.charter/vaults/`, write the machine store and edit `charter.local.toml`
without asking purlis for anything — and chose to ship anyway. His words:

> *"Ship the subprocess runtime as ADR 0041 recommends — for pure plugin system, but in future we
> can control it on level of future marketplace, so some checks can restrict plugins to have bad
> code inside, so un-trusted source plugins just can add some warning in charter — charter can
> announce that you are using plugin from not trusted sources. But this is all for future, now
> clean pure plugin system without sandboxes."*

So: **a subprocess over a unix socket, no OS sandbox, and marketplace vetting with
untrusted-source warnings deferred.** This is not re-litigated, and an implementer who reads this
amendment and builds something narrower has not followed it either.

**This removes no gate item, because the sandbox was never one.** Item 7 asked for the
confinement to be *decided* — "a sandbox, or explicitly not one, in writing" — and this is the
writing. What it removes is a reading this record invited: three paragraphs before the gate it
says *"real confinement is a sandbox … until one exists, a plugin is trusted code that charter is
polite to"*, and a reader could take that as a runtime waiting on one. It is not. The item is met
and the runtime may be built.

**Item 7's conditional half has therefore fired, and is discharged here.** "If not, ADR 0028 is
re-opened in the same PR." It is, in the same change as this amendment: 0028's own record now
carries the note that its re-opening clause has been triggered and what the answer to it is. That
clause exists because 0028 accepts a `stat`-then-open race *only* on the grounds that nothing in
purlis makes an agent less privileged than purlis itself — and the whole proposition of a plugin
boundary is that a plugin is less privileged. 0028's answer, written there, is the same one as
here: with no sandbox, a plugin is **not** less privileged, so the ground 0028 stands on has not
moved. That is a reprieve and not a resolution, and the day a sandbox lands the clause fires for
real.

**What the ruling costs, stated as plainly as the paragraph it overrode.** None of this is new
risk that the ruling created; it is the risk the record already described, now accepted rather
than deferred.

- A plugin can do everything the operator can, and the capability table in decision 2 describes
  **purlis's own conduct** and not a cage. Every "No, at any level" in it is a promise about what
  purlis will not do on a plugin's behalf, and none of them is an obstacle to a plugin doing it
  itself.
- The install-time decision carries the entire weight, which decision 3 already said and which is
  now the *final* answer rather than the interim one.
- The three named sandboxes — Seatbelt, seccomp or Landlock, AppContainer — remain uncosted and
  unbuilt, and each stays a platform-specific piece of work with ADR 0031's refuse-rather-than-
  degrade rule waiting on the other side of it.

**What the deferred answer is, and what it is not.** Marketplace vetting and an untrusted-source
warning are a *label on a supply chain*. A warning tells an operator where something came from; it
does not bound what the thing does once it is running, and no amount of it turns the table in
decision 2 into a cage. Vetting scales with reviewers and purlis has one operator, which is the
same argument the "What was rejected" section already makes against a registry. Both are worth
building and neither is a substitute for the sandbox, so neither is written here as though it
were. When they are built they get their own record and their own honest limits.

**What this obliges the first implementer to do, and it is the part most likely to be skipped.**
Because the table is conduct and not a cage, **the consent surface has to say exactly that.** A
prompt listing *this plugin may: contribute a theme, add one palette command*, while the plugin
can in fact read the operator's vaults, is worse than no prompt — it manufactures confidence
purlis cannot back, and a surface that over-promises is one the operator stops reading and then
trusts anyway. The prompt must say, in purlis's own plain voice, that **a plugin runs with the
operator's own access**, and that what purlis shows is what the plugin **declares** and not what
it is **limited to**. That sentence belongs in the core, beside the trust record, pinned by a
test, and carried to whatever draws it — not composed in the dialog, where it would drift kinder
than the truth one edit at a time.

**What is still unmet.** Items 2, 3, 4, 5, 6 and 8 stand exactly as written. In particular the
tool guard is still one blanket `exit 2` with three of six stages wired to nothing, and nothing in
this ruling touches that: the irony this record opens with is unchanged, and a plugin is still a
second door beside the one being built.
