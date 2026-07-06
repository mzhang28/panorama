#!/usr/bin/env bun
/** Package a single .panoapp from manifest + WASM + optional UI build output. */

import { command, positional, option, optional, string, run } from "cmd-ts";
import JSZip from "jszip";
import * as fs from "fs";
import * as path from "path";

function walkDir(dir: string): string[] {
  const results: string[] = [];
  const list = fs.readdirSync(dir);
  for (const file of list) {
    const fullPath = path.join(dir, file);
    const stat = fs.statSync(fullPath);
    if (stat.isDirectory()) {
      results.push(...walkDir(fullPath));
    } else {
      results.push(fullPath);
    }
  }
  return results;
}

const app = command({
  name: "package-panoapp",
  description: "Package a single .panoapp file",
  args: {
    manifest: positional({
      type: string,
      displayName: "manifest",
      description: "Path to manifest.json",
    }),
    wasm: positional({
      type: optional(string),
      displayName: "wasm",
      description: "Path to WASM module",
    }),
    outDir: positional({
      type: string,
      displayName: "out_dir",
      description: "Output directory",
    }),
    uiDir: option({
      type: optional(string),
      long: "ui-dir",
      description: "Directory containing built UI files",
    }),
  },
  handler: async (args) => {
    const manifestContent = fs.readFileSync(args.manifest, "utf-8");
    const manifest = JSON.parse(manifestContent);

    const zip = new JSZip();
    zip.file("manifest.json", JSON.stringify(manifest));

    if (args.wasm && fs.existsSync(args.wasm)) {
      const wasmBuffer = fs.readFileSync(args.wasm);
      zip.file("plugin.wasm", wasmBuffer);
    }

    if (
      args.uiDir &&
      fs.existsSync(args.uiDir) &&
      fs.statSync(args.uiDir).isDirectory()
    ) {
      const files = walkDir(args.uiDir);
      for (const file of files) {
        const relPath = path.relative(args.uiDir, file);
        const fileContent = fs.readFileSync(file);
        zip.file(`ui/${relPath}`, fileContent);
      }
    }

    fs.mkdirSync(args.outDir, { recursive: true });
    const outPath = path.join(args.outDir, `${manifest.id}.panoapp`);
    const zipBuffer = await zip.generateAsync({
      type: "nodebuffer",
      compression: "DEFLATE",
    });
    fs.writeFileSync(outPath, zipBuffer);

    console.log(
      `  ${manifest.id}.panoapp (${fs.statSync(outPath).size} bytes)`,
    );
  },
});

run(app, process.argv.slice(2));
