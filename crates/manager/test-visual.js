const { chromium } = require('playwright');

(async () => {
  console.log('Opening browser...');
  const browser = await chromium.launch({
    headless: false,
    slowMo: 500  // Slow down by 500ms
  });

  const page = await browser.newPage();

  // Enable console logging
  page.on('console', msg => console.log('PAGE LOG:', msg.text()));
  page.on('pageerror', error => console.log('PAGE ERROR:', error.message));

  console.log('Navigating to http://localhost:8000...');
  await page.goto('http://localhost:8000');

  console.log('Waiting for page to load...');
  await page.waitForTimeout(3000);

  // Check if Elm loaded
  const elmLoaded = await page.evaluate(() => typeof window.Elm !== 'undefined');
  console.log('Elm loaded:', elmLoaded);

  // Check if app element exists
  const appExists = await page.locator('.app').count();
  console.log('App elements found:', appExists);

  // Check if canvas exists
  const canvasExists = await page.locator('.canvas-container').count();
  console.log('Canvas elements found:', canvasExists);

  // Try to find nodes
  await page.waitForTimeout(2000);
  const nodeCount = await page.locator('.node').count();
  console.log('Nodes found:', nodeCount);

  console.log('\nBrowser will stay open for 30 seconds so you can interact with it...');
  await page.waitForTimeout(30000);

  await browser.close();
  console.log('Browser closed.');
})().catch(error => {
  console.error('Error:', error.message);
  process.exit(1);
});
