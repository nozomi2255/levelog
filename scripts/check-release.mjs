import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

import { buildReleaseConfig } from "./generate-release-config.mjs";

const root = new URL("../", import.meta.url);
const readJson = (path) => JSON.parse(readFileSync(new URL(path, root), "utf8"));
const packageJson = readJson("package.json");
const tauriConfig = readJson("src-tauri/tauri.conf.json");
const releaseConfig = readJson("src-tauri/tauri.release.conf.json");
const cargo = readFileSync(new URL("src-tauri/Cargo.toml", root), "utf8");
const releaseWorkflow = readFileSync(new URL(".github/workflows/release.yml", root), "utf8");
const cargoVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const failures = [];

if (!packageJson.version || packageJson.version !== tauriConfig.version || packageJson.version !== cargoVersion) {
  failures.push(`package/Tauri/Cargoのversionが一致しません: ${packageJson.version}, ${tauriConfig.version}, ${cargoVersion}`);
}
if (!tauriConfig.bundle?.targets?.includes("dmg")) failures.push("通常bundle targetにdmgがありません");
if (!tauriConfig.bundle?.icon?.includes("icons/icon.icns")) {
  failures.push("macOS app iconがbundle設定にありません");
}
if (releaseConfig.bundle?.createUpdaterArtifacts !== true) failures.push("release configで更新artifactが有効ではありません");
if (releaseConfig.bundle?.macOS?.signingIdentity !== "-") {
  failures.push("release configでmacOSのad-hoc signing identity（-）が明示されていません");
}
if (!releaseWorkflow.includes('bundle_dir="$GITHUB_WORKSPACE/src-tauri/target/')) {
  failures.push("Release署名検証へ渡すartifact pathがGITHUB_WORKSPACE基準の絶対パスではありません");
}
try {
  buildReleaseConfig({
    baseConfig: releaseConfig,
    env: {
      LEVELOG_UPDATER_PUBLIC_KEY: "release-check-public-placeholder",
      LEVELOG_UPDATER_ENDPOINT: "https://github.com/example/levelog/releases/latest/download/latest.json",
    },
  });
} catch {
  failures.push("Release config generatorが有効なupdater設定を生成できません");
}
if (packageJson.scripts?.["notices:generate"] !== "node scripts/generate-third-party-notices.mjs") {
  failures.push("依存ライセンス通知の生成scriptが設定されていません");
}
if (!tauriConfig.build?.beforeBuildCommand?.includes("pnpm notices:generate")) {
  failures.push("app buildが依存ライセンス通知を生成しません");
}
if (tauriConfig.bundle?.resources?.["../LICENSE"] !== "LICENSE") {
  failures.push("LevelogのMIT LICENSEがapp resourcesへ設定されていません");
}
if (tauriConfig.bundle?.resources?.["resources/THIRD_PARTY_NOTICES.md"] !== "THIRD_PARTY_NOTICES.md") {
  failures.push("依存ライセンス通知がapp resourcesへ設定されていません");
}
for (const required of [
  "README.md",
  "LICENSE",
  "SECURITY.md",
  "CONTRIBUTING.md",
  "scripts/generate-third-party-notices.mjs",
  "scripts/generate-release-config.mjs",
  "scripts/generate-release-config.node-tests.mjs",
  "src-tauri/resources/README.md",
  ".github/workflows/ci.yml",
  ".github/workflows/release.yml",
]) {
  try { readFileSync(new URL(required, root)); } catch { failures.push(`${required}がありません`); }
}

try {
  execFileSync(process.execPath, ["--test", "scripts/generate-release-config.node-tests.mjs"], {
    cwd: root,
    stdio: "pipe",
  });
} catch {
  failures.push("Release config generatorのtestが失敗しました");
}

if (process.env.GITHUB_REF_TYPE === "tag") {
  const expectedTag = `v${packageJson.version}`;
  if (process.env.GITHUB_REF_NAME !== expectedTag) failures.push(`tagは${expectedTag}である必要があります`);
}

try {
  execFileSync("git", ["diff", "--check"], { cwd: root, stdio: "pipe" });
} catch {
  failures.push("git diff --checkが失敗しました（空白エラーを修正してください）");
}

if (failures.length) {
  console.error("リリース設定の検証に失敗しました:\n" + failures.map((failure) => `- ${failure}`).join("\n"));
  process.exit(1);
}
console.log(`Levelog v${packageJson.version} のリリース設定を確認しました。`);
