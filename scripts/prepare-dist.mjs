import { cp, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const sourceFile = resolve(projectRoot, "index.html");
const distDir = resolve(projectRoot, "dist");
const distFile = resolve(distDir, "index.html");

await mkdir(distDir, { recursive: true });
await cp(sourceFile, distFile);
