# Security Policy

## Supported versions

最新のGitHub Releaseだけをセキュリティ更新の対象とします。古いバージョンを利用している場合は、設定画面の更新機能または最新DMGから更新してください。

## 脆弱性を報告する

公開Issueへ詳細を書かず、GitHubの **Security → Report a vulnerability** からPrivate Vulnerability Reportを送信してください。次を含めると調査が早くなります。

- 影響するLevelogのバージョンとmacOSのバージョン
- 再現手順と期待される動作
- 影響範囲
- 秘密情報や個人データを除いたログ

活動記録、プロフィール、SQLite、JSONエクスポート、Codexの認証情報、更新秘密鍵、Apple証明書を送信しないでください。受領後、影響と修正方針を確認し、公開可能な時点でSecurity Advisoryと修正版を案内します。

## 配布物の信頼境界

- 正式なDMGはGitHub Releasesだけから配布します。
- macOS配布物はDeveloper ID署名とApple公証を必須とします。
- アプリ内更新はHTTPSとTauri更新署名を必須とし、署名検証を無効化できません。
- 更新秘密鍵とAppleの署名・公証情報はGitHub Secretsにのみ設定し、リポジトリや配布物へ同梱しません。
