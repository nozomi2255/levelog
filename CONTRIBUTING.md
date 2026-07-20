# Contributing to Levelog

LevelogへのIssue、設計提案、ドキュメント改善、コード変更を歓迎します。

## 開始前

1. 大きな仕様変更は、実装前にIssueで目的とユーザー価値を共有してください。
2. 個人の活動記録、プロフィール、エクスポートJSON、SQLite、Codexの生出力、絶対パスをIssueやfixtureへ含めないでください。
3. テストデータは架空で決定的なfixtureだけを使用してください。

## Pull request

```bash
pnpm install --frozen-lockfile
pnpm audit:public
pnpm lint
pnpm test
pnpm build
pnpm test:e2e
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Pull requestには、変更理由、ユーザーへの影響、検証結果、データモデルやプライバシー境界の変更を記載してください。SQLite migrationは公開後に編集せず、append-onlyで追加します。

## AI・エビデンスの原則

- AI提案を事実として保存しない。
- 原文と送信payloadをAI出力で上書きしない。
- XP、スキル観測、プロジェクトリンクは明示的なユーザー判断を必要とする。
- 任意のshell実行、任意パス読み取り、秘密情報の保存をTauri Commandへ追加しない。

投稿したコントリビューションは本リポジトリのMIT Licenseで提供されることに同意したものとします。
