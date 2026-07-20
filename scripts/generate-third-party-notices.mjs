import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = join(root, "src-tauri/resources/THIRD_PARTY_NOTICES.md");
const forbidden = /(?:^|[ (])(?:(?:A?GPL|LGPL)(?:-|$)|SSPL|BUSL|Commons-Clause|LicenseRef-)/i;

function licenseRequiresReview(expression) {
  if (expression === "UNKNOWN") return true;
  const alternatives = expression.split(/\s+OR\s+/i);
  return alternatives.every((alternative) => forbidden.test(alternative));
}

function licenseLabel(pkg) {
  if (typeof pkg.license === "string") return pkg.license;
  if (pkg.license && typeof pkg.license.type === "string") return pkg.license.type;
  if (Array.isArray(pkg.licenses)) {
    return pkg.licenses
      .map((item) => (typeof item === "string" ? item : item.type))
      .filter(Boolean)
      .join(" OR ");
  }
  return "UNKNOWN";
}

function licenseFiles(directory, explicit) {
  const paths = new Set();
  if (typeof explicit === "string") {
    paths.add(isAbsolute(explicit) ? explicit : join(directory, explicit));
  }
  if (existsSync(directory)) {
    for (const name of readdirSync(directory)) {
      if (/^(LICEN[CS]E|COPYING|NOTICE)(?:\..*)?$/i.test(name)) {
        paths.add(join(directory, name));
      }
    }
  }
  return [...paths]
    .filter(existsSync)
    .map((path) => readFileSync(path, "utf8").trim())
    .filter(Boolean);
}

function upstream(pkg) {
  const value = pkg.homepage ?? pkg.repository?.url ?? pkg.repository;
  return typeof value === "string" ? value.replace(/^git\+/, "") : null;
}

function nodePackages() {
  const store = join(root, "node_modules/.pnpm");
  if (!existsSync(store)) {
    throw new Error("node_modulesがありません。先にpnpm installを実行してください。");
  }

  const packages = new Map();
  for (const entry of readdirSync(store)) {
    const modules = join(store, entry, "node_modules");
    if (!existsSync(modules)) continue;

    for (const name of readdirSync(modules)) {
      const scope = join(modules, name);
      const candidates = name.startsWith("@")
        ? readdirSync(scope).map((child) => join(scope, child))
        : [scope];

      for (const directory of candidates) {
        const manifest = join(directory, "package.json");
        if (!existsSync(manifest)) continue;
        const pkg = JSON.parse(readFileSync(manifest, "utf8"));
        if (!pkg.name || !pkg.version || pkg.private === true) continue;

        const key = `npm:${pkg.name}@${pkg.version}`;
        if (!packages.has(key)) {
          packages.set(key, {
            ecosystem: "npm",
            name: pkg.name,
            version: pkg.version,
            license: licenseLabel(pkg),
            homepage: upstream(pkg),
            texts: licenseFiles(directory, pkg.licenseFile),
          });
        }
      }
    }
  }
  return [...packages.values()];
}

function rustPackages() {
  const raw = execFileSync(
    "cargo",
    [
      "metadata",
      "--locked",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--format-version",
      "1",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
  );
  const metadata = JSON.parse(raw);

  return metadata.packages
    .filter((pkg) => pkg.source && pkg.name !== "levelog")
    .map((pkg) => {
      const directory = dirname(pkg.manifest_path);
      return {
        ecosystem: "cargo",
        name: pkg.name,
        version: pkg.version,
        license: pkg.license ?? "UNKNOWN",
        homepage: pkg.homepage ?? pkg.repository ?? null,
        texts: licenseFiles(directory, pkg.license_file),
      };
    });
}

const packages = [...nodePackages(), ...rustPackages()].sort((a, b) =>
  `${a.ecosystem}:${a.name}:${a.version}`.localeCompare(
    `${b.ecosystem}:${b.name}:${b.version}`,
  ),
);
const invalid = packages.filter(
  (pkg) => licenseRequiresReview(pkg.license),
);
if (invalid.length) {
  throw new Error(
    `確認が必要な依存ライセンス:\n${invalid
      .map(
        (pkg) =>
          `- ${pkg.ecosystem}:${pkg.name}@${pkg.version}: ${pkg.license}`,
      )
      .join("\n")}`,
  );
}

const licenseTexts = new Map();
for (const pkg of packages) {
  pkg.textRefs = [];
  for (const text of pkg.texts) {
    const id = createHash("sha256").update(text).digest("hex").slice(0, 12);
    if (!licenseTexts.has(id)) licenseTexts.set(id, text);
    pkg.textRefs.push(id);
  }
}

const lines = [
  "# Third-party notices",
  "",
  "This file is generated from the locked dependencies used to build Levelog. Package inclusion in this file does not change its license.",
  "",
  `Generated entries: ${packages.length}`,
  "",
  "## Package inventory",
  "",
];

for (const pkg of packages) {
  const refs = [...new Set(pkg.textRefs)];
  lines.push(
    `- **${pkg.ecosystem}:${pkg.name}@${pkg.version}** — \`${pkg.license}\`${
      refs.length ? ` — license text ${refs.map((id) => `\`${id}\``).join(", ")}` : ""
    }${pkg.homepage ? ` — ${pkg.homepage}` : ""}`,
  );
}

lines.push("", "## License and notice texts", "");
for (const [id, licenseText] of [...licenseTexts.entries()].sort(([a], [b]) =>
  a.localeCompare(b),
)) {
  lines.push(
    `### ${id}`,
    "",
    "```text",
    licenseText.replaceAll("```", "` ` `"),
    "```",
    "",
  );
}

mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, lines.join("\n"), "utf8");
console.log(
  `Generated ${output} with ${packages.length} dependency entries and ${licenseTexts.size} unique notice texts.`,
);
