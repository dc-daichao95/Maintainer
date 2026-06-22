const { test, expect } = require('@playwright/test');

test.describe('Multi-Server Monitor UI', () => {
  let errors = [];

  test.beforeEach(async ({ page }) => {
    errors = [];
    page.on('pageerror', err => errors.push(err.message));
    page.on('console', msg => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });
    
    // Auto-accept all dialogs unconditionally
    page.on('dialog', dialog => dialog.accept());
    
    // Clear any existing servers before tests to ensure a clean state
    await page.goto('http://127.0.0.1:3000/#/server-config');
    await page.waitForSelector('table');
    
    // Attempt to delete all existing servers
    const deleteButtons = await page.locator('button:has-text("删除")').all();
    for (const btn of deleteButtons) {
        await btn.click();
        await page.waitForTimeout(500); // small wait for UI update
    }
  });

  test('Server Config CRUD and Dashboard features', async ({ page, context }) => {
    // 1. Navigate via sidebar to 服务器配置 / `#/server-config`
    await page.goto('http://127.0.0.1:3000/#/stats');
    await page.waitForSelector('nav');
    await page.click('a[href="#/server-config"]');
    await page.waitForSelector('table');
    
    // 2. ADD SERVER
    await page.click('button:has-text("添加服务器")');
    const modal = page.locator('#addServerModal');
    await expect(modal).toBeVisible();

    // Verify exactly 4 fields + 1 hidden ID field
    const visibleInputs = modal.locator('input:visible');
    await expect(visibleInputs).toHaveCount(4);
    
    // Check no SSH fields
    const modalText = await modal.innerText();
    expect(modalText.toLowerCase()).not.toContain('ssh password');
    expect(modalText.toLowerCase()).not.toContain('ssh user');

    // Fill in the 4 fields
    await page.fill('#serverName', 'Test Server');
    await page.fill('#serverIp', '192.168.1.100');
    await page.fill('#serverWebPort', '8080');
    await page.fill('#serverDescription', 'Test Description');
    await page.click('button:has-text("保存")');

    // Verify it persists and appears in the table
    await expect(modal).toBeHidden();
    const table = page.locator('table');
    await expect(table).toContainText('Test Server');
    await expect(table).toContainText('192.168.1.100');

    // Reload and verify persistence
    await page.reload();
    await page.waitForSelector('table');
    await expect(page.locator('table')).toContainText('Test Server');

    // 3. EDIT SERVER (with apostrophe)
    const editBtn = page.locator('button:has-text("编辑")').first();
    await editBtn.click();
    await expect(modal).toBeVisible();
    await page.fill('#serverName', "Test's Server");
    await page.click('button:has-text("保存")');
    await expect(modal).toBeHidden();
    await expect(page.locator('table')).toContainText("Test's Server");

    // DASHBOARD OFFLINE RENDERING
    await page.click('a[href="#/"]');
    await page.waitForSelector('#trendChart'); // Ensure charts load
    await page.waitForTimeout(1000); // Allow fetch to complete
    
    // Find the server card
    const card = page.locator('.bg-white.rounded-xl', { hasText: "Test's Server" }).first();
    await expect(card).toBeVisible();
    
    // Verify it shows offline since it's a fake IP
    await expect(card).toContainText('离线');
    
    // Verify average accuracy shows a sane value, not NaN%
    const avgAccuracy = page.locator('div', { hasText: /^平均准确率$/ }).locator('xpath=following-sibling::div');
    const accuracyText = await avgAccuracy.innerText();
    expect(accuracyText).not.toContain('NaN');
    expect(accuracyText).toContain('%');

    // NO RESTART / DETAILS BUTTONS
    const cardText = await card.innerText();
    expect(cardText).not.toContain('重启服务');
    expect(cardText).not.toContain('查看详情');

    // WHOLE CARD CLICK OPENS IN NEW TAB
    await page.evaluate(() => {
      window.openArgs = [];
      window.open = (url, target) => { window.openArgs.push({url, target}); return null; };
    });
    await card.click();
    const openArgs = await page.evaluate(() => window.openArgs);
    expect(openArgs.length).toBe(1);
    expect(openArgs[0].url).toBe('http://192.168.1.100:8080');
    expect(openArgs[0].target).toBe('_blank');

    // NAVIGATION AND NO ERRORS
    await page.click('a[href="#/server-config"]');
    await page.waitForSelector('table');
    await page.click('a[href="#/"]');
    await page.waitForSelector('#trendChart');
    
    const hasUncaughtCrash = errors.some(err => err.includes('TypeError') || err.includes('Cannot read properties of null'));
    expect(hasUncaughtCrash).toBe(false);

    // 4. DELETE SERVER
    await page.click('a[href="#/server-config"]');
    await page.waitForSelector('table');
    const delBtn = page.locator('button:has-text("删除")').first();
    await delBtn.click();
    await page.waitForTimeout(1000);
    await expect(page.locator('table')).not.toContainText("Test's Server");
  });
});