import type { Answer, Input, Json, Output, Question } from "./types.ts";

export type * from "./types.ts";

export type Judge = (state: Json, questions: Record<string, Json>) => Promise<Json>;

export interface ResolveOptions {
  maxRounds?: number;
  concurrency?: number;
}

export interface Resolution {
  output: Output;
  answers: Answer[];
}

export interface Entityres {
  run(input: Input): Output;
  resolve(input: Input, judge: Judge, options?: ResolveOptions): Promise<Resolution>;
}

export class EntityresError extends Error {
  override name = "EntityresError";
}

export class JevError extends Error {
  override name = "JevError";
  status?: number;
  constructor(message: string, status?: number) {
    super(message);
    this.status = status;
  }
}

interface Exports {
  memory: WebAssembly.Memory;
  er_alloc(len: number): number;
  er_free(ptr: number, len: number): void;
  er_run(ptr: number, len: number): bigint;
}

export async function load(wasm: BufferSource | WebAssembly.Module): Promise<Entityres> {
  const instance =
    wasm instanceof WebAssembly.Module
      ? await WebAssembly.instantiate(wasm, {})
      : (await WebAssembly.instantiate(wasm, {})).instance;
  const ex = instance.exports as unknown as Exports;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  function runJson(text: string): string {
    const bytes = encoder.encode(text);
    const ptr = ex.er_alloc(bytes.length) >>> 0;
    new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes);
    const packed = BigInt.asUintN(64, ex.er_run(ptr, bytes.length));
    ex.er_free(ptr, bytes.length);
    const outPtr = Number(packed >> 32n);
    const outLen = Number(packed & 0xffffffffn);
    const out = decoder.decode(new Uint8Array(ex.memory.buffer, outPtr, outLen).slice());
    ex.er_free(outPtr, outLen);
    return out;
  }

  function run(input: Input): Output {
    const out = JSON.parse(runJson(JSON.stringify(input)));
    if (typeof out.error === "string") throw new EntityresError(out.error);
    return out as Output;
  }

  async function resolve(input: Input, judge: Judge, options: ResolveOptions = {}): Promise<Resolution> {
    const maxRounds = options.maxRounds ?? 10;
    const concurrency = options.concurrency ?? 8;
    const answers = [...(input.answers ?? [])];
    for (let round = 0; ; round++) {
      const output = run({ ...input, answers });
      if (output.pending.length === 0 || round === maxRounds) return { output, answers };
      const fresh = await pool(output.pending, concurrency, async (q: Question) => ({
        id: q.id,
        answers: await judge(q.state, q.questions),
      }));
      answers.push(...fresh);
    }
  }

  return { run, resolve };
}

async function pool<T, R>(items: T[], size: number, fn: (item: T) => Promise<R>): Promise<R[]> {
  const out: R[] = new Array(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      out[i] = await fn(items[i]);
    }
  };
  await Promise.all(Array.from({ length: Math.min(size, items.length) }, worker));
  return out;
}

export interface JevOptions {
  apiKey?: string;
  model?: string;
  baseUrl?: string;
  fetch?: typeof fetch;
  maxRetries?: number;
  timeoutMs?: number;
}

export function jev(options: JevOptions = {}): Judge {
  const env = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process?.env ?? {};
  const apiKey = options.apiKey ?? env.TYPESAFE_API_KEY ?? "";
  const baseUrl = (options.baseUrl ?? env.TYPESAFE_BASE_URL ?? "https://api.typesafe.ai").replace(/\/$/, "");
  const model = options.model ?? "jev-latest";
  const doFetch = options.fetch ?? fetch;
  const maxRetries = options.maxRetries ?? 3;
  const timeoutMs = options.timeoutMs ?? 15_000;
  if (!apiKey) throw new JevError("TYPESAFE_API_KEY is not set");

  return async (state, questions) => {
    const body = JSON.stringify({ state, model, questions });
    for (let attempt = 0; ; attempt++) {
      let res: Response;
      try {
        res = await doFetch(`${baseUrl}/v1/systemone`, {
          method: "POST",
          headers: { "content-type": "application/json", authorization: `Bearer ${apiKey}` },
          body,
          signal: AbortSignal.timeout(timeoutMs),
        });
      } catch (err) {
        if (attempt < maxRetries) {
          await sleep(backoff(attempt));
          continue;
        }
        throw new JevError(`jev request failed: ${(err as Error).message}`);
      }
      if ((res.status === 429 || res.status >= 500) && attempt < maxRetries) {
        await sleep(backoff(attempt));
        continue;
      }
      if (!res.ok) throw new JevError(`jev ${res.status}: ${(await res.text()).slice(0, 300)}`, res.status);
      const json = (await res.json()) as { answers: Json };
      return json.answers;
    }
  };
}

function backoff(attempt: number): number {
  return Math.min(4000, 250 * 2 ** attempt) + Math.random() * 100;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
