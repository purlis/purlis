# The runner and operator images are public, 2026-11-01

The person ruled on 2026-11-01 (#4498, spec #4469) to publish the runner image and the cluster
operator's image, so a newcomer needs no GitHub sign-in with package access and a cluster needs
no pull secret. This ends the ruling of 2026-10-14 that kept both private. The change of
visibility is the person's to make, and GitHub does not let a public package go private again.

1. **The gate turns round.** `image` pushes only into a package GitHub's packages API shows
   public, and `index` tags and signs only when that still holds and the registry serves both
   images to a puller who has not signed in. Any other answer stops the run, as before. A package
   that is not there stops it too: it would start private, so nothing makes a placeholder any more.
2. **A check from outside.** A job named `outside` in each workflow holds no permission on the
   package and never signs in to the registry. It pulls the image, checks its signature and, for
   the runner image, its provenance, with the commands the docs give. *Decided in implementation,
   to be confirmed* (D-4498-1).
3. **Every version is published.** Visibility belongs to the package, so each earlier image, its
   signatures and its attestations became public with it.
4. **The order.** The gate's change lands right after the packages are made public, not before,
   and needs no setting. Between the two, image jobs fail and a release still ships its desktop,
   with no runner image in its manifest. *Decided in implementation, to be confirmed* (D-4498-2).
5. **A candidate is public from its push.** An image is pushed, pulled back and checked, so one
   that fails stays pullable by its digest, untagged and unsigned. A release names only a digest
   that passed. *Decided in implementation, to be confirmed* (D-4498-3).
