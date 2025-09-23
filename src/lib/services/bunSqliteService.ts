import Database from "bun:sqlite";
import { SqliteService } from "./sqliteService";

export class BunSqliteService extends SqliteService {
  private db: Database;

  constructor(databasePath?: string) {
    super();
    this.db = new Database(databasePath ?? ":memory:");
  }

  async init(): Promise<void> {
    // Create tables if they don't exist
    this.db.run(`
      CREATE TABLE IF NOT EXISTS node (
        id TEXT PRIMARY KEY,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        type TEXT NOT NULL,
        extraFields TEXT
      )
    `);

    // Create _panorama_tables table
    this.db.run(`
      CREATE TABLE IF NOT EXISTS _panorama_tables (
        field_name TEXT PRIMARY KEY,
        table_name TEXT NOT NULL,
        is_indexed BOOLEAN DEFAULT 0
      )
    `);

    // Create _panorama_node_field_map table
    this.db.run(`
      CREATE TABLE IF NOT EXISTS _panorama_node_field_map (
        node_id TEXT NOT NULL,
        field_name TEXT NOT NULL,
        PRIMARY KEY (node_id, field_name),
        FOREIGN KEY (node_id) REFERENCES node(id) ON DELETE CASCADE
      )
    `);
  }

  async query(sql: string, params?: any[]): Promise<any[]> {
    try {
      const stmt = this.db.prepare(sql);
      if (params) {
        return stmt.all(...params);
      } else {
        return stmt.all();
      }
    } catch (error) {
      console.error("SQLite query error:", error, "SQL:", sql);
      return [];
    }
  }

  async exec(sql: string, params?: any[]): Promise<void> {
    try {
      const stmt = this.db.prepare(sql);
      if (params) {
        stmt.run(...params);
      } else {
        stmt.run();
      }
    } catch (error) {
      console.error("SQLite exec error:", error, "SQL:", sql);
      throw error;
    }
  }

  // Helper method to inspect tables for testing
  getTable(name: string): any[] {
    return this.db.prepare(`SELECT * FROM ${name}`).all();
  }

  close(): void {
    this.db.close();
  }
}
