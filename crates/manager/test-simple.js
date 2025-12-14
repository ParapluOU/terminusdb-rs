const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();

  // Enable console logging
  page.on('console', msg => console.log('PAGE LOG:', msg.text()));
  page.on('pageerror', error => console.log('PAGE ERROR:', error.message));

  console.log('Navigating to http://localhost:8000...');
  await page.goto('http://localhost:8000');

  // Wait for the app element
  console.log('Waiting for .app element...');
  await page.waitForSelector('.app', { timeout: 10000 });
  console.log('✓ .app element found!');

  // Wait for canvas
  console.log('Waiting for .canvas-container...');
  await page.waitForSelector('.canvas-container', { timeout: 10000 });
  console.log('✓ .canvas-container found!');

  // Wait for API calls
  console.log('Waiting for API /api/nodes call...');
  await page.waitForResponse(response =>
    response.url().includes('/api/nodes') && response.status() === 200,
    { timeout: 10000 }
  );
  console.log('✓ /api/nodes responded!');

  console.log('Waiting for API /api/status call...');
  await page.waitForResponse(response =>
    response.url().includes('/api/status') && response.status() === 200,
    { timeout: 10000 }
  );
  console.log('✓ /api/status responded!');

  // Take a screenshot
  await page.screenshot({ path: 'manager-screenshot.png', fullPage: true });
  console.log('✓ Screenshot saved to manager-screenshot.png');

  // Check for nodes
  const nodeCount = await page.locator('.node').count();
  console.log(`✓ Found ${nodeCount} node(s) on canvas`);

  await browser.close();

  console.log('\n✅ All tests passed! Frontend is working correctly.');
})().catch(error => {
  console.error('❌ Test failed:', error.message);
  process.exit(1);
});
