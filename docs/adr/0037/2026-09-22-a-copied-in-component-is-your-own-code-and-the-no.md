# Amendment, 2026-09-22: a copied-in component is your own code, and the no-wrapper rule was never about that

The agent that built the theme system (purlis#144) hit this record's rule against the thing
the operator now wants, and left the conflict on the table rather than deciding it. The operator
decided it on 2026-09-22: **shadcn/ui components may be copied into purlis**, and this record
is amended to say what its rule actually meant.

**First, where the rule that conflicts is written, because it is not here.** The words are
purlis's, in `docs/ui-primitives.md`:

> **Do not write a wrapper layer around them.** No `<Modal>`, no `<Field>`, no house component
> library. The primitive is the component; purlis's look is CSS on it. A wrapper is the custom
> tooling this repo's first rule exists to prevent, and it is how a primitives migration turns
> back into hand-rolled markup with extra steps.

This record never says "wrapper". What it says is that behaviour comes from a headless primitive
library and appearance is purlis's own CSS, and it rejects *"a headless library for the look as
well — an unstyled component set plus a token system imported from it"*. The no-wrapper rule is
the code-side expression of that, written where an implementer reads it, and the two are read
together as one rule. **Which is why the amendment belongs here**: a code-side file cannot loosen
a decision this sequence made, and a decision this sequence makes has to arrive in the file the
implementer actually reads. Both are changed, in that order, and this record is the authority.

**What the conflict is.** A shadcn/ui component *is* a thin wrapper around a Radix primitive —
that is the whole shape of the thing. So the rule as written forbids it, and the theme-system
agent read the rule correctly. It took shadcn's conventions and none of its components: `cn` at
shadcn's address (`app/src/lib/utils.ts`) with shadcn's two dependencies, zero components copied,
and the conflict recorded in `docs/design-system.md` for the operator rather than resolved by an
agent. That was the right call and this amendment is the answer to it.

**What the rule was written against, which is a different thing wearing the same word.** A
component library is **a dependency that owns your markup**: an upstream you cannot edit, an API
you are stuck with, a look you fight on every surface, and a version bump that changes your window
without touching your diff. Every argument above — *Why not Material*, argument 1 in particular —
is an argument against *that*. None of it is an argument against source code sitting in
`app/src/`.

**A copied-in component is purlis's own code in purlis's own repo, editable line by line.** It
arrives in a diff a reviewer reads, it changes only when somebody changes it, its props are on the
page, and it has no upstream to fight because it has no upstream at all. Calling it and a
dependency by one name is the conflation this record made, and it is the conflation this
amendment removes.

**The amended rule, in one line: no library between you and the primitive, and no indirection you
cannot read.** Copied-in source is neither, and it is allowed.

## What is now allowed

- **Copying a shadcn/ui component's source into the repo**, at shadcn's address
  (`app/src/components/ui/`), and editing it. `cn` is already there and already justified;
  `class-variance-authority` arrives with the first component that has variants.
- **A copied file re-exporting a primitive's parts under their own names** —
  `Dialog`, `DialogContent`, `DialogTitle` over `@radix-ui/react-dialog`. That is a spelling of
  the primitive, not a layer over it: every part is still a part, every prop still lands on the
  primitive, and the file that does it is one `Cmd-click` away.
- **Editing the copy freely.** This is the condition, not a permission. A copy kept pristine
  "because upstream will fix it" is a dependency with worse ergonomics and no version — there is
  no upstream once it is copied, and a file nobody will edit should have been an import.

## What is still refused, and this is the part to write carefully

- **A component library as a dependency.** MUI, Chakra, Mantine, Ant, and equally any published
  package of shadcn-shaped components. Argument 1 above decides this and the amendment does not
  reach it. The `@radix-ui/*` packages are not this: they ship behaviour and no markup you have to
  keep.
- **A house abstraction layer, whether written or copied.** `<ConfirmModal open onConfirm>` is
  refused. A purlis API in front of Radix is refused *because it is a purlis API*, not because
  of where the file came from — copying it from shadcn would not launder it. The test is at the
  **call site**: can the next person see which primitive this is and reach its props? If the
  answer needs the wrapper's source and then the wrapper's own decisions, it is the layer this
  record refuses.
- **The look still does not come from a package.** A shadcn component arrives wearing Tailwind
  utility classes from shadcn's own token set — `bg-background`, `text-foreground`,
  `bg-destructive` — and every one of them has to be renamed to purlis's vocabulary by hand.
  *"What is not taken: colour, type, spacing, density, elevation, motion, iconography"* is
  unchanged by this amendment and is the clause a paste is most likely to break.

  **And this is the cost the amendment actually adds, so it is stated rather than waved at: the
  build does not catch it.** purlis deletes Tailwind's palette (`--color-*: initial`), which
  means a class naming a colour it does not have emits **no CSS at all** — it does not fail,
  it disappears. `literals.test.ts` catches a hex literal and an arbitrary value (`bg-[#fff]`)
  and would catch those; it cannot catch a class that simply does not exist, and
  `tailwind.test.ts` checks that the palette is gone rather than that a source file avoided it.
  The failure mode of a missed rename is **an element rendered undressed**, which is
  `claudeclaudeclaudebuilt-indefault`, which is the defect at the top of this record. A paste is
  therefore reviewed against `app/src/styles.css`'s `@theme` block and looked at running, and
  `docs/design-system.md` says so where an implementer will read it. Nothing mechanical closes
  this, and pretending otherwise would be the version of this amendment that gets somebody
  burned.
- **Copying components nothing renders.** Unchanged, and it is `docs/design-system.md`'s existing
  reason: a copied component that nothing uses is dead code in purlis's tree, which is worse than
  an unused dependency because it looks maintained.

## What this costs

- **A copy does not get upstream's fixes, including its accessibility fixes.** This is the real
  price of the amendment, and it is the mirror image of the benefit: the reason nothing changes
  under you is the reason nothing improves under you either. The primitive underneath still
  updates with its package; the markup and the classes on top do not.
- **The trap named above gets a second edge.** *"We style it ourselves" must not become "we build
  the controls ourselves"* — and a file that is purlis's to edit is a file that can be edited
  until it is no longer the primitive's behaviour. The test in that section is unchanged and now
  has to be applied to the copies too: the next control that needs focus, arrows or a menu is not
  hand-written, and a copied component that has had its primitive edited out of it is
  hand-written.
- **Provenance is the copying PR's to answer.** A copied file carries somebody else's licence and
  no dependency manifest records it. The PR that copies one says where it came from and at what
  version, in the file, or purlis has vendored code it cannot account for.

## Where each rule now lives

This record decides the rule. `docs/ui-primitives.md` and `docs/design-system.md` in
`diazoxide/charter` are its code-side expression and are updated to match; where they and this
record disagree, **this record is authoritative** and the code-side file is the defect.
