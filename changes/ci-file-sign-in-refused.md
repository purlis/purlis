### Fixed

- When GitHub refuses the sign-in that allows CI files (401), a held CI-file push no longer asks "Once more" to send the same push again. The ask ends with a plain sentence: GitHub refused that sign-in, so allow CI files again in Settings › You › Accounts and push again. purlis also removes the refused sign-in and shows a Notice, so Settings no longer shows CI files as allowed (#7658).
