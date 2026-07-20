import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const REQUIRED_INPUTS = [
  "LEVELOG_UPDATER_PUBLIC_KEY",
  "LEVELOG_UPDATER_ENDPOINT",
];

function requireNonempty(value, name) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${name} must be set to a nonempty value`);
  }
  return value.trim();
}

function validateEndpoint(value) {
  const endpoint = requireNonempty(value, "LEVELOG_UPDATER_ENDPOINT");
  let url;
  try {
    url = new URL(endpoint);
  } catch {
    throw new Error("LEVELOG_UPDATER_ENDPOINT must be a valid URL");
  }
  if (url.protocol !== "https:") {
    throw new Error("LEVELOG_UPDATER_ENDPOINT must use HTTPS");
  }
  if (url.username || url.password || url.hash) {
    throw new Error("LEVELOG_UPDATER_ENDPOINT must not contain credentials or a fragment");
  }
  return url.href;
}

export function buildReleaseConfig({ baseConfig, env }) {
  if (!baseConfig || typeof baseConfig !== "object" || Array.isArray(baseConfig)) {
    throw new Error("base release config must be a JSON object");
  }
  for (const name of REQUIRED_INPUTS) requireNonempty(env[name], name);
  if (baseConfig.bundle?.createUpdaterArtifacts !== true) {
    throw new Error("base release config must enable updater artifacts");
  }
  if (baseConfig.bundle?.macOS?.signingIdentity !== "-") {
    throw new Error("base release config must explicitly use macOS ad-hoc signing");
  }

  const endpoint = validateEndpoint(env.LEVELOG_UPDATER_ENDPOINT);
  const publicKey = env.LEVELOG_UPDATER_PUBLIC_KEY.trim();
  return {
    ...baseConfig,
    plugins: {
      ...(baseConfig.plugins ?? {}),
      updater: {
        endpoints: [endpoint],
        pubkey: publicKey,
      },
    },
  };
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if ((flag !== "--base" && flag !== "--output") || !value) {
      throw new Error("usage: generate-release-config.mjs --base <path> --output <path>");
    }
    options[flag.slice(2)] = value;
  }
  if (!options.base || !options.output) {
    throw new Error("both --base and --output are required");
  }
  return options;
}

export function generateReleaseConfig({ basePath, outputPath, env }) {
  let baseConfig;
  try {
    baseConfig = JSON.parse(readFileSync(basePath, "utf8"));
  } catch {
    throw new Error("base release config must exist and contain valid JSON");
  }
  const generated = buildReleaseConfig({ baseConfig, env });
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, `${JSON.stringify(generated, null, 2)}\n`, { mode: 0o600 });
}

function main() {
  try {
    const options = parseArguments(process.argv.slice(2));
    generateReleaseConfig({
      basePath: resolve(options.base),
      outputPath: resolve(options.output),
      env: process.env,
    });
    console.log("Generated the macOS release config without private signing material.");
  } catch (error) {
    console.error(`Release config generation failed: ${error.message}`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) main();
