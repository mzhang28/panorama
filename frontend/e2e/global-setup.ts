import { spawnInstance, ServerInstance } from "../../scripts/instance";

let globalInstance: ServerInstance | undefined;

export default async function globalSetup() {
  console.log("Starting single shared Panorama server instance...");
  globalInstance = await spawnInstance();
  process.env.PLAYWRIGHT_BASE_URL = globalInstance.url;

  return async () => {
    if (globalInstance) {
      console.log("Stopping shared Panorama server instance...");
      await globalInstance.stop();
      globalInstance = undefined;
    }
  };
}
