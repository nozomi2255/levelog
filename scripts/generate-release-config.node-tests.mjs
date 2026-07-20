import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { buildReleaseConfig, generateReleaseConfig } from "./generate-release-config.mjs";

const baseConfig = {
  bundle: {
    targets: ["app", "dmg"],
    createUpdaterArtifacts: true,
    macOS: { signingIdentity: "-" },
  },
};
const validEnv = {
  LEVELOG_UPDATER_PUBLIC_KEY: "public-test-value",
  LEVELOG_UPDATER_ENDPOINT: "https://github.com/example/levelog/releases/latest/download/latest.json",
};
const privateSentinels = {
  TAURI_SIGNING_PRIVATE_KEY: "private-test-value",
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "password-test-value",
};

test("injects only public updater configuration and ignores private inputs", () => {
  const generated = buildReleaseConfig({
    baseConfig,
    env: { ...validEnv, ...privateSentinels },
  });
  assert.deepEqual(generated.plugins.updater, {
    endpoints: [validEnv.LEVELOG_UPDATER_ENDPOINT],
    pubkey: validEnv.LEVELOG_UPDATER_PUBLIC_KEY,
  });
  const serialized = JSON.stringify(generated);
  assert.equal(serialized.includes(privateSentinels.TAURI_SIGNING_PRIVATE_KEY), false);
  assert.equal(serialized.includes(privateSentinels.TAURI_SIGNING_PRIVATE_KEY_PASSWORD), false);
});

for (const name of Object.keys(validEnv)) {
  test(`rejects an empty ${name}`, () => {
    assert.throws(
      () => buildReleaseConfig({ baseConfig, env: { ...validEnv, [name]: " " } }),
      new RegExp(name),
    );
  });
}

test("rejects a non-HTTPS updater endpoint", () => {
  assert.throws(
    () =>
      buildReleaseConfig({
        baseConfig,
        env: { ...validEnv, LEVELOG_UPDATER_ENDPOINT: "http://example.com/latest.json" },
      }),
    /must use HTTPS/,
  );
});

test("writes a private-material-free generated config", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "levelog-release-config-test-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const basePath = join(directory, "base.json");
  const outputPath = join(directory, "generated.json");
  const { writeFile } = await import("node:fs/promises");
  await writeFile(basePath, JSON.stringify(baseConfig));
  generateReleaseConfig({
    basePath,
    outputPath,
    env: { ...validEnv, ...privateSentinels },
  });
  const generated = readFileSync(outputPath, "utf8");
  assert.match(generated, /"updater"/);
  assert.equal(generated.includes(privateSentinels.TAURI_SIGNING_PRIVATE_KEY), false);
  assert.equal(generated.includes(privateSentinels.TAURI_SIGNING_PRIVATE_KEY_PASSWORD), false);
});
