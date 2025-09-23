import { GraphQLExecutor } from "./graphql";
import { SqliteService } from "./services/sqliteService";
import journalManifestPath from "../../apps/journal/manifest.json?url";

export interface AppManifest {
  name: string;
  description: string;
  version: string;
  indexedFields?: Array<{
    name: string;
    description: string;
    type: string;
    indexed: boolean;
  }>;
  nodeType: string;
}

export class AppManager {
  private apps: Map<
    string,
    { manifest: AppManifest; executor: GraphQLExecutor }
  > = new Map();

  constructor(private db: SqliteService) {}

  async discoverApps(): Promise<void> {
    // Load known apps directly (for now, hardcoded but will be dynamic later)
    const knownApps = [
      { name: "journal", manifestPath: "/apps/journal/manifest.json" },
    ];

    for (const app of knownApps) {
      try {
        // Try to load manifest from URL first
        let manifest = await this.loadManifest(app.manifestPath);

        // If that fails, try alternative paths
        if (!manifest) {
          const altPaths = [
            "./apps/journal/manifest.json",
            "apps/journal/manifest.json",
            journalManifestPath,
          ];

          for (const altPath of altPaths) {
            manifest = await this.loadManifest(altPath);
            if (manifest) break;
          }
        }

        if (manifest && manifest.name) {
          const appName = manifest.name.toLowerCase().replace(/\s+/g, "");
          const executor = new GraphQLExecutor(this.db, manifest);
          await executor.init();
          this.apps.set(appName, { manifest, executor });
          console.log(
            `AppManager: Successfully loaded app '${appName}' from manifest:`,
            manifest,
          );
        } else {
          console.error(
            `AppManager: Failed to load valid manifest for app '${app.name}' from ${app.manifestPath}`,
          );
        }
      } catch (error) {
        console.error(
          `AppManager: Error loading app '${app.name}':`,
          error instanceof Error ? error.message : String(error),
        );
      }
    }

    if (this.apps.size === 0) {
      console.error("AppManager: No apps were successfully loaded!");
    } else {
      console.log(
        `AppManager: Loaded ${this.apps.size} app(s): ${Array.from(this.apps.keys()).join(", ")}`,
      );
    }
  }

  private async loadManifest(path: string): Promise<AppManifest | null> {
    try {
      console.log(`AppManager: Fetching manifest from ${path}`);
      const response = await fetch(path);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }
      const manifest = await response.json();
      console.log(`AppManager: Loaded manifest:`, manifest);
      return manifest;
    } catch (error) {
      console.error(`AppManager: Failed to load manifest from ${path}:`, error);
      return null;
    }
  }

  getExecutor(appName: string): GraphQLExecutor | null {
    const app = this.apps.get(appName);
    return app ? app.executor : null;
  }

  getApps(): string[] {
    return Array.from(this.apps.keys());
  }

  getManifest(appName: string): AppManifest | null {
    const app = this.apps.get(appName);
    return app ? app.manifest : null;
  }
}
