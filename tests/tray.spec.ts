import { test, expect } from '@playwright/test';

test('dev server loads app shell', async ({ page }) => {
  await page.goto('http://localhost:1420');
  await expect(page.locator('h1')).toHaveText('🎤 Commander');
});


