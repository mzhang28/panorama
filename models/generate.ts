import { parse } from "yaml";
import { dirname, join } from "node:path";
import { readFile, writeFile } from "node:fs/promises";

const data = parse(
  await readFile(join(__dirname, "models.yaml"), { encoding: "utf-8" })
);

const lines = [];
for (const [modelName, modelSpec] of Object.entries(data.models)) {
  for (const field of modelSpec.fields) {
    lines.push(`${modelName}__${field} String?`);
  }
}

const code = `
generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model Node {
  id String @id @default(uuid())
  ${lines.join("\n")}
}
`;

await writeFile(join(dirname(__dirname), "prisma", "schema.prisma"), code);
