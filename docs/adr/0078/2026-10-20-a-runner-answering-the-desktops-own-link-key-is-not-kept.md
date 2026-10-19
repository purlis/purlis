# A runner answering the desktop's own link key is not kept, 2026-10-20

This note adds to *The desktop keeps a runner's key once and removes it whole* (#2525) and
changes none of its rulings. Its item is **decided in implementation, to be confirmed**
(D-2683-1).

1. **Keeping a runner refuses the desktop's own link key as it refuses the desktop's own device
   id** (#2683). The desktop keeps no link key of its own yet: no store holds one, and the
   desktop's side of a link is given its key by whoever starts the link. So `runners::keep`
   takes the desktop as the pairing presented it, its device id and the public half of the key
   it links with (`Pinned`, the same value `Docker::pair` hands the runner), and refuses a
   runner whose answered key equals that key. Nothing is kept. When the desktop's link-key
   store lands, the key it reads is the one passed here. *Rejected:* waiting for that store,
   since the key a pairing hands the runner is already the desktop's link key for that runner.
   *D-2683-1.*
