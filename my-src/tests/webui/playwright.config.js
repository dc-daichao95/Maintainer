import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: 'e2e.test.js',
  use: {
    channel: 'msedge',
    headless: true,
    baseURL: 'http://127.0.0.1:8080',
  },
});
