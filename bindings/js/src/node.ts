import { readFile } from "node:fs/promises";
import { load, type Entityres } from "./index.ts";

export * from "./index.ts";

export async function loadNode(): Promise<Entityres> {
  return load(await readFile(new URL("../entityres.wasm", import.meta.url)));
}
