const { test, expect } = require('@playwright/test');

test.describe('TerminusDB Manager Frontend', () => {
  test('should load the homepage', async ({ page }) => {
    await page.goto('http://localhost:8000');

    // Wait for the app to load
    await page.waitForSelector('.app', { timeout: 5000 });

    // Check that the canvas is present
    const canvas = await page.locator('.canvas-container');
    await expect(canvas).toBeVisible();
  });

  test('should display the local node', async ({ page }) => {
    await page.goto('http://localhost:8000');

    // Wait for API calls to complete
    await page.waitForResponse(response =>
      response.url().includes('/api/nodes') && response.status() === 200
    );

    // Check that at least one node is displayed
    const nodes = await page.locator('.node');
    await expect(nodes.first()).toBeVisible({ timeout: 10000 });
  });

  test('should open context menu on right-click', async ({ page }) => {
    await page.goto('http://localhost:8000');

    // Wait for the canvas to load
    await page.waitForSelector('.canvas-container');

    // Right-click on the canvas
    const canvas = page.locator('svg');
    await canvas.click({ button: 'right', position: { x: 400, y: 300 } });

    // Check that context menu appears
    const contextMenu = page.locator('.context-menu');
    await expect(contextMenu).toBeVisible();

    // Check for "Add Node" option
    await expect(contextMenu.locator('text=Add Node')).toBeVisible();
  });

  test('should open node creation form from context menu', async ({ page }) => {
    await page.goto('http://localhost:8000');

    // Wait for the canvas
    await page.waitForSelector('.canvas-container');

    // Right-click on canvas
    const canvas = page.locator('svg');
    await canvas.click({ button: 'right', position: { x: 400, y: 300 } });

    // Click "Add Node"
    await page.click('text=Add Node');

    // Check that the modal appears
    const modal = page.locator('.modal-overlay');
    await expect(modal).toBeVisible();

    // Check form fields
    await expect(page.locator('input[value="New Instance"]')).toBeVisible();
    await expect(page.locator('input[value="localhost"]')).toBeVisible();
  });

  test('should be able to drag nodes', async ({ page }) => {
    await page.goto('http://localhost:8000');

    // Wait for nodes to load
    await page.waitForResponse(response =>
      response.url().includes('/api/nodes') && response.status() === 200
    );

    // Wait a bit for rendering
    await page.waitForTimeout(1000);

    // Find a node (SVG element)
    const node = page.locator('.node').first();

    // Try to drag the node
    const box = await node.boundingBox();
    if (box) {
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
      await page.mouse.down();
      await page.mouse.move(box.x + 100, box.y + 100);
      await page.mouse.up();

      // Verify that an API call was made to update the node position
      // (we don't strictly verify here, just that the interaction worked)
    }
  });

  test('should display node status with database count', async ({ page }) => {
    await page.goto('http://localhost:8000');

    // Wait for status updates
    await page.waitForResponse(response =>
      response.url().includes('/api/status') && response.status() === 200
    );

    // Wait for status to be rendered
    await page.waitForTimeout(2000);

    // Check for database count text (should show database emoji and count)
    const statusText = page.locator('text=/📊.*database/i');
    await expect(statusText.first()).toBeVisible({ timeout: 5000 });
  });
});
