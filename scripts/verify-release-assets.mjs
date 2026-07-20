import { createHash } from "node:crypto";
import { basename, join } from "node:path";
import { readFileSync, statSync, writeFileSync } from "node:fs";

function invariant(condition, message) {
  if (!condition) throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    invariant(name?.startsWith("--") && value, `Invalid argument near ${name ?? "end of input"}`);
    values.set(name.slice(2), value);
  }
  return values;
}

function required(values, name) {
  const value = values.get(name);
  invariant(value, `Missing required --${name}`);
  return value;
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function matchingAssets(assets, suffix) {
  return assets.filter((asset) => asset.name.endsWith(suffix));
}

function fileContents(path, label) {
  try {
    return readFileSync(path, "utf8").trim();
  } catch (error) {
    throw new Error(`Could not read ${label}: ${error.message}`);
  }
}

export function verifyReleaseAssets({ release, latest, assetsDirectory, repository, tag, commit, checksumPath }) {
  invariant(release && typeof release === "object", "Release response must be an object");
  invariant(release.draft === true, "Release must remain a Draft until verification completes");
  invariant(release.tag_name === tag, `Release tag must be ${tag}`);
  invariant(release.target_commitish === commit, `Release target must be the workflow commit ${commit}`);
  invariant(Number.isSafeInteger(release.id), "Release id is missing or invalid");
  invariant(
    release.url === `https://api.github.com/repos/${repository}/releases/${release.id}`,
    "Release API URL is outside the current repository",
  );
  invariant(Array.isArray(release.assets), "Release assets are missing");

  const assets = release.assets;
  const names = assets.map((asset) => asset.name);
  invariant(new Set(names).size === names.length, "Release asset names must be unique");
  for (const name of names) {
    invariant(name === basename(name), `Release asset name is unsafe: ${name}`);
  }

  const version = tag.replace(/^v/, "");
  const expectedNames = [
    `Levelog_${version}_aarch64.dmg`,
    `Levelog_${version}_x64.dmg`,
    `Levelog_${version}_aarch64.app.tar.gz`,
    `Levelog_${version}_aarch64.app.tar.gz.sig`,
    `Levelog_${version}_x64.app.tar.gz`,
    `Levelog_${version}_x64.app.tar.gz.sig`,
    "latest.json",
  ].toSorted();
  invariant(
    JSON.stringify(names.toSorted()) === JSON.stringify(expectedNames),
    "Release must contain exactly the seven expected version assets before checksum publication",
  );
  for (const asset of assets) {
    invariant(Number.isSafeInteger(asset.id) && asset.id > 0, `Asset id is invalid: ${asset.name}`);
    invariant(Number.isSafeInteger(asset.size) && asset.size > 0, `Asset size is invalid: ${asset.name}`);
    const actualSize = statSync(join(assetsDirectory, asset.name)).size;
    invariant(actualSize === asset.size, `Downloaded asset size does not match GitHub metadata: ${asset.name}`);
  }

  const dmgs = matchingAssets(assets, ".dmg");
  const archives = matchingAssets(assets, ".app.tar.gz");
  const signatures = matchingAssets(assets, ".app.tar.gz.sig");
  invariant(dmgs.length === 2, `Expected exactly two DMG assets, found ${dmgs.length}`);
  invariant(archives.length === 2, `Expected exactly two updater archives, found ${archives.length}`);
  invariant(signatures.length === 2, `Expected exactly two updater signatures, found ${signatures.length}`);
  invariant(assets.filter((asset) => asset.name === "latest.json").length === 1, "Expected exactly one latest.json asset");

  invariant(latest?.version === version, "latest.json version does not match the tag");
  invariant(latest.platforms && typeof latest.platforms === "object", "latest.json platforms are missing");
  const expectedPlatforms = ["darwin-aarch64", "darwin-aarch64-app", "darwin-x86_64", "darwin-x86_64-app"];
  invariant(
    JSON.stringify(Object.keys(latest.platforms).toSorted()) === JSON.stringify(expectedPlatforms),
    "latest.json must contain only the four expected macOS platform entries",
  );

  const referencedArchives = new Set();
  for (const platform of ["darwin-aarch64", "darwin-x86_64"]) {
    const entry = latest.platforms[platform];
    invariant(entry && typeof entry === "object", `latest.json is missing ${platform}`);
    invariant(typeof entry.url === "string" && entry.url.length > 0, `${platform} URL is missing`);
    invariant(typeof entry.signature === "string" && entry.signature.length > 0, `${platform} signature is missing`);

    const archive = assets.find((asset) => asset.name.endsWith(".app.tar.gz") && asset.url === entry.url);
    invariant(archive, `${platform} URL does not identify an updater archive in this exact Release`);
    referencedArchives.add(archive.name);

    const signatureName = `${archive.name}.sig`;
    invariant(signatures.some((asset) => asset.name === signatureName), `${platform} signature asset is missing`);
    const signatureContents = fileContents(join(assetsDirectory, signatureName), `${platform} signature asset`);
    invariant(entry.signature.trim() === signatureContents, `${platform} signature does not match its asset`);
  }

  for (const [canonical, alias] of [
    ["darwin-aarch64", "darwin-aarch64-app"],
    ["darwin-x86_64", "darwin-x86_64-app"],
  ]) {
    invariant(
      latest.platforms[alias].url === latest.platforms[canonical].url &&
        latest.platforms[alias].signature === latest.platforms[canonical].signature,
      `${alias} must exactly match ${canonical}`,
    );
  }

  invariant(referencedArchives.size === 2, "Updater platforms must reference distinct archives");
  invariant(
    archives.every((asset) => referencedArchives.has(asset.name)),
    "Every updater archive must be referenced by a required macOS platform",
  );

  const checksumLines = dmgs
    .toSorted((left, right) => left.name.localeCompare(right.name))
    .map((asset) => {
      const bytes = readFileSync(join(assetsDirectory, asset.name));
      return `${createHash("sha256").update(bytes).digest("hex")}  ${asset.name}`;
    });
  writeFileSync(checksumPath, `${checksumLines.join("\n")}\n`, { mode: 0o600 });

  return { dmgs: dmgs.map((asset) => asset.name), archives: [...referencedArchives] };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    const args = parseArgs(process.argv.slice(2));
    const releasePath = required(args, "release-json");
    const latestPath = required(args, "latest-json");
    const assetsDirectory = required(args, "assets-dir");
    const checksumPath = required(args, "checksum-output");
    const result = verifyReleaseAssets({
      release: readJson(releasePath, "Release response"),
      latest: readJson(latestPath, "latest.json"),
      assetsDirectory,
      repository: required(args, "repository"),
      tag: required(args, "tag"),
      commit: required(args, "commit"),
      checksumPath,
    });
    console.log(`Verified ${result.dmgs.length} DMGs and ${result.archives.length} signed updater archives.`);
  } catch (error) {
    console.error(`Release asset verification failed: ${error.message}`);
    process.exit(1);
  }
}
