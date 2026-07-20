# Changelog

Levelogの利用者向け変更を記録します。形式は[Keep a Changelog](https://keepachangelog.com/ja/1.1.0/)を参考にし、バージョンはSemantic Versioningに従います。

## [Unreleased]

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
