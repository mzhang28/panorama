// TODO: Dynamic imports?
import SQLiteESMFactory from "wa-sqlite/dist/wa-sqlite.mjs";
// @ts-expect-error No typings for WASm.
import SQLiteWasmURL from "wa-sqlite/dist/wa-sqlite.wasm?url";
import * as SQLite from "wa-sqlite";
import { SqliteService } from "../lib/services/sqliteService";

export class WASqliteService extends SqliteService {
  private sqlite3: any = null;
  private db: any = null;

  async init(): Promise<void> {
    if (this.db) return;

    const module = await SQLiteESMFactory({
      locateFile: (file: string) => {
        return file.endsWith(".wasm") ? SQLiteWasmURL : file;
      },
    });

    this.sqlite3 = SQLite.Factory(module);
    this.db = await this.sqlite3.open_v2("panorama");

    await this.initSchema();
  }

  private async initSchema(): Promise<void> {
    // Create node table
    await this.sqlite3.run(
      this.db,
      `
      CREATE TABLE IF NOT EXISTS node (
        id TEXT PRIMARY KEY,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        type TEXT NOT NULL,
        extraFields TEXT
      )
    `,
    );

    // Create _panorama_tables table
    await this.sqlite3.run(
      this.db,
      `
      CREATE TABLE IF NOT EXISTS _panorama_tables (
        field_name TEXT PRIMARY KEY,
        table_name TEXT NOT NULL,
        is_indexed BOOLEAN DEFAULT 0
      )
    `,
    );

    // Create _panorama_node_field_map table
    await this.sqlite3.run(
      this.db,
      `
      CREATE TABLE IF NOT EXISTS _panorama_node_field_map (
        node_id TEXT NOT NULL,
        field_name TEXT NOT NULL,
        PRIMARY KEY (node_id, field_name),
        FOREIGN KEY (node_id) REFERENCES node(id) ON DELETE CASCADE
      )
    `,
    );
  }

  async query(sql: string, params?: any[]): Promise<any[]> {
    if (!this.db || !this.sqlite3) {
      throw new Error("Database not initialized. Call init() first.");
    }

    // For now, use a simple approach - if it's a SELECT query, try to execute it
    // and return empty array if it fails
    if (sql.trim().toUpperCase().startsWith("SELECT")) {
      try {
        const results: any[] = [];
        await this.sqlite3.exec(
          this.db,
          sql,
          (row: any[], columnNames: string[]) => {
            const rowObj: any = {};
            columnNames.forEach((name, index) => {
              rowObj[name] = row[index];
             });
            results.push(rowObj);
          },
        );
        return results;
      } catch (error) {
        console.error("SQLite query error:", error, "SQL:", sql);
        // Return empty array for failed queries
        return [];
      }
    } else {
      // For non-SELECT queries, just execute
      await this.exec(sql, params);
      return [];
    }
  }

  async exec(sql: string, params?: any[]): Promise<void> {
    if (!this.db || !this.sqlite3) {
      throw new Error("Database not initialized. Call init() first.");
    }

    await this.sqlite3.run(this.db, sql, params || []);
  }
}

export const sqliteService = new WASqliteService();
