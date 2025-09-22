// TODO: Dynamic imports?
import SQLiteESMFactory from "wa-sqlite/dist/wa-sqlite.mjs";
import SQLiteWasmURL from "wa-sqlite/dist/wa-sqlite.wasm?url";
import * as SQLite from "wa-sqlite";

async function setupSqlite() {
  const module = await SQLiteESMFactory({
    locateFile: (file: string) => {
      return file.endsWith(".wasm") ? SQLiteWasmURL : file;
    },
  });
  console.log("module", module);
  const sqlite3 = SQLite.Factory(module);
  const db = await sqlite3.open_v2("panorama");
  console.log("SHIET", db);
}
