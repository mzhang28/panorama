import { parse } from "yaml";
import { dirname, join } from "node:path";
import { readFile, writeFile } from "node:fs/promises";
import { z } from "zod";

const FieldSpec = z.object({
  name: z.string(),
  indexed: z.boolean().default(false),
  type: z.string().default("String"),
});

const ModelSpec = z.object({
  fields: FieldSpec.array(),
});

const ModelSpecs = z.object({
  models: z.record(ModelSpec),
});

const data = ModelSpecs.parse(
  parse(await readFile(join(__dirname, "models.yaml"), { encoding: "utf-8" })),
);

export async function generatePrisma() {
  const lines = [];
  for (const [modelName, modelSpec] of Object.entries(data.models)) {
    for (const field of modelSpec.fields) {
      lines.push(`${modelName}__${field.name} String?`);
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
  id String @id @default(uuid(7))
  ${lines.join("\n")}
}
`;

  await writeFile(join(dirname(__dirname), "prisma", "schema.prisma"), code);
}
