import json
import os
import random
import time
import urllib.error
import urllib.request

from entityres._api import Json, Judge


class JevError(Exception):
    def __init__(self, message: str, status: int | None = None) -> None:
        super().__init__(message)
        self.status = status


def _backoff(attempt: int) -> float:
    return min(4.0, 0.25 * 2**attempt) + random.random() * 0.1


def jev(
    api_key: str | None = None,
    model: str = "jev-latest",
    base_url: str | None = None,
    max_retries: int = 3,
    timeout: float = 15.0,
) -> Judge:
    key = api_key or os.environ.get("TYPESAFE_API_KEY", "")
    if not key:
        raise JevError("TYPESAFE_API_KEY is not set")
    url = (base_url or os.environ.get("TYPESAFE_BASE_URL") or "https://api.typesafe.ai").rstrip("/")
    url = f"{url}/v1/systemone"

    def judge(state: Json, questions: dict[str, Json]) -> Json:
        body = json.dumps({"state": state, "model": model, "questions": questions}).encode()
        headers = {"content-type": "application/json", "authorization": f"Bearer {key}"}
        for attempt in range(max_retries + 1):
            request = urllib.request.Request(url, data=body, headers=headers, method="POST")
            try:
                with urllib.request.urlopen(request, timeout=timeout) as response:
                    return json.loads(response.read())["answers"]
            except urllib.error.HTTPError as err:
                retry = err.code == 429 or err.code >= 500
                if not retry or attempt == max_retries:
                    raise JevError(
                        f"jev {err.code}: {err.read()[:300].decode(errors='replace')}", err.code
                    ) from err
            except (urllib.error.URLError, TimeoutError) as err:
                if attempt == max_retries:
                    raise JevError(f"jev request failed: {err}") from err
            time.sleep(_backoff(attempt))
        raise AssertionError("unreachable")

    return judge
