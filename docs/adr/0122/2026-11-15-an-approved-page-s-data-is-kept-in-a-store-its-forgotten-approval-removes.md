# An approved page's data is kept in a store of its approval's own, removed when it is forgotten

**2026-11-15, #6697.** Amends the amendment of 2026-11-10 that keeps an approved page's storage
in an origin of its approval's own: its bullet "What an old origin kept stays on disk,
unreachable".

That amendment left what an approved page kept on disk once its approval was forgotten, because
clearing one origin's website data needed an API Tauri did not expose, or `unsafe` on macOS.
Tauri 2.12 exposes both halves without `unsafe`: `WebviewBuilder::data_store_identifier` lays a
webview in a WebKit data store of its own, and `AppHandle::remove_data_store` removes one.

- **On macOS 14 and later an approved page is laid in a data store of its approval's own**,
  named by the 16 bytes of the host its approval serves it from
  (`purlis_core::artifact::approval::page_host`). The origin stays as the earlier amendment drew
  it, its random `storage` included; the store only follows it. A page whose script does not
  run keeps nothing and is laid in the window's own store, and so is a view as of a version.
- **Unpinning in the window forgets the approval and removes its store**
  (`purlis_core::artifact::unpin_here` names the forgotten approval's host;
  `artifacts::forget_web_data` closes every webview still served from it, or laid in its store
  though its folder was read again under another host since, and removes the store, asking again
  a few times while WebKit lets the closed ones go).
- **What stays on disk, unreachable, as before**: an approval forgotten by `purlis artifact
  unpin`, which has no WebKit to ask; an approval recorded before its `storage` was drawn, whose
  host is its pin and fingerprint alone and so is shared by a clone of the plane approved alike,
  which forgetting it here must not empty; a template or pin changed since approval, which asks again
  rather than forgetting; a store WebKit would not let go of; and, on Linux and on macOS before
  14, every approved page's data, kept in the window's store apart by its origin alone.
  WebKitGTK's equivalent is a web context of its own, which the artifact scheme was found not to
  reach.
- **An approved page starts once with empty storage after this change**: what it kept in the
  window's store at its origin stays there, unreachable, as an old origin's did.
