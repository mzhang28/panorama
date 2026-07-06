import { test as base, expect } from "@playwright/test";
import { spawnInstance, ServerInstance } from "../../scripts/instance";

type WorkerFixtures = {
  serverInstance: ServerInstance;
};

export const test = base.extend<{}, WorkerFixtures>({
  serverInstance: [
    async ({}, use) => {
      const instance = await spawnInstance();
      await use(instance);
      await instance.stop();
    },
    { scope: "worker", auto: true },
  ],
  baseURL: async ({ serverInstance }, use) => {
    await use(serverInstance.url);
  },
});

export { expect };
