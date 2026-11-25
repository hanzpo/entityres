import json
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from typing import Any

from entityres._core import run_json

type Json = Any
type Judge = Callable[[Json, dict[str, Json]], Json]


class EntityresError(Exception):
    pass


@dataclass
class Resolution:
    output: dict[str, Json]
    answers: list[dict[str, Json]] = field(default_factory=list[dict[str, Json]])


def run(input: dict[str, Json]) -> dict[str, Json]:
    out: dict[str, Json] = json.loads(run_json(json.dumps(input)))
    if isinstance(out.get("error"), str):
        raise EntityresError(out["error"])
    return out


def resolve(input: dict[str, Json], judge: Judge, max_rounds: int = 10, workers: int = 8) -> Resolution:
    answers: list[dict[str, Json]] = list(input.get("answers", []))

    def ask(q: dict[str, Json]) -> dict[str, Json]:
        return {"id": q["id"], "answers": judge(q["state"], q["questions"])}

    with ThreadPoolExecutor(max_workers=workers) as executor:
        for round in range(max_rounds + 1):
            output = run({**input, "answers": answers})
            pending: list[dict[str, Json]] = output["pending"]
            if not pending or round == max_rounds:
                return Resolution(output, answers)
            answers.extend(executor.map(ask, pending))
    raise AssertionError("unreachable")
