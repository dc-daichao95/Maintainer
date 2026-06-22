import { chromium } from '@playwright/test';

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  page.on('console', msg => console.log('BROWSER CONSOLE:', msg.text()));
  page.on('pageerror', error => console.log('BROWSER ERROR:', error));
  await page.goto('http://127.0.0.1:3000/index.html');
  await page.waitForTimeout(1000);
  await browser.close();
})();
