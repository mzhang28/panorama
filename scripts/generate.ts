import { $ } from "bun";
import { generatePrisma } from "../models/generate";
import { dirname } from "node:path";

const root = dirname(dirname(new URL(import.meta.url).pathname));

const depend = async <T, U>(d: Promise<T>, e: () => Promise<U>): Promise<U> => {
  await d;
  return e();
};

const prismaFile = generatePrisma();

await Promise.all([
  // prismaFile,
  // depend(prismaFile, () => $`prisma format`),
  // depend(prismaFile, () => $`prisma generate`),
  $`peggy -o ${root}/generated/searchQuery.js --dts -m ${root}/generated/searchQuery.map.js ${root}/backend/query.pegjs`,
]);
