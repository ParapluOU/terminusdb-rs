const { chromium } = require('playwright');

(async () => {
  console.log('Opening browser...');
  const browser = await chromium.launch({ headless: false, devtools: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  // Listen for all console messages
  page.on('console', msg => {
    const type = msg.type();
    console.log(`PAGE [${type.toUpperCase()}]:`, msg.text());
  });

  // Listen for page errors
  page.on('pageerror', error => {
    console.log('PAGE ERROR:', error.message);
  });

  // Listen for network requests
  page.on('request', request => {
    if (request.url().includes('api')) {
      console.log('API REQUEST:', request.method(), request.url());
    }
  });

  page.on('response', async response => {
    if (response.url().includes('api')) {
      console.log('API RESPONSE:', response.status(), response.url());
      if (response.status() !== 200) {
        const body = await response.text().catch(() => '');
        console.log('Response body:', body);
      }
    }
  });

  console.log('Navigating to http://localhost:8000...');
  await page.goto('http://localhost:8000');

  console.log('Waiting for page to load...');
  await page.waitForTimeout(5000);

  // Inject click tracking
  await page.evaluate(() => {
    document.addEventListener('click', (e) => {
      console.log('CLICK EVENT on:', e.target.tagName, e.target.className);
    }, true);
  });

  // Check that Elm loaded
  const elmLoaded = await page.evaluate(() => {
    return typeof Elm !== 'undefined' && Elm.Main;
  });
  console.log('Elm loaded:', elmLoaded);

  // Find nodes on the canvas
  const nodeCount = await page.locator('.node').count();
  console.log('Nodes found:', nodeCount);

  if (nodeCount > 0) {
    console.log('\nClicking first node (waiting 2s after click)...');
    await page.locator('.node').first().click({ force: true });
    await page.waitForTimeout(2000);

    // Check for popover elements
    const popoverTexts = await page.locator('text=Databases').allTextContents();
    console.log('Popover "Databases" elements found:', popoverTexts.length);

    // Check for any elements with database-related styling
    const dbElements = await page.evaluate(() => {
      const elements = Array.from(document.querySelectorAll('*'))
        .filter(el => {
          const text = el.textContent || '';
          return text.includes('Databases') || text.includes('database');
        });
      return elements.map(el => ({
        tag: el.tagName,
        text: el.textContent?.substring(0, 50),
        visible: el.offsetParent !== null
      }));
    });
    console.log('Elements with "database" text:', JSON.stringify(dbElements, null, 2));
  }

  console.log('\nTest paused - browser will stay open for 30 seconds...');
  console.log('Check the browser DevTools console for Elm messages');
  await page.waitForTimeout(30000);

  await browser.close();
  console.log('Browser closed.');
})();
