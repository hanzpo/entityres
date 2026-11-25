import json
from pathlib import Path

import pytest

from entityres import EntityresError, Json, resolve, run

ROOT = Path(__file__).parents[3] / "fixtures"
FIXTURES = sorted(p.name for p in ROOT.iterdir() if (p / "input.json").exists())


def load(name: str, file: str) -> Json:
    return json.loads((ROOT / name / file).read_text())


class FakeJudge:
    def __init__(self, spec: dict[str, Json]) -> None:
        self.spec = spec
        self.calls = 0

    def __call__(self, state: Json, questions: dict[str, Json]) -> Json:
        self.calls += 1
        if "same" in questions:
            p = self.spec["pairs"].get(f"{state['a']['id']}|{state['b']['id']}", 0.0)
            return {"same": {"type": "noul", "noul": p}}
        want = self.spec["values"].get(state["id"], {}).get(state["field"])
        criteria: dict[str, str] = questions["pick"]["criteria"]
        choice = next((k for k, v in criteria.items() if v == want), "0")
        return {"pick": {"type": "choice", "choice": choice}}


@pytest.mark.parametrize("name", FIXTURES)
def test_fixture(name: str) -> None:
    expected = load(name, "expected.json")
    judge = FakeJudge(load(name, "judge.json"))
    output = resolve(load(name, "input.json"), judge).output
    got = sorted(",".join(e["records"]) for e in output["entities"])
    want = sorted(",".join(e["records"]) for e in expected["entities"])
    assert got == want
    by_records = {tuple(e["records"]): e for e in output["entities"]}
    for e in expected["entities"]:
        for k, v in e.get("fields", {}).items():
            assert by_records[tuple(e["records"])]["fields"][k] == v
    if "max_calls" in expected:
        assert judge.calls <= expected["max_calls"]
    assert output["conflicts"] == []


def test_bad_input_raises() -> None:
    with pytest.raises(EntityresError, match="records"):
        run({})
