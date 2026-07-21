# Levelog

<img src="assets/levelog-icon.png" width="128" height="128" alt="Levelog app icon">

Levelogは、日々の仕事・学習・判断を、ユーザー自身が確認できる成長の証拠へ変えるmacOSアプリです。活動の原文、AIへの送信内容、AIの提案、採否、クエスト、XP、エビデンスグラフをSQLiteへローカル保存します。

## インストール

1. [Levelog GitHub Releases](https://github.com/nozomi2255/levelog/releases) から、Macに合う`Levelog_*.dmg`をダウンロードします。ほかの配布元から取得しないでください。
2. DMGを開き、LevelogをApplicationsへドラッグします。
3. ApplicationsにあるLevelogを一度開きます。
4. 現在のDMGは**ad-hoc署名**であり、Apple Developer ID署名およびApple公証は行っていません。そのためmacOSの警告で起動が止まった場合は、 → **システム設定** → **プライバシーとセキュリティ** → 「セキュリティ」までスクロール → **このまま開く** → macOSのログインパスワードを入力 → **開く**の順に操作します。

「このまま開く」は、最初の起動を試した後にそのアプリだけに適用する例外です。Gatekeeperを無効化したり、ターミナルで保護属性を削除したりしないでください。組織が管理するMacでは、この操作が許可されない場合があります。

Releaseに`SHA256SUMS.txt`がある場合は、ダウンロードしたDMGのSHA-256値と照合して破損を確認できます。これはダウンロードの完全性を確認するための値であり、Appleの開発者本人性確認やDeveloper ID署名の代わりにはなりません。

```bash
shasum -a 256 /path/to/Levelog_*.dmg
```

表示されたSHA-256を同じReleaseの`SHA256SUMS.txt`にある該当DMGの値と比較します。このコマンドはダウンロードの照合だけを行うもので、Gatekeeperを回避・変更するものではありません。

## アプリ内更新

正式版では、**設定 → アプリの更新 → 更新を確認**からGitHub Releasesの最新版を確認できます。更新を選ぶとLevelogが次を行います。

- HTTPSの固定リリースチャネルだけを確認
- Tauriの更新署名を検証
- 対応するアーキテクチャの更新だけをダウンロード
- インストール後にアプリを再起動

ここで検証するのは**Tauri更新署名**です。これはAppleのコード署名・Apple公証とは別の仕組みで、更新ファイルの改ざんを検出します。初回にDMGを導入する際のmacOS警告をなくすものではありません。署名が一致しない更新はインストールされません。開発ビルドでは更新チャネルを無効にしています。

## プライバシー

- ユーザーデータはmacOSのApplication Support配下にあるSQLiteへ保存されます。
- バックアップとJSONアーカイブは、設定画面からユーザーが明示的に作成します。
- Codex CLIを使う分析はクラウド推論を伴う場合があります。送信前に実際のJSONを確認・編集できます。
- AI出力は候補です。ユーザーが承認するまでスキル観測、プロジェクト、ポートフォリオ、分析由来XPへ反映しません。
- LevelogはCodexの認証情報や更新署名の秘密鍵をアプリデータへ保存しません。

詳しい設計は[成長ログ設計](docs/growth-log-system-design.md)と[Personal Evidence Graph設計](docs/personal-evidence-graph-design.md)を参照してください。

## 開発

必要環境:

- macOS 11以降
- Node.js 22.17以降
- pnpm 10.13.1
- Rust stable
- Xcode Command Line Tools

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm test
pnpm build
pnpm notices:generate
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

実Codex smoke testは通常テストから分離しています。`test:codex-connection-smoke`はローカルCLIの接続・安全機能だけを確認し、推論リクエストは送信しません。

```bash
LEVELOG_CODEX_PATH=/absolute/path/to/codex pnpm test:codex-connection-smoke
LEVELOG_CODEX_PATH=/absolute/path/to/codex pnpm test:codex-smoke
```

公開前監査とリリース手順は[公開監査](docs/publication-audit.md)と[リリース手順](docs/release-runbook.md)に記載しています。

## コントリビューションとセキュリティ

- 変更提案: [CONTRIBUTING.md](CONTRIBUTING.md)
- 脆弱性の報告: [SECURITY.md](SECURITY.md)
- Issueへ活動記録、エクスポートJSON、Codex出力、ローカルパスなどの個人データを添付しないでください。

## License

[MIT](LICENSE)。配布アプリにはロック済みnpm/Cargo依存関係から生成したthird-party noticesも同梱します。
