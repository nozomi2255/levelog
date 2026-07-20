# Bundled legal notices

`THIRD_PARTY_NOTICES.md` is generated from the locked npm and Cargo dependency trees by
`pnpm notices:generate`. It is intentionally ignored because it can exceed normal source-review size.
Release and app builds regenerate it, validate every detected license expression, and bundle it next
to Levelog's MIT `LICENSE` inside the application resources.
