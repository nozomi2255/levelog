import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { verifyReleaseAssets } from "./verify-release-assets.mjs";

const repository = "example/levelog";
const tag = "v0.1.0";
const commit = "0123456789abcdef";

async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), "levelog-release-assets-"));
  const contents = {
    "Levelog_0.1.0_aarch64.dmg": "arm dmg",
    "Levelog_0.1.0_x64.dmg": "intel dmg",
    "Levelog_0.1.0_aarch64.app.tar.gz": "arm archive",
    "Levelog_0.1.0_aarch64.app.tar.gz.sig": "arm signature",
    "Levelog_0.1.0_x64.app.tar.gz": "intel archive",
    "Levelog_0.1.0_x64.app.tar.gz.sig": "intel signature",
  };
  const asset = (id, name, size = Buffer.byteLength(contents[name])) => ({
    id,
    name,
    size,
    url: `https://api.github.com/repos/${repository}/releases/assets/${id}`,
    browser_download_url: `https://github.com/${repository}/releases/download/${tag}/${name}`,
  });
  const assets = [
    asset(1, "Levelog_0.1.0_aarch64.dmg"),
    asset(2, "Levelog_0.1.0_x64.dmg"),
    asset(3, "Levelog_0.1.0_aarch64.app.tar.gz"),
    asset(4, "Levelog_0.1.0_aarch64.app.tar.gz.sig"),
    asset(5, "Levelog_0.1.0_x64.app.tar.gz"),
    asset(6, "Levelog_0.1.0_x64.app.tar.gz.sig"),
    asset(7, "latest.json", 1),
  ];
  const latest = {
    version: "0.1.0",
    platforms: {
      "darwin-aarch64": { url: assets[2].url, signature: "arm signature" },
      "darwin-aarch64-app": { url: assets[2].url, signature: "arm signature" },
      "darwin-x86_64": { url: assets[4].url, signature: "intel signature" },
      "darwin-x86_64-app": { url: assets[4].url, signature: "intel signature" },
    },
  };
  const latestContents = JSON.stringify(latest);
  assets[6].size = Buffer.byteLength(latestContents);
  await Promise.all([
    ...Object.entries(contents).map(([name, value]) => writeFile(join(directory, name), value)),
    writeFile(join(directory, "latest.json"), latestContents),
  ]);
  return {
    directory,
    checksumPath: join(directory, "SHA256SUMS.txt"),
    release: {
      id: 99,
      url: `https://api.github.com/repos/${repository}/releases/99`,
      tag_name: tag,
      target_commitish: commit,
      draft: true,
      assets,
    },
    latest,
  };
}

test("binds asset API URLs to the exact Draft assets and writes two checksums", async (context) => {
  const data = await fixture();
  context.after(() => rm(data.directory, { recursive: true, force: true }));
  const result = verifyReleaseAssets({
    ...data,
    assetsDirectory: data.directory,
    repository,
    tag,
    commit,
  });
  assert.equal(result.archives.length, 2);
  const checksums = await readFile(data.checksumPath, "utf8");
  assert.equal(checksums.trim().split("\n").length, 2);
  assert.match(checksums, /^[0-9a-f]{64}  Levelog_0\.1\.0_aarch64\.dmg/m);
});

test("rejects a signature that differs from its Release asset", async (context) => {
  const data = await fixture();
  context.after(() => rm(data.directory, { recursive: true, force: true }));
  data.latest.platforms["darwin-aarch64"].signature = "tampered";
  assert.throws(
    () => verifyReleaseAssets({ ...data, assetsDirectory: data.directory, repository, tag, commit }),
    /signature does not match/,
  );
});

test("rejects a URL that is not an asset of the exact Release", async (context) => {
  const data = await fixture();
  context.after(() => rm(data.directory, { recursive: true, force: true }));
  data.latest.platforms["darwin-x86_64"].url = "https://example.com/foreign.app.tar.gz";
  assert.throws(
    () => verifyReleaseAssets({ ...data, assetsDirectory: data.directory, repository, tag, commit }),
    /does not identify an updater archive/,
  );
});

test("rejects a Draft built from a different commit", async (context) => {
  const data = await fixture();
  context.after(() => rm(data.directory, { recursive: true, force: true }));
  assert.throws(
    () => verifyReleaseAssets({ ...data, assetsDirectory: data.directory, repository, tag, commit: "different" }),
    /Release target must be the workflow commit/,
  );
});
