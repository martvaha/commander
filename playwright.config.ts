import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  use: {
    headless: true,
    viewport: { width: 800, height: 600 },
  },
});


