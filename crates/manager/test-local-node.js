const { chromium } = require('playwright');

(async () => {
  console.log('Opening browser...');
  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();

  // Listen for console messages
  page.on('console', msg => console.log('PAGE LOG:', msg.text()));

  // Listen for API responses
  page.on('response', async response => {
    if (response.url().includes('/databases')) {
      console.log('DATABASES API:', response.status(), response.url());
      const body = await response.json().catch(() => null);
      console.log('Response:', JSON.stringify(body, null, 2));
    }
  });

  console.log('Navigating to http://localhost:8000...');
  await page.goto('http://localhost:8000');

  console.log('Waiting for page to load...');
  await page.waitForTimeout(5000);

  // Find the "Local Instance" node (index 2, since test_node is first)
  const nodes = await page.locator('.node').all();
  console.log('Total nodes found:', nodes.length);

  // Click on the last node (should be "Local Instance" or another accessible one)
  console.log('\nClicking on last node (should be accessible)...');
  await nodes[nodes.length - 1].click();

  console.log('Waiting for popover...');
  await page.waitForTimeout(2000);

  // Check for popover
  const popoverVisible = await page.locator('h2:has-text("Databases")').isVisible().catch(() => false);
  console.log('Popover visible:', popoverVisible);

  if (popoverVisible) {
    // Take a screenshot
    await page.screenshot({ path: 'popover-screenshot.png' });
    console.log('Screenshot saved: popover-screenshot.png');

    // Count database items
    const dbCount = await page.locator('[style*="cursor: pointer"]').count();
    console.log('Database items:', dbCount);

    // Click on first database
    if (dbCount > 0) {
      console.log('\nClicking first database...');
      await page.locator('[style*="cursor: pointer"]').first().click({ timeout: 5000 });

      await page.waitForTimeout(1000);

      // Check for modal
      const modalVisible = await page.locator('.modal-overlay').isVisible().catch(() => false);
      console.log('Modal visible:', modalVisible);

      if (modalVisible) {
        await page.screenshot({ path: 'modal-screenshot.png' });
        console.log('Screenshot saved: modal-screenshot.png');
      }
    }
  } else {
    console.log('WARNING: Popover not visible. Taking screenshot for debugging...');
    await page.screenshot({ path: 'no-popover-screenshot.png' });
    console.log('Screenshot saved: no-popover-screenshot.png');
  }

  console.log('\nBrowser will stay open for 20 seconds...');
  await page.waitForTimeout(20000);

  await browser.close();
  console.log('Browser closed.');
})();
