const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 }
  });
  await page.goto('http://127.0.0.1:3000/index.html');
  // wait for render
  await page.waitForTimeout(2000);
  await page.screenshot({ path: 'current_dashboard.png' });
  
  await page.click('a[href="#/issues"]');
  await page.waitForTimeout(1000);
  await page.screenshot({ path: 'current_issues.png' });
  
  await page.click('a[href="#/stats"]');
  await page.waitForTimeout(1000);
  await page.screenshot({ path: 'current_stats.png' });
  
  await browser.close();
})();
