import { expect, test } from "@playwright/test";

test("overview, objects, graph, diff, sql, and report load against local API", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  await expect(page.getByText("Objects").first()).toBeVisible();
  await expect(page.getByText("Edges").first()).toBeVisible();

  await page.goto("/?page=objects&q=Widget&sort=object-id");
  await expect(page.getByRole("heading", { name: "Objects" })).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Filter objects" })).toHaveValue("Widget");
  await expect(page.getByRole("cell", { name: "app.Widget" }).first()).toBeVisible();
  await expect(page).toHaveURL(/page=objects/);
  await expect(page).toHaveURL(/q=Widget/);

  await page.goto("/?page=objects&snapshot=2&sort=object-id&limit=1&offset=0");
  await expect(page.getByText("1-1 of 3")).toBeVisible();
  await expect(page.getByRole("cell", { name: "102" })).toBeVisible();
  await page.getByRole("button", { name: "Next" }).click();
  await expect(page).toHaveURL(/offset=1/);
  await expect(page.getByRole("cell", { name: "101" })).toBeVisible();

  const typeHeader = page.locator("thead th").nth(1);
  await expect(typeHeader).toBeVisible();
  await expect.poll(async () => (await typeHeader.boundingBox())?.width ?? 0).toBeGreaterThan(120);

  await page.getByRole("cell", { name: "101" }).click();
  await expect(page.getByRole("heading", { name: "101" })).toBeVisible();
  await expect(page.getByRole("dialog").getByText("estimated reachable")).toBeVisible();
  await page.getByRole("button", { name: "Close sheet" }).click();

  await page.goto("/?page=types&snapshot=2&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Types" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "redis.ConnectionPool" })).toBeVisible();
  await page.locator("tr").filter({ hasText: "redis.ConnectionPool" }).getByRole("button", { name: "View" }).click();
  await expect(page).toHaveURL(/type=redis\.ConnectionPool/);
  await expect(page.getByRole("cell", { name: "redis.ConnectionPool" })).toBeVisible();

  await page.goto("/?page=modules&snapshot=2&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Modules" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "redis" })).toBeVisible();
  await page.locator("tr").filter({ hasText: "redis" }).getByRole("button", { name: "View" }).click();
  await expect(page).toHaveURL(/module=redis/);

  await page.goto("/?page=cohorts&snapshot=2&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Cohorts" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "database_cache" })).toBeVisible();
  await page.locator("tr").filter({ hasText: "database_cache" }).getByRole("button", { name: "View" }).click();
  await expect(page).toHaveURL(/cohort=database_cache/);

  await page.goto("/?page=graph&root=100");
  await expect(page.getByRole("heading", { name: "Object Graph" })).toBeVisible();
  await expect(page.getByText(/nodes/).first()).toBeVisible();
  await expect(page.getByText(/edges/).first()).toBeVisible();
  await expect(page.getByText("Graph controls")).toBeVisible();
  await expect(page.getByText("reference").first()).toBeVisible();

  await page.goto("/?page=graph&snapshot=3&root=10");
  await expect(page.getByRole("button", { name: /stub/ }).first()).toBeVisible();

  await page.goto("/?page=graph&snapshot=4&root=20");
  await expect(page.getByRole("button", { name: /missing/ }).first()).toBeVisible();

  await page.goto("/?page=diff&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Diff" })).toBeVisible();
  await expect(page.getByText("high")).toBeVisible();
  await expect(page.getByText("Object Delta")).toBeVisible();
  await expect(page.getByText("+1", { exact: true }).first()).toBeVisible();
  await expect(page.getByRole("cell", { name: "102" })).toBeVisible();

  await page.goto("/?page=diff&from=1&to=3");
  await expect(page.getByText("Use aggregate-only interpretation")).toBeVisible();

  await page.goto("/?page=findings&snapshot=4");
  await expect(page.getByRole("heading", { name: "Findings" })).toBeVisible();
  await page.getByTitle("Open JSON").first().click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByRole("button", { name: "Copy JSON" })).toBeVisible();
  await page.getByRole("button", { name: "Close sheet" }).click();

  await page.goto("/?page=sql");
  await expect(page.getByRole("heading", { name: "SQL" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Schema" })).toBeVisible();
  await expect(page.locator(".schema-browser").getByRole("button", { name: "objects", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText('"rows"')).toBeVisible();
  const idsetName = `Smoke SQL ids ${Date.now()}`;
  await page.getByPlaceholder("idset name").fill(idsetName);
  await page.getByRole("button", { name: "Save Idset" }).click();
  const savedIdsetRow = page.locator(".saved-idset-list > div").filter({ hasText: idsetName });
  await expect(savedIdsetRow).toBeVisible();
  await savedIdsetRow.getByRole("button", { name: "Use" }).click();
  await expect(page.locator(".sql-editor")).toHaveValue(/saved_idset_objects/);
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText('"rows"')).toBeVisible();
  await page.getByRole("button", { name: "Explain" }).click();
  await expect(page.getByRole("heading", { name: "SQL Explain Plan" })).toBeVisible();
  await expect(page.getByRole("dialog").getByText('"explain": true')).toBeVisible();
  await page.getByRole("button", { name: "Close sheet" }).click();
  await page.locator(".sql-editor").fill("delete from objects");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText("query_failed")).toBeVisible();
  await expect(page.getByText("Next step:")).toBeVisible();
  await expect(page.getByText("Use a SELECT or WITH query")).toBeVisible();
  await page.locator(".sql-editor").fill("select object_id, type, shallow_size from objects limit 20");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText('"rows"')).toBeVisible();
  await page.getByRole("button", { name: "Run Job" }).click();
  await expect(page.locator(".job-status")).toBeVisible();
  await expect(page.locator(".job-status").getByText(/succeeded|running|queued/)).toBeVisible();

  await page.goto("/?page=report");
  await expect(page.getByRole("heading", { name: "Report" })).toBeVisible();
  await expect(page.getByText("# Memory Forensics Report")).toBeVisible();
});
