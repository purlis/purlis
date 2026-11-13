# Amended (2026-11-13): a TLS pool's files are held from chats, and a link's key file lives as long as the link

Records what the review of [#6267](https://github.com/purlis/purlis/issues/6267) changed or left
as it is. It adds to §3 and §8. The rest of this record stands.

## What this process holds for a pool goes when the pool does

A key read from a vault is held in memory for a while, so a link's several runs read the vault
once (`tlsroute::VAULT_KEY_KEPT_FOR`). When a pool is removed, its key and its engine's last
check are let go of at once (`tlsroute::pool::forget_pool`). A run folder that a run left
behind, a crashed one say, is still swept when the window starts
(`dockerpool::settings::sweep_at_start`).

## A link's key file lives as long as the link

Each run of `docker` for a TLS pool is handed `ca.pem`, `cert.pem` and `key.pem` in a run folder
of its own (`0600` in `0700`, under `<data>/docker-settings`, which no chat reads or writes). A
short run's folder goes when the run ends. The link's `docker exec` is one run that lasts as
long as the link, so its folder, key included, is on disk for that long.

Docker's client reads the files once, as it opens the connection, and does not read them again.
Removing the key file once the handshake is up would therefore be safe. But `docker exec` prints
nothing that tells purlis when it has read the key, and guessing with a delay would break a slow
handshake. So this is left as it is: the key is on disk for the link's life, in a folder no chat
reaches, and it goes when the link ends or, after a crash, at the next start.
