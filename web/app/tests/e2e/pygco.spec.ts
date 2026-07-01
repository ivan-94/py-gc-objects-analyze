import { expect, test } from "@playwright/test";

test("overview, objects, graph, diff, sql, and report load against local API", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  await expect(page.getByText("Objects").first()).toBeVisible();
  await expect(page.getByText("Edges").first()).toBeVisible();

  await page.goto("/?page=objects&q=Widget&sort=object-id");
  await expect(page.getByRole("heading", { name: "Objects" })).toBeVisible();
  await expect(page.getByPlaceholder("filter")).toHaveValue("Widget");
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
  const beforeResize = await typeHeader.boundingBox();
  const resizeHandle = page.getByRole("button", { name: "Resize type column" });
  const handleBox = await resizeHandle.boundingBox();
  expect(beforeResize).not.toBeNull();
  expect(handleBox).not.toBeNull();
  await page.mouse.move(handleBox!.x + handleBox!.width / 2, handleBox!.y + handleBox!.height / 2);
  await page.mouse.down();
  await page.mouse.move(handleBox!.x + 70, handleBox!.y + handleBox!.height / 2);
  await page.mouse.up();
  await expect.poll(async () => (await typeHeader.boundingBox())?.width ?? 0).toBeGreaterThan((beforeResize?.width ?? 0) + 30);

  await page.getByRole("row", { name: /101/ }).first().click();
  await expect(page.getByRole("heading", { name: "101" })).toBeVisible();
  await expect(page.locator(".drawer").getByText("estimated reachable")).toBeVisible();
  await page.getByRole("button", { name: "Close" }).click();

  await page.goto("/?page=types&snapshot=2&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Types" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "redis.ConnectionPool" })).toBeVisible();
  await page.getByRole("row", { name: /redis\.ConnectionPool/ }).getByRole("button", { name: "View Objects" }).click();
  await expect(page.locator(".filter-pill").filter({ hasText: "redis.ConnectionPool" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "redis.ConnectionPool" })).toBeVisible();

  await page.goto("/?page=modules&snapshot=2&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Modules" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "redis" })).toBeVisible();
  await page.getByRole("row", { name: /redis/ }).getByRole("button", { name: "View Objects" }).click();
  await expect(page.locator(".filter-pill").filter({ hasText: "redis" })).toBeVisible();

  await page.goto("/?page=cohorts&snapshot=2&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Cohorts" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "database_cache" })).toBeVisible();
  await page.getByRole("row", { name: /database_cache/ }).getByRole("button", { name: "View Objects" }).click();
  await expect(page.locator(".filter-pill").filter({ hasText: "database_cache" })).toBeVisible();

  await page.goto("/?page=graph&root=100");
  await expect(page.getByRole("heading", { name: "Object Graph" })).toBeVisible();
  await expect(page.locator(".graph-panel svg circle")).not.toHaveCount(0);
  await expect(page.getByText("missing edge")).toBeVisible();
  await page.getByRole("button", { name: "Expand node 101" }).click();
  await expect(page).toHaveURL(/root=101/);
  await expect(page.locator('[data-node-id="101"] circle.root-node')).toBeVisible();

  await page.goto("/?page=graph&snapshot=3&root=10");
  await expect(page.locator(".graph-panel svg circle.stub-node")).not.toHaveCount(0);

  await page.goto("/?page=graph&snapshot=4&root=20");
  await expect(page.locator(".graph-panel svg line.missing-edge")).not.toHaveCount(0);
  await expect(page.locator(".graph-panel svg circle.missing-node")).not.toHaveCount(0);

  await page.goto("/?page=diff&from=1&to=2");
  await expect(page.getByRole("heading", { name: "Diff" })).toBeVisible();
  await expect(page.getByText("high")).toBeVisible();
  await expect(page.locator(".metric").filter({ hasText: "Object Delta" }).getByText("+1", { exact: true })).toBeVisible();
  await expect(page.getByRole("cell", { name: "102" })).toBeVisible();

  await page.goto("/?page=diff&from=1&to=3");
  await expect(page.getByText("Use aggregate-only interpretation")).toBeVisible();

  await page.goto("/?page=findings&snapshot=4");
  await expect(page.getByRole("heading", { name: "Findings" })).toBeVisible();
  await page.locator(".data-table .icon-button").first().click();
  await expect(page.locator(".drawer")).toBeVisible();
  await expect(page.getByRole("button", { name: "Copy JSON" })).toBeVisible();
  await page.getByRole("button", { name: "Close" }).click();

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
  await expect(page.locator(".drawer").getByText('"explain": true')).toBeVisible();
  await page.getByRole("button", { name: "Close" }).click();
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
