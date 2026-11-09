# Amended 2026-11-09: turning a Pool's minting on takes use on its key

*Decided in implementation, taken as the recommended option under the operator's standing
instruction to build recommended options.* It narrows item 3 of the amendment of 2026-10-31 *the
operator mints for a vouched runner*, where changing the image of a Pool that mints takes `use` on
its key.

1. **Turning a Pool's minting on is held exactly as changing the image of a Pool that mints.** The
   rule in `purlis-forge` now asks `use` on the key the Pool names after the write whenever that
   Pool mints and either its image changed or it did not mint before. It no longer looks only at
   a Pool that minted before the write.
2. **Why.** The image holds the link keys the operator vouches for. Turning minting on adopts the
   image the Pool already names, which anyone who may write a Pool that does not mint could have
   set, so it is spending the key in the same way. Writing `spec.forge` already takes `use` on
   the key; this rule says it of the image itself, so it holds whatever else the write does.
3. **The key asked is the one the Pool mints with after the write.** A write that moves a minting
   Pool to another key and changes its image asks `use` on the new key, which is the one spent;
   the old one is no longer the Pool's.
