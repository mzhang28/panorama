import { test as base, expect } from '@playwright/test';
import { spawnInstance, ServerInstance } from '../../scripts/instance';

type TestFixtures = {
  instance: ServerInstance;
};

export const test = base.extend<TestFixtures>({
  instance: async ({}, use) => {
    const instance = await spawnInstance();
    await use(instance);
    await instance.stop();
  },
  baseURL: async ({ instance }, use) => {
    await use(instance.url);
  },
});

export { expect };
