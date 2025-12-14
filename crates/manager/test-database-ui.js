const { chromium } = require('playwright');

(async () => {
  console.log('Opening browser...');
  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();

  // Listen for console messages
  page.on('console', msg => console.log('PAGE LOG:', msg.text()));

  console.log('Navigating to http://localhost:8000...');
  await page.goto('http://localhost:8000');

  console.log('Waiting for page to load...');
  await page.waitForTimeout(3000);

  // Check that Elm loaded
  const elmLoaded = await page.evaluate(() => {
    return typeof Elm !== 'undefined' && Elm.Main;
  });
  console.log('Elm loaded:', elmLoaded);

  // Find nodes on the canvas
  const nodeCount = await page.locator('.node').count();
  console.log('Nodes found:', nodeCount);

  if (nodeCount > 0) {
    console.log('\n--- Testing Database Popover ---');

    // Click on the first node
    console.log('Clicking first node...');
    await page.locator('.node').first().click();

    // Wait for popover to appear
    await page.waitForTimeout(1000);

    // Check if popover is visible
    const popoverVisible = await page.locator('text=Databases').isVisible().catch(() => false);
    console.log('Database popover visible:', popoverVisible);

    if (popoverVisible) {
      // Count databases in popover
      const dbItems = await page.locator('[style*="cursor: pointer"][style*="border-bottom"]').count();
      console.log('Database items in popover:', dbItems);

      if (dbItems > 0) {
        console.log('\n--- Testing Database View Modal ---');

        // Click on first database
        console.log('Clicking first database...');
        await page.locator('[style*="cursor: pointer"][style*="border-bottom"]').first().click();

        // Wait for modal to appear
        await page.waitForTimeout(1000);

        // Check if modal is visible
        const modalVisible = await page.locator('.modal-overlay').isVisible().catch(() => false);
        console.log('Database view modal visible:', modalVisible);

        if (modalVisible) {
          // Check tabs
          const modelsTabVisible = await page.locator('text=/Models \\(\\d+\\)/').isVisible().catch(() => false);
          const commitsTabVisible = await page.locator('text=/Commits \\(\\d+\\)/').isVisible().catch(() => false);
          const remotesTabVisible = await page.locator('text=/Remotes \\(\\d+\\)/').isVisible().catch(() => false);

          console.log('Models tab visible:', modelsTabVisible);
          console.log('Commits tab visible:', commitsTabVisible);
          console.log('Remotes tab visible:', remotesTabVisible);

          // Try clicking different tabs
          if (commitsTabVisible) {
            console.log('Clicking Commits tab...');
            await page.locator('text=/Commits \\(\\d+\\)/').click();
            await page.waitForTimeout(500);

            // Check if commits are displayed
            const commitsDisplayed = await page.locator('text=/commit/i').isVisible().catch(() => false);
            console.log('Commits displayed:', commitsDisplayed);
          }

          if (remotesTabVisible) {
            console.log('Clicking Remotes tab...');
            await page.locator('text=/Remotes \\(\\d+\\)/').click();
            await page.waitForTimeout(500);

            // Check for add remote button
            const addRemoteButton = await page.locator('text=Add Remote').isVisible().catch(() => false);
            console.log('Add Remote button visible:', addRemoteButton);
          }

          if (modelsTabVisible) {
            console.log('Clicking Models tab...');
            await page.locator('text=/Models \\(\\d+\\)/').click();
            await page.waitForTimeout(500);

            // Check if models are displayed
            const modelsDisplayed = await page.locator('text=/Model Name|Instances/i').isVisible().catch(() => false);
            console.log('Models displayed:', modelsDisplayed);
          }

          // Close modal by clicking X button
          console.log('Closing modal...');
          const closeButton = await page.locator('button:has-text("×")').first();
          if (await closeButton.isVisible().catch(() => false)) {
            await closeButton.click();
            await page.waitForTimeout(500);

            const modalClosed = await page.locator('.modal-overlay').isVisible().catch(() => false);
            console.log('Modal closed:', !modalClosed);
          }
        }
      }
    } else {
      console.log('WARNING: Popover did not appear after clicking node');
    }
  } else {
    console.log('WARNING: No nodes found on canvas');
  }

  console.log('\n--- Test Complete ---');
  console.log('Browser will stay open for 15 seconds so you can interact with it...');
  await page.waitForTimeout(15000);

  await browser.close();
  console.log('Browser closed.');
})();
