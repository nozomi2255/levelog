import { execFileSync } from "node:child_process";
import { existsSync, lstatSync, readFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" });
const files = git("ls-files", "--cached", "--others", "--exclude-standard", "-z")
  .split("\0")
  .filter(Boolean);
const findings = [];

const forbiddenNames = /(^|\/)(\.env(?:\..+)?|credentials?(?:\..+)?|secrets?(?:\..+)?|id_rsa|id_ed25519|.*\.(?:p12|p8|pem|mobileprovision))$/i;
const generatedPaths = /^(node_modules|dist|coverage|playwright-report|test-results|src-tauri\/target)\//;
const highConfidenceSecrets = [
  ["private key", /-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----/],
  ["GitHub token", /(?:gh[opusr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})/],
  ["OpenAI-style secret", /\bsk-[A-Za-z0-9_-]{32,}\b/],
  ["AWS access key", /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/],
  ["Slack token", /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/],
  ["Stripe live secret", /\bsk_live_[A-Za-z0-9]{20,}\b/],
  ["Google API key", /\bAIza[A-Za-z0-9_-]{30,}\b/],
];

for (const file of files) {
  if (forbiddenNames.test(file)) findings.push(`${file}: 公開対象にできない秘密情報ファイル名です`);
  if (generatedPaths.test(file)) findings.push(`${file}: 生成物をGit追跡しないでください`);
  const url = new URL(file, root);
  if (!existsSync(url)) continue;
  const stat = lstatSync(url);
  if (stat.isSymbolicLink()) {
    findings.push(`${file}: 公開監査で追跡できないsymbolic linkです`);
    continue;
  }
  if (stat.size > 10 * 1024 * 1024) findings.push(`${file}: 10 MiBを超えるファイルです`);
  const bytes = readFileSync(url);
  if (bytes.includes(0)) continue;
  const text = bytes.toString("utf8");
  if (file !== "scripts/audit-publication.mjs") {
    for (const [label, pattern] of highConfidenceSecrets) {
      if (pattern.test(text)) findings.push(`${file}: ${label}候補を検出しました`);
    }
    const personalPath = new RegExp(["/", "Users", "/", "[^/\\s]+", "/"].join(""));
    if (personalPath.test(text)) findings.push(`${file}: macOSの個人絶対パスを検出しました`);
  }
}

const publicEmailDomains = new Set(["users.noreply.github.com", "noreply.github.com", "example.com", "localhost"]);
const authorEmails = new Set(git("log", "--all", "--format=%ae").split("\n").filter(Boolean));
for (const email of authorEmails) {
  const domain = email.split("@").at(-1)?.toLowerCase() ?? "";
  if (!publicEmailDomains.has(domain)) {
    findings.push("Git履歴: 公開用ではない可能性があるauthor emailを検出しました（値は出力しません）");
  }
}

if (findings.length) {
  console.error("公開前監査に失敗しました:\n" + findings.map((finding) => `- ${finding}`).join("\n"));
  process.exit(1);
}

console.log(`公開前監査に合格しました（${files.length}ファイル、${authorEmails.size} author email）。`);
