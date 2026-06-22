import { test, expect } from '@playwright/test';

test.describe('Issue Handling Stats UI', () => {
  test.beforeEach(async ({ page }) => {
    page.on('console', msg => console.log('BROWSER CONSOLE:', msg.text()));
    page.on('pageerror', error => console.log('BROWSER ERROR:', error.message));
    page.on('response', response => {
      if (response.status() === 404) {
        console.log('404 Not Found:', response.url());
      }
    });
    // Navigate to the local server
    await page.goto('/#/stats');
  });

  test('Dashboard should display overall accuracy and charts', async ({ page }) => {
    // Wait for the Stats view to be visible (by checking the title)
    await page.waitForSelector('h1:has-text("Statistics Dashboard")');
    
    // Check if the accuracy card is displayed
    const accuracyCard = page.locator('h3:has-text("Overall Accuracy")').first();
    await expect(accuracyCard).toBeVisible();
    
    // Check for the presence of the charts
    const trendChart = page.locator('#weeklyChart');
    const subsystemChart = page.locator('#subsystemChart');
    
    await expect(trendChart).toBeVisible();
    await expect(subsystemChart).toBeVisible();
  });

  test('Issue List should support filtering, pagination, and inline editing', async ({ page }) => {
    // Switch to Issues tab
    await page.click('a[href="#/issues"]');
    await page.waitForSelector('h1:has-text("Issue List")');

    // Check if table is populated
    const rows = page.locator('#table-container tbody tr');
    await expect(rows).toHaveCount(10); // Assuming 10 items per page

    // Test filtering by Subsystem
    await page.selectOption('#filter-subsystem', 'net');
    // Wait for table to update
    await page.waitForTimeout(500);
    
    // Verify that the rows now only contain 'net' subsystem
    const netRowsCount = await rows.count();
    for (let i = 0; i < netRowsCount; i++) {
        const subsystemText = await rows.nth(i).locator('td').nth(1).textContent();
        expect(subsystemText?.trim()).toBe('net');
    }

    // Test pagination
    // Clear filter
    await page.selectOption('#filter-subsystem', 'All');
    await page.waitForTimeout(500);
    
    const allRowsCount = await rows.count();
    console.log('Rows count after clearing filter:', allRowsCount);
    
    // Click next page
    await page.click('#nextPage');
    await page.waitForTimeout(500);
    
    const pageInfo = await page.locator('#table-container').textContent();
    expect(pageInfo).toContain('Showing page 2');

    // Test inline editing
    // Change feedback of the first row
    const firstRowSelect = rows.nth(0).locator('select.status-select');
    await firstRowSelect.selectOption('TP');
    
    const firstRowInput = rows.nth(0).locator('input.comment-input');
    await firstRowInput.fill('This is a true positive');
    
    // Trigger blur or change event to save
    await firstRowInput.blur();
    // Also trigger change event directly just in case
    await firstRowInput.evaluate(node => {
        node.dispatchEvent(new Event('change', { bubbles: true }));
    });
    
    // Reload page and check if the state is persisted
    // Navigate to issues route directly
    await page.goto('/#/issues');
    await page.waitForSelector('h1:has-text("Issue List")');
    
    const reloadedSelect = page.locator('#table-container tbody tr').nth(0).locator('select.status-select');
    const reloadedInput = page.locator('#table-container tbody tr').nth(0).locator('input.comment-input');
    
    await expect(reloadedSelect).toHaveValue('TP');
    await expect(reloadedInput).toHaveValue('This is a true positive');
  });
});
