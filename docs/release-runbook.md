# Levelog release runbook（ad-hoc署名DMG）

この手順は、GitHub ReleasesへApple Silicon / Intel向けの**ad-hoc署名DMG**と、Tauri更新署名付きアプリ内更新を公開するための運用手順です。現在のDMGはApple Developer ID署名・Apple公証を行いません。秘密鍵をリポジトリ、Issue、Actions artifact、アプリデータへ保存しません。

## 1. 一度だけ行う公開準備

1. GitHubに公開リポジトリを作り、ローカルの`origin`を設定する。
2. 公開前に`docs/publication-audit.md`のblockerをすべて解消する。
3. Repository Settingsで次を有効にする。
   - Default `GITHUB_TOKEN` permission: read-only
   - `main`のbranch protectionと必須CI
   - Private vulnerability reporting
   - Dependency graph、Dependabot alerts、secret scanning、push protection
   - ActionsはGitHubと明示的に許可したpublisherだけを許可し、可能ならfull-length SHA pinningを必須化
4. `release` Environmentを作り、可能なら承認者と保護ルールを設定する。Environmentを使わない場合でも、秘密値はRepository Actions Secretsへ限定する。

## 2. Tauri更新署名鍵

更新署名鍵はAppleのコード署名証明書とは別です。秘密鍵を失うと、その鍵を固定した既存アプリへ新しい更新を配れません。リポジトリ内ではなく、ユーザーが管理する永続パスへ生成します。

```bash
mkdir -p ~/.tauri
pnpm tauri signer generate -w ~/.tauri/levelog.key
```

生成前に保存先を用意し、生成時に強いパスワードを設定します。秘密鍵`~/.tauri/levelog.key`、公開鍵`~/.tauri/levelog.key.pub`、秘密鍵の暗号化バックアップ、鍵のパスワードは、互いに別の管理された場所で保管してください。秘密鍵・バックアップ・パスワードはリポジトリ、GitHub Issue、Release asset、Actions logへ置きません。

GitHub Actions Secretsへ次を登録します。Repository Secretsでも動作しますが、可能なら`release` Environment Secretsとして登録し、Release jobだけに限定します。

| Secret | 内容 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | `~/.tauri/levelog.key`の内容 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 生成時のパスワード |
| `LEVELOG_UPDATER_PUBLIC_KEY` | `~/.tauri/levelog.key.pub`の内容。公開情報だが改変防止のためRelease設定で管理 |

秘密鍵をローテーションする場合は、旧鍵で署名した中間リリースへ新しい公開鍵を組み込み、十分な移行期間を設けます。旧鍵を失ってからのアプリ内ローテーションはできません。

## 3. 現在の配布方針と将来のDeveloper ID移行

現在のReleaseに必要なGitHub Actions Secretは、次の3つだけです。

| Secret | 内容 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | 更新署名秘密鍵 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 更新署名秘密鍵のパスワード |
| `LEVELOG_UPDATER_PUBLIC_KEY` | 更新署名公開鍵 |

DMGのSHA-256をまとめた`SHA256SUMS.txt`をRelease assetへ添付し、Release notesからそのassetを案内します。これはダウンロードの破損・取り違えを検出するためであり、配布者の本人性を証明するものではありません。

将来Apple Developer Programへ加入する場合は、`Developer ID Application`証明書とApple公証を追加します。その時点でApple証明書、証明書パスワード、keychain用パスワード、Apple ID、公証用app-specific password、Team IDを`release` Environmentへ追加し、署名・公証をRelease gateに戻します。それまでは、Apple資格情報を要求したり、署名・公証済みと表現したりしません。

## 4. リリースを作る

1. `CHANGELOG.md`の`Unreleased`を対象バージョンへ移す。
2. 次の3ファイルのversionを同じSemVerへ更新する。
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
3. 検証する。

```bash
pnpm install --frozen-lockfile
pnpm check:release
pnpm audit:public
pnpm notices:generate
pnpm lint
pnpm test
pnpm build
pnpm test:e2e
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

4. 変更をレビュー・commitし、versionと同じtagをpushする。Git tag署名鍵を設定済みなら署名付きtag、未設定ならannotated tagを使う。どちらの場合も、配布物自体はTauri Updater鍵で署名される。

```bash
# Git tag署名鍵を設定済みの場合
git tag -s v0.1.0 -m "Levelog v0.1.0"

# Git tag署名鍵を未設定の場合
git tag -a v0.1.0 -m "Levelog v0.1.0"
git push origin main
git push origin v0.1.0
```

`release.yml`はtagとversionの不一致、公開前監査、上記3つの更新署名secret不足、build、更新署名のいずれかで失敗するとReleaseを完了しません。Apple資格情報は現在のRelease条件に含めません。両アーキテクチャのbuild中はDraftのまま保持し、DMGが2件あり、`SHA256SUMS.txt`、`latest.json`、Apple Silicon/Intel双方の更新URLとTauri署名が揃うことを確認した最終jobだけが公開します。

## 5. 公開後の確認

両アーキテクチャについて次を確認します。

- Release assetsに両アーキテクチャのDMG、`SHA256SUMS.txt`、`.app.tar.gz`、`.sig`がある。
- `SHA256SUMS.txt`とダウンロードしたDMGのSHA-256が一致する。この照合は破損確認であり、Appleによる配布者確認ではない。
- `latest.json`に`darwin-aarch64`と`darwin-x86_64`がある。
- DMGを未導入Macへダウンロードし、Applicationsへ導入後にGatekeeper警告が出ること、およびREADMEの「システム設定 → プライバシーとセキュリティ → このまま開く」手順でアプリ単位に起動できることを確認する。Gatekeeperを無効化したり、`xattr`を削除したりしない。
- 1つ前の正式版から「更新を確認」→「更新して再起動」で新versionになる。
- ローカルSQLite、バックアップ、プロフィールが更新後も保持される。

実更新のsmoke testには、現在versionより高いテストReleaseが必要です。通常のCIで外部Releaseを変更しません。

## 6. 事故対応

- 問題のあるReleaseを「latest」のまま放置せず、原因修正したより高いpatch versionを公開する。
- 更新秘密鍵の漏えいが疑われる場合は、新規公開を停止してSecurity Advisoryを開始する。単純にReleaseを削除しても、漏えい鍵を固定した既存アプリの信頼は回復しない。
- 将来Apple証明書を導入後に漏えいが疑われる場合はApple Developerで失効し、新しい証明書へ交換する。
- Actions logやartifactに秘密情報が出た場合は、値を直ちにローテーションして該当run/artifactの削除をGitHub上で行う。
