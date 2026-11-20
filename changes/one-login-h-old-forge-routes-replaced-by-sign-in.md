### Changed

- Chats reach a forge through the one sign-in, and no chat holds a forge token. A tool line that asks for a GitHub App token (`mint = "github-app"`) or hands a vault token in `GH_TOKEN` or `GITLAB_TOKEN` still loads but runs nothing: a Notice and a `purlis doctor` row say to sign in under Settings › Forges and delete the line. purlis deletes no vault item and no keyring entry unless you press to remove it. Pool runners hold no installation token either: they use the desktop's proxy, or a token connection where no desktop is present.
