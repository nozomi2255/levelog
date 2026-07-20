import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: "html",
  use: {
    baseURL: "http://127.0.0.1:1420",
    trace: "on-first-retry",
  },
  projects: [
    { name: "mock-reference", use: { ...devices["Desktop Chrome"], viewport: { width: 1586, height: 992 } } },
    { name: "desktop", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } },
    { name: "compact", use: { ...devices["Desktop Chrome"], viewport: { width: 1024, height: 720 } } },
  ],
  webServer: {
    command: "pnpm dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
  },
});

