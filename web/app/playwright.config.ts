import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:3771",
    trace: "on-first-retry"
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] }
    }
  ],
  webServer: {
    command:
      "cd ../.. && mkdir -p .scratch && target/debug/pygco import fixtures/golden/diff-before-v1.jsonl.gz fixtures/golden/diff-after-v1.jsonl.gz fixtures/golden/stubs-v1.jsonl.gz fixtures/golden/missing-referents-v1.jsonl.gz -o .scratch/playwright.sqlite --rebuild --format json >/tmp/pygco-playwright-import.json && PYGCO_WEB_DIST=web/app/dist target/debug/pygco web .scratch/playwright.sqlite --host 127.0.0.1 --port 3771 --no-browser",
    url: "http://127.0.0.1:3771/api/session",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000
  }
});
