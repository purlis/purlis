# Built: the account store and a token sign-in (#1814, 2026-10-12)

The PAT half of 0077 §4 and the store of §6–§8 are written, ahead of the browser flows (the
onboarding spec #1810's ship order). Within this record, these were settled while building it:

- **The keyring item is `purlis/@forge/<host>/<id>`**, the rename's spelling of
  `charter/@forge/…` (V93l). No item was ever written under the old spelling, so none is read.
- **A token's scopes:** a GitHub classic token needs `repo` and `read:org` (a wider
  organisation scope grants `read:org`); a GitLab token needs `api`, the one scope of §2, which
  covers git over HTTPS as well. A GitHub fine-grained token, whose permissions GitHub does not
  report, is taken with its scopes unknown, as §4 accepts it.
- **Sign-out does not revoke a pasted token.** §3's revocation is of a token purlis's own flow
  minted; a token the person made may be theirs for other uses. The page says it still works at
  the forge until they delete it there.
- **A host is a full host name**, with a dot. A single word is far more often a token pasted
  into the wrong box, and a token must never be looked up as a host name.
- **The three audit actions of "ADR 0075, amended"** are left to FW-14, as "What changes where"
  says.
