# Changelog

Levelogの利用者向け変更を記録します。形式は[Keep a Changelog](https://keepachangelog.com/ja/1.1.0/)を参考にし、バージョンはSemantic Versioningに従います。

## [Unreleased]

## [0.1.2] - 2026-07-21

### Changed

- AIへ送る活動・プロフィール・確認回答を、JSON編集欄ではなく読みやすい確認画面で表示
- 技術的なJSONは必要な場合だけ詳細表示から確認する形に変更

## [0.1.1] - 2026-07-21

### Fixed

- Codexの認証済み起動パスを検出できるように改善

## [0.1.0] - 2026-07-20

### Added

- GitHub ReleasesからTauri更新署名を検証して更新を確認・インストール・再起動する設定画面
- Apple Silicon / Intel向けDMG、更新artifact、`latest.json`を作成するRelease workflow
- MIT License、公開前監査、Security Policy、Contribution Guide
- Personal Evidence Graphを表すLevelog専用アイコンと、全platform向け派生asset
- ロック済みnpm/Cargo依存関係から生成し、アプリへ同梱するthird-party notices

### Security

- 更新エンドポイントと公開鍵をReleaseビルド時に固定し、WebViewから任意URLを指定できない設計
- Tauri更新署名の秘密情報が不足したReleaseをfail closedにする運用を追加
- 両アーキテクチャのDMGと更新署名が揃うまでGitHub ReleaseをDraftに保つ公開gate
- Apple Developer ID署名・Apple公証を行わないad-hoc配布であることを、アプリ、README、Release notesへ明示
- 各更新archiveを埋め込み公開鍵で暗号検証し、DMGの`SHA256SUMS.txt`を公開前に生成するRelease gate
