# Amended 2026-10-21: a pool's follow leaves no runner behind

**Built by #2869** on item 5 of the ruling of 2026-10-20 for #2749. What follows was decided in
#2869 and is marked *decided in implementation, to be confirmed*.

1. **A follow first removes what an earlier one left of its runner.** Before it makes the
   runner `<pool>-<8 hex>`, under the lock of `runners.json`, a follow removes a container of
   exactly that runner's name that carries the runner label of that name, and the home volume of
   exactly its name. It does this only where no kept runner and no pool holds the name, so what a
   kept runner claims is never touched. A follow that stopped after Docker made the runner and
   before it was kept therefore no longer holds the pool on its old runner for good. *Decided in
   implementation, to be confirmed* (D-2869-1).
2. **The volume is removed by its name alone.** Docker makes a runner's home volume as it starts
   the container, with no label, so a volume named `purlis-runner-<name>-home` that no kept
   runner claims is taken as one purlis made. *Decided in implementation, to be confirmed*
   (D-2869-2).
