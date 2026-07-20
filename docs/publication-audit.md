# Open-source publication audit

Last reviewed: 2026-07-20 (Asia/Tokyo)

## Scope

公開対象となるGit追跡ファイル、未追跡だが公開予定のファイル、既存commitのauthor metadata、秘密情報らしいファイル名、既知token/private-key形式、個人絶対パス、画像asset、配布・更新設定を確認した。

自動確認:

```bash
pnpm audit:public
pnpm check:release
git status --short --branch
git remote -v
git log --all --format='%h %an <%ae> %s'
```

`scripts/audit-publication.mjs`は秘密値を検出しても値自体を出力しない。これは高確度パターンの予防線であり、GitHub secret scanningや人手レビューを置き換えない。

## Findings

### Cleared in the working tree

- API token、private key、Apple証明書、`.env`、credential fileは追跡されていない。
- ソース、設定、要件・設計文書にmacOSの個人絶対パスは残っていない。
- runtimeのデモユーザーデータやMock AIはなく、決定的な架空データはtest fixtureに限定されている。
- `docs/mock-ui.png`はブラウザchromeや個人通知を含まないデザイン参照画像である。
- アプリアイコンはこのリポジトリ専用に生成した`assets/levelog-icon.png`をsourceとし、外部商標を含まない。生成経緯、編集内容、派生生成コマンド、MITでの配布条件を`assets/README.md`へ記録した。
- `.gitignore`は環境ファイル、秘密鍵・証明書、署名生成物、build/test生成物を除外する。
- npm/Cargoのロック済み依存関係848件を検査し、不明ライセンス、またはpermissiveな`OR`選択肢がない強いcopyleft/source-available条件を検出した場合に失敗する通知生成を追加した。237個の重複排除済みlicense/notice本文とMIT Licenseをapp resourceへ同梱する。
- MIT License、Security Policy、Contribution Guide、CI、Dependabot、fail-closed Release workflowを追加した。
- 既存2 commitはtree、日時、メッセージを保持したまま`nozomi2255@users.noreply.github.com`へ書き換え、`pnpm audit:public`は公開用author identityだけで合格した。
- 書き換え前の完全な履歴は公開対象外の`.git/pre-publication-history-20260720.bundle`へ退避し、`git bundle verify`で復元可能性を確認した。
- 空であることを確認した`https://github.com/nozomi2255/levelog.git`を`origin`へ設定した。既存のremote branchやtagは上書きしていない。
- 監査時点の公開対象sourceを通常pushし、`origin/main`とローカル`main`が一致することを確認した。Release tagは作成していない。以後の変更では、公開直前にこの監査とremoteの一致を再確認する。

### Blocking before the first updater-signed Release

1. **更新署名鍵が未作成。** `docs/release-runbook.md`に従い、永続保管する鍵をユーザー自身のパスワードで生成する必要がある。

Apple Developer ID配布証明書・公証資格情報は、現在は意図的に設定しない。したがって最初のDMGはad-hoc署名であり、Apple Developer ID署名・Apple公証は行われない。ソース公開の監査は合格済みである。Release workflowはTauri更新署名の3 secretが揃うまで意図的に停止する。

## Distribution decision

- 新規導入: [GitHub Releases](https://github.com/nozomi2255/levelog/releases) のad-hoc署名DMG。Apple Developer ID署名・Apple公証は行わない。
- 更新: GitHub Releasesの`latest.json`、アーキテクチャ別`.app.tar.gz`、Tauri更新署名。
- 対応: macOS 11以降、Apple Silicon / Intel。
- 更新チャネル: build時に固定するHTTPS URL。WebViewやユーザー入力から変更しない。
- 更新鍵: public keyはアプリへ固定し、private keyはGitHub Actions SecretsからRelease jobだけへ渡す。承認ルールを持つ`release` Environmentへの移行を推奨する。
- Apple署名: 現在は導入しない。将来のDeveloper ID ApplicationとApple公証は、別途明示して移行する。

## Residual risks

- 依存関係のライセンス・脆弱性は時間とともに変わるため、GitHub dependency graph、Dependabot、各Release前のreviewを継続する。
- GitHub-hosted ReleaseとActionsが侵害されても、更新private keyが守られていれば不正artifactはアプリ側で拒否される。ただし秘密鍵を同じCIへ渡す以上、Release Environmentの承認・最小権限・Action SHA固定が重要である。
- 初回導入DMGにはAppleの開発者本人性確認・公証がないため、Gatekeeper警告が表示される。公式Release URLだけを案内し、`SHA256SUMS.txt`で破損を確認する。SHA-256照合は配布者の本人性を証明しない。
- Tauri更新署名とApple Developer ID署名・公証は別の信頼境界である。Tauri更新署名は更新artifactの改ざん検出を担うが、初回DMGのAppleによる確認を代替しない。
