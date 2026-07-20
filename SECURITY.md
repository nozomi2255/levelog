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
- 現在のDMGはad-hoc署名であり、Apple Developer ID署名・Apple公証は行っていません。初回起動時にGatekeeperの警告が出るため、READMEのアプリ単位の「このまま開く」手順で、公式Releaseから取得したものだけを開いてください。
- アプリ内更新はHTTPSとTauri更新署名を必須とし、署名検証を無効化できません。
- Tauri更新署名はApple署名・公証の代替ではありません。前者は更新artifactの完全性、後者はmacOSが確認する配布者・公証の信頼境界です。
- 更新秘密鍵はGitHub Actions Secretsにのみ設定し、リポジトリや配布物へ同梱しません。可能な場合は承認ルールを持つ`release` Environmentへ限定します。将来Developer ID配布へ移行する場合も、Appleの証明書・公証資格情報は同様にSecretsだけへ設定します。
