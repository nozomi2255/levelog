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
- 監査済みsource commit `4354bd5`を通常pushし、`origin/main`とローカル`main`が同一commitであることを確認した。Release tagは作成していない。

### Blocking before the first signed Release

1. **更新署名鍵が未作成。** `docs/release-runbook.md`に従い、永続保管する鍵をユーザー自身のパスワードで生成する必要がある。
2. **Apple Developer ID配布証明書・公証secretが未設定。** 現在のローカルkeychainには有効なcode-signing identityがない。Release workflowは不足時に停止する。

ソース公開の監査は合格済みである。Release workflowは上記2項目のsecretが揃うまで意図的に停止し、unsigned/ad-hoc artifactを公開しない。

## Distribution decision

- 新規導入: GitHub Releasesの署名・公証済みDMG。
- 更新: GitHub Releasesの`latest.json`、アーキテクチャ別`.app.tar.gz`、Tauri更新署名。
- 対応: macOS 11以降、Apple Silicon / Intel。
- 更新チャネル: build時に固定するHTTPS URL。WebViewやユーザー入力から変更しない。
- 更新鍵: public keyはアプリへ固定し、private keyはGitHub `release` Environmentだけへ渡す。
- Apple署名: Developer ID ApplicationとApple公証を必須化し、ad-hoc/unsigned Releaseへ自動fallbackしない。

## Residual risks

- 依存関係のライセンス・脆弱性は時間とともに変わるため、GitHub dependency graph、Dependabot、各Release前のreviewを継続する。
- GitHub-hosted ReleaseとActionsが侵害されても、更新private keyが守られていれば不正artifactはアプリ側で拒否される。ただし秘密鍵を同じCIへ渡す以上、Release Environmentの承認・最小権限・Action SHA固定が重要である。
- Developer IDとTauri更新署名は別の信頼境界であり、どちらか一方だけでは正式配布条件を満たさない。
