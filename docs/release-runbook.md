# Levelog release runbook

この手順は、GitHub ReleasesへApple Silicon / Intel向けDMGと署名済みアプリ内更新を公開するための運用手順です。秘密鍵をリポジトリ、Issue、Actions artifact、アプリデータへ保存しません。

## 1. 一度だけ行う公開準備

1. GitHubに公開リポジトリを作り、ローカルの`origin`を設定する。
2. 公開前に`docs/publication-audit.md`のblockerをすべて解消する。
3. Repository Settingsで次を有効にする。
   - Default `GITHUB_TOKEN` permission: read-only
   - `main`のbranch protectionと必須CI
   - Private vulnerability reporting
   - Dependency graph、Dependabot alerts、secret scanning、push protection
   - ActionsはGitHubと明示的に許可したpublisherだけを許可し、可能ならfull-length SHA pinningを必須化
4. `release` Environmentを作り、必要に応じて承認者と保護ルールを設定する。

## 2. Tauri更新署名鍵

更新署名鍵はAppleのコード署名証明書とは別です。秘密鍵を失うと、その鍵を固定した既存アプリへ新しい更新を配れません。暗号化バックアップを複数の管理された場所へ保管してください。

```bash
mkdir -p .secrets
pnpm tauri signer generate -- -w .secrets/levelog.key
```

生成時に強いパスワードを設定し、GitHubの`release` Environmentへ次を登録します。

| Secret | 内容 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | `.secrets/levelog.key`の内容 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 生成時のパスワード |
| `LEVELOG_UPDATER_PUBLIC_KEY` | `.secrets/levelog.key.pub`の内容。公開情報だが改変防止のためRelease設定で管理 |

秘密鍵をローテーションする場合は、旧鍵で署名した中間リリースへ新しい公開鍵を組み込み、十分な移行期間を設けます。旧鍵を失ってからのアプリ内ローテーションはできません。

## 3. Apple Developer IDと公証

App Store外配布には`Developer ID Application`証明書とApple公証を使います。無料Apple Developerアカウントでは公証できません。

証明書をパスワード付き`.p12`へ書き出し、1行のbase64へ変換します。

```bash
openssl base64 -A -in /absolute/path/to/developer-id.p12 -out certificate-base64.txt
```

GitHubの`release` Environmentへ登録します。

| Secret | 内容 |
| --- | --- |
| `APPLE_CERTIFICATE` | `certificate-base64.txt`の内容 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12`のパスワード |
| `KEYCHAIN_PASSWORD` | Actions内の一時keychain用ランダムパスワード |
| `APPLE_ID` | 公証に使うApple ID |
| `APPLE_PASSWORD` | Apple IDのapp-specific password |
| `APPLE_TEAM_ID` | Apple Developer Team ID |

`certificate-base64.txt`は`.gitignore`対象です。登録後は安全に削除し、`.p12`はアクセス制御された保管場所へ戻します。

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

4. 変更をレビュー・commitし、versionと同じ署名付きtagをpushする。

```bash
git tag -s v0.1.0 -m "Levelog v0.1.0"
git push origin main
git push origin v0.1.0
```

`release.yml`はtagとversionの不一致、公開前監査、秘密情報不足、Developer ID証明書不在、build、公証、更新署名のいずれかで失敗するとReleaseを完了しません。両アーキテクチャのbuild中はDraftのまま保持し、DMGが2件あり、`latest.json`にApple Silicon/Intel双方のURLと署名があることを確認した最終jobだけが公開します。

## 5. 公開後の確認

両アーキテクチャについて次を確認します。

- Release assetsにDMG、`.app.tar.gz`、`.sig`がある。
- `latest.json`に`darwin-aarch64`と`darwin-x86_64`がある。
- DMGを未導入Macへダウンロードし、Applicationsへ導入して警告なく起動できる。
- `codesign --verify --deep --strict`が成功する。
- `spctl --assess --type execute`が成功する。
- 1つ前の正式版から「更新を確認」→「更新して再起動」で新versionになる。
- ローカルSQLite、バックアップ、プロフィールが更新後も保持される。

実更新のsmoke testには、現在versionより高いテストReleaseが必要です。通常のCIで外部Releaseを変更しません。

## 6. 事故対応

- 問題のあるReleaseを「latest」のまま放置せず、原因修正したより高いpatch versionを公開する。
- 更新秘密鍵の漏えいが疑われる場合は、新規公開を停止してSecurity Advisoryを開始する。単純にReleaseを削除しても、漏えい鍵を固定した既存アプリの信頼は回復しない。
- Apple証明書の漏えいが疑われる場合はApple Developerで失効し、新しい証明書へ交換する。
- Actions logやartifactに秘密情報が出た場合は、値を直ちにローテーションして該当run/artifactの削除をGitHub上で行う。
