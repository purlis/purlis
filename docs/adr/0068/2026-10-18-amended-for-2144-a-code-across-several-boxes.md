# Amended for #2144, a code across several boxes, 2026-10-18

Spec #2008 rules that what the person typed never reaches the model (the operator's ruling of
2026-10-14). The amendment for #2014, item 4, keeps what a blanked element holds out of every later
answer; the amendment for #2311 says which reads are kept. This note adds what a group of blanked
boxes holds together. Nothing else changes, and the session protocol does not.

1. **Boxes next to each other hold one value.** Blanked boxes next to each other on the page, with
   nothing but separators between them, are a group, such as a code typed one character a box.
   What the group's boxes hold, joined in their order on the page, and for what the person typed
   also in the order they typed into the boxes, is kept out of every later answer as one more
   value, wherever the page shows it with or without spaces or dashes between its characters. It
   is kept wherever what the boxes hold is kept: on that page for an element a login blanked, and
   on every page of the browser for what the person typed. *D-2144-1, decided in implementation,
   to be confirmed.*
2. **A group keeps its latest value.** A read of a group from the boxes themselves is read as a
   field's is (the amendment for #2311, item 1, the person's ruling of 2026-10-17): it drops the
   group's earlier joined values that it contains while they are shorter than 8 characters, and
   nothing of any one box. A read of a group from the page's tree drops nothing. *D-2144-2,
   applies the ruling of 2026-10-17.*
