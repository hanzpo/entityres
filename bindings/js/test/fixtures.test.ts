import assert from "node:assert/strict";
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { test } from "node:test";
import { EntityresError, loadNode, type Input, type Json, type Judge } from "../src/node.ts";

const root = new URL("../../../fixtures/", import.meta.url);
const read = (dir: string, file: string) => JSON.parse(readFileSync(new URL(`${dir}/${file}`, root), "utf8"));

interface FakeJudge {
  pairs: Record<string, number>;
  values: Record<string, Record<string, string>>;
}

function fake(spec: FakeJudge): { judge: Judge; calls: () => number } {
  let calls = 0;
  const judge: Judge = async (state, questions) => {
    calls++;
    const s = state as Record<string, Record<string, string>> & { id: string; field: string };
    if ("same" in questions) {
      return { same: { type: "noul", noul: spec.pairs[`${s.a.id}|${s.b.id}`] ?? 0 } };
    }
    const want = spec.values[s.id]?.[s.field];
    const criteria = (questions.pick as { criteria: Record<string, string> }).criteria;
    const choice = Object.entries(criteria).find(([, v]) => v === want)?.[0] ?? "0";
    return { pick: { type: "choice", choice } };
  };
  return { judge, calls: () => calls };
}

const er = await loadNode();

for (const dir of readdirSync(root).filter((d) => existsSync(new URL(`${d}/input.json`, root))).sort()) {
  test(`fixture ${dir}`, async () => {
    const input = read(dir, "input.json") as Input;
    const expected = read(dir, "expected.json") as {
      entities: { records: string[]; fields?: Record<string, Json> }[];
      max_calls?: number;
    };
    const { judge, calls } = fake(read(dir, "judge.json"));
    const { output } = await er.resolve(input, judge);
    const got = output.entities.map((e) => e.records.join(",")).sort();
    const want = expected.entities.map((e) => e.records.join(",")).sort();
    assert.deepEqual(got, want);
    for (const e of expected.entities) {
      const entity = output.entities.find((x) => x.records.join(",") === e.records.join(","))!;
      for (const [k, v] of Object.entries(e.fields ?? {})) assert.deepEqual(entity.fields[k], v, `${entity.id} ${k}`);
    }
    if (expected.max_calls !== undefined) assert.ok(calls() <= expected.max_calls, `${calls()} calls`);
    assert.deepEqual(output.conflicts, []);
  });
}

test("bad input throws", () => {
  assert.throws(() => er.run({} as Input), EntityresError);
});
