# Amended 2026-10-31: a profile over its pool's limit is refused, and offers to raise it

Amends the 2026-10-30 amendment's point that a profile over the overall limit leaves whole skills
behind and starts. The person's rulings on a pool's limits (spec #4275, Q2, Q5, Q9) replace it.

1. **Nothing stays behind.** Over a limit nothing is sent and the start is refused, saying what is
   needed and a value to raise the limit to, which the person can edit and apply.
2. **The limit is the pool's** (`profileSize`, `profileFiles`), 128 MiB and 8,192 files by default,
   as the overall limit was. Files are still read as an import reads them and sent without
   credentials, in batches the person never sees.
