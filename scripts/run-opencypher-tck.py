#!/usr/bin/env python3
"""Run the pinned openCypher TCK against a Protocol v1 server.

Every scenario of every feature (Scenario Outlines expanded per Examples row)
runs on an isolated graph: the tenant is purged, the scenario's graph and
setup statements are executed, its query runs, and its result table, error
expectation and side effects are compared with the upstream expectations.

Each scenario ends `pass` or `fail` with a category (see `categorize`). The
outcome of every scenario is compared with the committed ledger
(`--ledger`): any difference fails the run, so a regression cannot land and
an improvement must update the ledger (`--write-ledger`). The profile file
explains every failure category, as an `exclusion` (a feature outside the
supported Cypher profile, advertised as a capability limitation) or a
`defect` (a wrong result of the embedded Grafeo engine); a failure whose
category it does not explain fails the run.

With `--reference` (CI passes it), every defect failure is rerun on plain
Grafeo (`crates/rushdb-language/examples/grafeo_reference.rs`: Grafeo's own
store, no LMPG, no RushDB adapter). A scenario that passes there fails
because of RushDB: it becomes `rushdb-adapter`, which the profile never
explains, unless its rule is `confirmed` upstream with a reason.

Comparison rules (documented in docs/QUERY_LANGUAGE_CONFORMANCE.md):
- an expected error passes when the statement is refused with any error
  (the TCK's error type is not compared);
- values compare by TCK semantics: integers and floats are distinct, nodes
  compare by labels and properties, relationships by type and properties,
  lists in order unless the scenario ignores list element order.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

UPSTREAM_COMMIT = "59edf2e1c17b845bf97c334ed06b2eb780950c13"
CONFORMANCE = Path(__file__).resolve().parent.parent / "crates/rushdb-server/conformance"
DEFAULT_LEDGER = CONFORMANCE / "opencypher-tck-v2024.2-ledger.json"
DEFAULT_PROFILE = CONFORMANCE / "opencypher-tck-v2024.2-profile.json"


# ---------------------------------------------------------------- Gherkin


@dataclass
class Step:
    keyword: str
    text: str
    doc: str | None = None
    table: list[list[str]] | None = None


@dataclass
class Scenario:
    feature: str
    name: str
    steps: list[Step] = field(default_factory=list)

    @property
    def key(self) -> str:
        return f"{self.feature}::{self.name}"


def table_row(line: str) -> list[str]:
    cells, current, escaped = [], "", False
    body = line.strip()[1:]
    for char in body:
        if escaped:
            # Gherkin cell escapes: \| \\ \n; any other pair stays as is.
            current += {"|": "|", "\\": "\\", "n": "\n"}.get(char, "\\" + char)
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == "|":
            cells.append(current.strip())
            current = ""
        else:
            current += char
    return cells


def parse_feature(path: Path, relative: str) -> list[Scenario]:
    lines = path.read_text(encoding="utf-8").splitlines()
    scenarios: list[Scenario] = []
    i = 0
    current: Scenario | None = None
    outline: tuple[Scenario, list[list[str]]] | None = None
    in_examples = False

    def flush_outline() -> None:
        nonlocal outline
        if outline is None:
            return
        template, rows = outline
        if rows:
            header, *values = rows
            for index, row in enumerate(values, start=1):
                binding = dict(zip(header, row))

                def bind(text: str | None) -> str | None:
                    if text is None:
                        return None
                    return re.sub(r"<([^<>]+)>", lambda m: binding.get(m.group(1), m.group(0)), text)

                steps = [
                    Step(
                        s.keyword,
                        bind(s.text) or "",
                        bind(s.doc),
                        [[bind(c) or "" for c in r] for r in s.table] if s.table else None,
                    )
                    for s in template.steps
                ]
                scenarios.append(Scenario(template.feature, f"{template.name} #{index}", steps))
        outline = None

    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if stripped.startswith("Scenario Outline:") or stripped.startswith("Scenario:"):
            flush_outline()
            if current is not None:
                scenarios.append(current)
                current = None
            name = stripped.split(":", 1)[1].strip()
            scenario = Scenario(relative, name)
            if stripped.startswith("Scenario Outline:"):
                outline = (scenario, [])
            else:
                current = scenario
            in_examples = False
        elif stripped.startswith("Examples:"):
            in_examples = True
        elif in_examples and stripped.startswith("|") and outline is not None:
            outline[1].append(table_row(stripped))
        elif re.match(r"^(Given|When|Then|And|But)\b", stripped):
            target = current if current is not None else (outline[0] if outline else None)
            keyword, _, text = stripped.partition(" ")
            step = Step(keyword, text.rstrip(":").strip())
            # Doc string or table follows.
            j = i + 1
            if j < len(lines) and lines[j].strip().startswith('"""'):
                indent = len(lines[j]) - len(lines[j].lstrip())
                body = []
                j += 1
                while not lines[j].strip().startswith('"""'):
                    body.append(lines[j][indent:] if len(lines[j]) >= indent else lines[j].strip())
                    j += 1
                step.doc = "\n".join(body).strip()
                i = j
            elif j < len(lines) and lines[j].strip().startswith("|"):
                rows = []
                while j < len(lines) and lines[j].strip().startswith("|"):
                    rows.append(table_row(lines[j]))
                    j += 1
                step.table = rows
                i = j - 1
            if target is not None:
                target.steps.append(step)
        i += 1
    flush_outline()
    if current is not None:
        scenarios.append(current)
    return scenarios


# ------------------------------------------------------------ TCK values


class ValueParser:
    """Recursive-descent parser for TCK result cells and parameter values."""

    def __init__(self, text: str):
        self.text = text
        self.pos = 0

    def parse(self):
        value = self.value()
        self.ws()
        if self.pos != len(self.text):
            raise ValueError(f"trailing input in TCK value {self.text!r}")
        return value

    def ws(self) -> None:
        while self.pos < len(self.text) and self.text[self.pos].isspace():
            self.pos += 1

    def peek(self) -> str:
        self.ws()
        return self.text[self.pos] if self.pos < len(self.text) else ""

    def expect(self, char: str) -> None:
        self.ws()
        if not self.text.startswith(char, self.pos):
            raise ValueError(f"expected {char!r} at {self.pos} in {self.text!r}")
        self.pos += len(char)

    def value(self):
        char = self.peek()
        if char == "'":
            return self.string()
        if char == "[":
            return self.list_or_relationship()
        if char == "{":
            return ("map", self.map_body())
        if char == "(":
            return self.node()
        if char == "<":
            return self.path()
        return self.scalar()

    def string(self) -> str:
        self.expect("'")
        out = []
        while self.text[self.pos] != "'":
            if self.text[self.pos] == "\\":
                self.pos += 1
                out.append({"n": "\n", "t": "\t"}.get(self.text[self.pos], self.text[self.pos]))
            else:
                out.append(self.text[self.pos])
            self.pos += 1
        self.pos += 1
        return out_str("".join(out))

    def scalar(self):
        self.ws()
        match = re.compile(r"-?(?:\d+\.\d*(?:[eE][-+]?\d+)?|\.\d+(?:[eE][-+]?\d+)?|\d+[eE][-+]?\d+|0x[0-9a-fA-F]+|0o[0-7]+|\d+|Infinity|NaN)|-Infinity|true|false|null").match(self.text, self.pos)
        if not match:
            raise ValueError(f"cannot parse TCK value at {self.pos} in {self.text!r}")
        self.pos = match.end()
        token = match.group(0)
        if token == "null":
            return None
        if token in {"true", "false"}:
            return token == "true"
        if token in {"Infinity", "-Infinity", "NaN"}:
            return ("float", float(token.replace("Infinity", "inf")))
        if token.lstrip("-").startswith("0x"):
            return int(token, 16)
        if token.lstrip("-").startswith("0o"):
            return int(token.replace("0o", ""), 8)
        if re.fullmatch(r"-?\d+", token):
            return int(token)
        return ("float", float(token))

    def map_body(self) -> tuple:
        self.expect("{")
        entries = []
        if self.peek() != "}":
            while True:
                self.ws()
                key = re.compile(r"`[^`]*`|[A-Za-z_][A-Za-z0-9_]*").match(self.text, self.pos)
                if not key:
                    raise ValueError(f"bad map key in {self.text!r}")
                self.pos = key.end()
                self.expect(":")
                entries.append((key.group(0).strip("`"), self.value()))
                if self.peek() == ",":
                    self.pos += 1
                    continue
                break
        self.expect("}")
        return tuple(sorted(entries, key=lambda e: e[0]))

    def labels(self) -> tuple:
        labels = []
        while self.peek() == ":":
            self.pos += 1
            self.ws()
            match = re.compile(r"`[^`]*`|[A-Za-z_][A-Za-z0-9_]*").match(self.text, self.pos)
            self.pos = match.end()
            labels.append(match.group(0).strip("`"))
        return tuple(sorted(labels))

    def node(self):
        self.expect("(")
        labels = self.labels()
        props = self.map_body() if self.peek() == "{" else ()
        self.expect(")")
        return ("node", labels, props)

    def list_or_relationship(self):
        self.expect("[")
        if self.peek() == ":":
            labels = self.labels()
            props = self.map_body() if self.peek() == "{" else ()
            self.expect("]")
            return ("rel", labels[0] if labels else "", props)
        items = []
        if self.peek() != "]":
            while True:
                items.append(self.value())
                if self.peek() == ",":
                    self.pos += 1
                    continue
                break
        self.expect("]")
        return ("list", tuple(items))

    def path(self):
        self.expect("<")
        parts = [self.node()]
        while self.peek() in {"-", "<"}:
            backwards = False
            if self.text.startswith("<-", self.pos):
                backwards = True
                self.pos += 2
            else:
                self.expect("-")
            rel = self.list_or_relationship()
            if self.text.startswith("->", self.pos):
                self.pos += 2
            else:
                self.expect("-")
            parts.append((rel, backwards))
            parts.append(self.node())
        self.expect(">")
        return ("path", tuple(parts))


class out_str(str):
    """A parsed TCK string (distinct from map keys and identifiers)."""


def tck_value(text: str):
    return ValueParser(text).parse()


# The engine emits protocol record shapes (`__id`, `__label`, `__type`,
# `__source_id`, `__target_id`); the Grafeo reference emits the harness shapes
# (`_id`, `__labels`, `type`, `source`, `target`). Normalise both to the TCK
# value model, where entity ids are not part of a result.
NODE_ID_KEYS = {"_id", "__id"}
NODE_LABEL_KEYS = {"__label", "__labels"}
REL_ENTITY_KEYS = {
    "_id",
    "__id",
    "type",
    "__type",
    "source",
    "__source_id",
    "target",
    "__target_id",
}


def entity_id(entity: dict):
    return entity.get("__id", entity.get("_id"))


def entity_labels(entity: dict) -> list:
    if "__labels" in entity:
        return list(entity["__labels"])
    label = entity.get("__label")
    return [label] if label else []


def is_node(entity: dict) -> bool:
    return entity_id(entity) is not None and any(key in entity for key in NODE_LABEL_KEYS)


def is_relationship(entity: dict) -> bool:
    return (
        entity_id(entity) is not None
        and ("type" in entity or "__type" in entity)
        and ("source" in entity or "__source_id" in entity)
        and ("target" in entity or "__target_id" in entity)
    )


def from_json(value):
    """A Protocol v1 JSON value in the TCK value model."""
    if value is None or isinstance(value, bool):
        return value
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return ("float", value)
    if isinstance(value, str):
        return out_str(value)
    if isinstance(value, list):
        return ("list", tuple(from_json(v) for v in value))
    if isinstance(value, dict):
        if is_node(value):
            props = tuple(
                sorted(
                    (k, from_json(v))
                    for k, v in value.items()
                    if k not in NODE_ID_KEYS | NODE_LABEL_KEYS
                )
            )
            labels = tuple(sorted(label for label in entity_labels(value) if label))
            return ("node", labels, props)
        if is_relationship(value):
            props = tuple(
                sorted((k, from_json(v)) for k, v in value.items() if k not in REL_ENTITY_KEYS)
            )
            return ("rel", value.get("type", value.get("__type")), props)
        if set(value.keys()) == {"nodes", "edges"} and isinstance(value["nodes"], list):
            nodes = [from_json(n) for n in value["nodes"]]
            edges = [from_json(e) for e in value["edges"]]
            parts = [nodes[0]] if nodes else []
            for index, edge in enumerate(edges):
                backwards = False
                raw = value["edges"][index]
                raw_nodes = value["nodes"]
                if isinstance(raw, dict) and isinstance(raw_nodes[index], dict):
                    source = raw.get("source", raw.get("__source_id"))
                    backwards = source != entity_id(raw_nodes[index])
                parts.append((edge, backwards))
                parts.append(nodes[index + 1] if index + 1 < len(nodes) else None)
            return ("path", tuple(parts))
        return ("map", tuple(sorted((k, from_json(v)) for k, v in value.items())))
    raise ValueError(f"unsupported JSON value {value!r}")


def unordered_lists(value):
    if isinstance(value, tuple) and value and value[0] == "list":
        return ("list", tuple(sorted((unordered_lists(v) for v in value[1]), key=repr)))
    if isinstance(value, tuple) and value and value[0] == "map":
        return ("map", tuple((k, unordered_lists(v)) for k, v in value[1]))
    return value


def same(expected, actual) -> bool:
    if isinstance(expected, tuple) and isinstance(actual, tuple):
        if expected[:1] == ("float",) and actual[:1] == ("float",):
            a, b = expected[1], actual[1]
            if math.isnan(a) and math.isnan(b):
                return True
            return a == b or abs(a - b) <= 1e-9 * max(1.0, abs(a))
        if len(expected) != len(actual):
            return False
        return all(same(e, a) for e, a in zip(expected, actual))
    if isinstance(expected, bool) or isinstance(actual, bool):
        return type(expected) is type(actual) and expected == actual
    return expected == actual


# -------------------------------------------------------------- protocol


class ProtocolError(Exception):
    pass


class Server:
    def __init__(self, endpoint: str, token: str):
        self.endpoint = endpoint
        self.token = token

    def call(self, operation: dict) -> dict:
        payload = json.dumps({"protocolVersion": 1, "context": {}, "operation": operation}).encode()
        req = urllib.request.Request(
            self.endpoint,
            data=payload,
            headers={"Authorization": f"Bearer {self.token}", "Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as response:
                body = json.load(response)
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")
            try:
                parsed = json.loads(detail)
                message = parsed.get("error", {}).get("message", detail)
            except json.JSONDecodeError:
                message = detail
            raise ProtocolError(message) from error
        if body.get("error"):
            raise ProtocolError(body["error"].get("message", str(body["error"])))
        return body["response"]

    def statement(self, statement: str, parameters: dict | None, write: bool) -> list[dict]:
        if write:
            operation = {"kind": "mutate", "mutation": {"kind": "statement", "language": "cypher", "statement": statement}}
            if parameters:
                operation["mutation"]["parameters"] = parameters
        else:
            operation = {"kind": "query", "query": {"kind": "statement", "language": "cypher", "statement": statement}}
            if parameters:
                operation["query"]["parameters"] = parameters
        response = self.call(operation)
        return response.get("rows", [])

    def run(self, statement: str, parameters: dict | None = None) -> list[dict]:
        """Read path first; a statement that changes the graph goes through
        the write-authorized mutation operation."""
        return self.statement(statement, parameters, bool(WRITE.search(strip_strings(statement))))

    def purge(self) -> None:
        self.call({"kind": "purgeTenant"})

    def snapshot(self) -> dict:
        nodes = self.statement("MATCH (n) RETURN n", None, False)
        rels = self.statement("MATCH ()-[r]->() RETURN r", None, False)
        state = {"nodes": set(), "relationships": set(), "labels": set(), "properties": set()}
        for row in nodes:
            node = row["n"]
            node_id = entity_id(node)
            state["nodes"].add(node_id)
            state["labels"].update(label for label in entity_labels(node) if label)
            for key, value in node.items():
                if key not in NODE_ID_KEYS | NODE_LABEL_KEYS:
                    state["properties"].add((node_id, key, json.dumps(value, sort_keys=True)))
        for row in rels:
            rel = row["r"]
            rel_id = entity_id(rel)
            state["relationships"].add(rel_id)
            for key, value in rel.items():
                if key not in REL_ENTITY_KEYS:
                    state["properties"].add((rel_id, key, json.dumps(value, sort_keys=True)))
        return state


class ReferenceServer:
    """Plain Grafeo (`crates/rushdb-language/examples/grafeo_reference.rs`)
    behind the same interface as `Server`, to attribute failures."""

    TIMEOUT = 20.0

    def __init__(self, binary: str):
        self.binary = binary
        self.process = None
        self.start()

    def start(self) -> None:
        if self.process is not None:
            self.process.kill()
        self.process = subprocess.Popen(
            [self.binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1
        )

    def command(self, command: dict) -> dict:
        import select

        self.process.stdin.write(json.dumps(command) + "\n")
        self.process.stdin.flush()
        ready, _, _ = select.select([self.process.stdout], [], [], self.TIMEOUT)
        if not ready:
            self.start()
            raise ProtocolError("reference statement did not finish")
        return json.loads(self.process.stdout.readline())

    def purge(self) -> None:
        self.command({"op": "reset"})

    def run(self, statement: str, parameters: dict | None = None) -> list[dict]:
        response = self.command({"op": "run", "statement": statement, "parameters": parameters})
        if "error" in response:
            raise ProtocolError(response["error"])
        return response["rows"]

    def snapshot(self) -> dict:
        response = self.command({"op": "snapshot"})
        state = {"nodes": set(), "relationships": set(), "labels": set(), "properties": set()}
        for node in response["nodes"]:
            state["nodes"].add(node["_id"])
            state["labels"].update(node.get("__labels", []))
            for key, value in node.items():
                if key not in {"_id", "__labels"}:
                    state["properties"].add((node["_id"], key, json.dumps(value, sort_keys=True)))
        for rel in response["relationships"]:
            state["relationships"].add(rel["_id"])
            for key, value in rel.items():
                if key not in {"_id", "type", "source", "target"}:
                    state["properties"].add((rel["_id"], key, json.dumps(value, sort_keys=True)))
        return state


WRITE = re.compile(r"\b(CREATE|MERGE|SET|DELETE|REMOVE)\b", re.IGNORECASE)


def strip_strings(statement: str) -> str:
    return re.sub(r"'(?:\\.|[^'\\])*'|\"(?:\\.|[^\"\\])*\"", "''", statement)


# -------------------------------------------------------------- scenarios


class Failure(Exception):
    def __init__(self, category: str, detail: str):
        super().__init__(detail)
        self.category = category
        self.detail = detail


def side_effects(before: dict, after: dict) -> dict:
    effects = {}
    for kind in ("nodes", "relationships", "labels", "properties"):
        added = len(after[kind] - before[kind])
        removed = len(before[kind] - after[kind])
        if added:
            effects[f"+{kind}"] = added
        if removed:
            effects[f"-{kind}"] = removed
    return effects


def run_scenario(server: Server, scenario: Scenario, graphs: Path) -> None:
    parameters: dict | None = None
    result: list[dict] | None = None
    error: str | None = None
    before = None
    for step in scenario.steps:
        text = step.text
        if text in {"an empty graph", "any graph"}:
            server.purge()
        elif m := re.fullmatch(r"the (\S+) graph", text):
            server.purge()
            script = (graphs / m.group(1) / f"{m.group(1)}.cypher").read_text(encoding="utf-8")
            try:
                server.run(script)
            except ProtocolError as exc:
                raise Failure("setup-refused", str(exc))
        elif text == "having executed":
            try:
                server.run(step.doc or "")
            except ProtocolError as exc:
                raise Failure("setup-refused", str(exc))
        elif text == "parameters are":
            parameters = {}
            for row in step.table or []:
                parameters[row[0]] = to_json(tck_value(row[1]))
        elif text.startswith("there exists a procedure"):
            raise Failure("procedures", "user-defined procedures are not exposed")
        elif text in {"executing query", "executing control query"}:
            if text == "executing query":
                before = server.snapshot()
            try:
                result = server.run(step.doc or "", parameters)
                error = None
            except ProtocolError as exc:
                error = str(exc)
                result = None
        elif text == "the result should be empty":
            if error is not None:
                raise Failure("refused", error)
            if result:
                raise Failure("result-mismatch", f"expected empty, got {result!r}"[:400])
        elif text.startswith("the result should be"):
            if error is not None:
                raise Failure("refused", error)
            expected_table = step.table or []
            compare(expected_table, result or [], ordered="in order" in text, lists_unordered="ignoring element order" in text)
        elif m := re.fullmatch(r"a (\w+) should be raised at (\w+ ?\w*)", text.split(":")[0].strip()):
            if error is None:
                raise Failure("error-not-raised", f"expected {m.group(1)}, got {result!r}"[:400])
        elif text == "no side effects":
            effects = side_effects(before, server.snapshot()) if before is not None else {}
            if effects:
                raise Failure("side-effects-mismatch", f"expected none, got {effects}")
        elif text == "the side effects should be":
            expected = {row[0]: int(row[1]) for row in step.table or []}
            effects = side_effects(before, server.snapshot()) if before is not None else {}
            if effects != expected:
                raise Failure("side-effects-mismatch", f"expected {expected}, got {effects}")
        else:
            raise Failure("runner-step", f"unhandled step {step.keyword} {text}")


def to_json(value):
    if isinstance(value, tuple):
        if value[0] == "float":
            return value[1]
        if value[0] == "list":
            return [to_json(v) for v in value[1]]
        if value[0] == "map":
            return {k: to_json(v) for k, v in value[1]}
        raise ValueError(f"cannot pass {value!r} as a parameter")
    return value


def compare(expected_table: list[list[str]], actual: list[dict], ordered: bool, lists_unordered: bool) -> None:
    header, *rows = expected_table
    try:
        expected = [tuple(tck_value(cell) for cell in row) for row in rows]
    except ValueError as exc:
        raise Failure("runner-value", str(exc))
    try:
        got = [tuple(from_json(row.get(column, MISSING)) for column in header) for row in actual]
    except ValueError as exc:
        raise Failure("result-value", str(exc))
    if any(MISSING_MARK in repr(row) for row in got):
        raise Failure("result-columns", f"columns {header} not all in {list(actual[0].keys()) if actual else []}")
    if lists_unordered:
        expected = [tuple(unordered_lists(v) for v in row) for row in expected]
        got = [tuple(unordered_lists(v) for v in row) for row in got]
    if not ordered:
        expected = sorted(expected, key=repr)
        got = sorted(got, key=repr)
    if len(expected) != len(got) or not all(same(e, a) for e, a in zip(expected, got)):
        raise Failure("result-mismatch", f"expected {expected!r} got {got!r}"[:600])


MISSING_MARK = "__tck_missing_column__"
MISSING = MISSING_MARK


# ------------------------------------------------------------------ main


def categorize(profile: dict, key: str, failure: Failure) -> tuple[str, dict | None]:
    """The first profile rule matching the failure names its category: a
    rule may require the runner's raw category (`when`), a scenario key
    pattern (`key`) and a detail pattern (`pattern`). Unmatched failures
    keep the raw category, which the profile does not explain."""
    for rule in profile.get("rules", []):
        if rule.get("when") and rule["when"] != failure.category:
            continue
        if rule.get("key") and not re.search(rule["key"], key):
            continue
        if rule.get("pattern") and not re.search(rule["pattern"], failure.detail, re.IGNORECASE | re.DOTALL):
            continue
        return rule["category"], rule
    return failure.category, None


def check_profile(profile: dict) -> list[str]:
    """Every category is an `exclusion` (a declared feature gap, advertised
    by the capability limitations it names) or a `defect` (a wrong result of
    the embedded engine); every rule names a category."""
    problems = []
    categories = profile.get("categories", {})
    for name, category in categories.items():
        if category.get("kind") not in {"exclusion", "defect"}:
            problems.append(f"{name}: kind must be exclusion or defect")
        if category.get("kind") == "exclusion" and not category.get("limitations"):
            problems.append(f"{name}: an exclusion names the capability limitations advertising it")
    for rule in profile.get("rules", []):
        if rule.get("category") not in categories:
            problems.append(f"rule {rule}: unknown category")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--endpoint", required=True)
    parser.add_argument("--token", default="local-token")
    parser.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    parser.add_argument("--profile", type=Path, default=DEFAULT_PROFILE)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--write-ledger", action="store_true", help="record this run as the ledger")
    parser.add_argument("--only", help="run scenarios whose key matches this regex")
    parser.add_argument("--trace", type=Path, help="append each scenario key before running it")
    parser.add_argument(
        "--reference",
        help="plain Grafeo reference binary: a failure that passes there is RushDB's (category rushdb-adapter)",
    )
    args = parser.parse_args()

    source = args.source.resolve()
    commit = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if commit != UPSTREAM_COMMIT:
        print(f"expected upstream commit {UPSTREAM_COMMIT}, found {commit}", file=sys.stderr)
        return 2
    profile = json.loads(args.profile.read_text(encoding="utf-8"))
    if problems := check_profile(profile):
        print("invalid profile:\n  " + "\n  ".join(problems), file=sys.stderr)
        return 2
    features = sorted((source / "tck/features").rglob("*.feature"))
    scenarios = [
        scenario
        for path in features
        for scenario in parse_feature(path, str(path.relative_to(source)))
    ]
    if args.only:
        scenarios = [s for s in scenarios if re.search(args.only, s.key)]
    server = Server(args.endpoint, args.token)
    reference = ReferenceServer(args.reference) if args.reference else None
    outcomes: dict[str, dict] = {}
    for scenario in scenarios:
        if args.trace:
            with args.trace.open("a", encoding="utf-8") as trace:
                trace.write(scenario.key + "\n")
        try:
            run_scenario(server, scenario, source / "tck/graphs")
            outcomes[scenario.key] = {"outcome": "pass"}
        except Failure as failure:
            category, rule = categorize(profile, scenario.key, failure)
            defect = profile["categories"].get(category, {}).get("kind") == "defect"
            if reference is not None and defect and not (rule or {}).get("confirmed"):
                try:
                    run_scenario(reference, scenario, source / "tck/graphs")
                    # Plain Grafeo passes: the failure is RushDB's.
                    category = "rushdb-adapter"
                except (Failure, ProtocolError):
                    pass
            outcomes[scenario.key] = {
                "outcome": "fail",
                "category": category,
                "detail": failure.detail[:600],
            }
        except ProtocolError as exc:
            outcomes[scenario.key] = {"outcome": "fail", "category": "protocol", "detail": str(exc)[:600]}

    passed = sum(1 for o in outcomes.values() if o["outcome"] == "pass")
    by_category: dict[str, int] = {}
    for outcome in outcomes.values():
        if outcome["outcome"] == "fail":
            by_category[outcome["category"]] = by_category.get(outcome["category"], 0) + 1
    explained = profile.get("categories", {})
    unexplained = {c: n for c, n in by_category.items() if c not in explained}
    kinds = {"exclusion": 0, "defect": 0}
    for category, count in by_category.items():
        kind = explained.get(category, {}).get("kind")
        if kind in kinds:
            kinds[kind] += count
    report = {
        "schemaVersion": 2,
        "referenceAttribution": reference is not None,
        "repository": "https://github.com/opencypher/openCypher",
        "commit": commit,
        "scenarios": len(outcomes),
        "passed": passed,
        "failedByCategory": dict(sorted(by_category.items())),
        "excluded": kinds["exclusion"],
        "knownDefects": kinds["defect"],
        "unexplainedCategories": unexplained,
        "outcomes": outcomes,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        f"{passed}/{len(outcomes)} scenarios pass; {kinds['exclusion']} excluded by the profile,"
        f" {kinds['defect']} known defects"
    )
    for category, count in sorted(by_category.items(), key=lambda kv: -kv[1]):
        kind = explained.get(category, {}).get("kind", "UNEXPLAINED")
        print(f"  {count:5} {category} ({kind})")

    ledger_view = {key: (o["outcome"], o.get("category")) for key, o in outcomes.items()}
    if args.write_ledger:
        args.ledger.write_text(
            json.dumps(
                {
                    "schemaVersion": 1,
                    "commit": commit,
                    "outcomes": {k: ({"outcome": v[0]} if v[0] == "pass" else {"outcome": v[0], "category": v[1]}) for k, v in sorted(ledger_view.items())},
                },
                indent=1,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        return 0
    status = 0
    if unexplained:
        print(f"unexplained failure categories: {unexplained}", file=sys.stderr)
        status = 1
    if args.only is None:
        ledger = json.loads(args.ledger.read_text(encoding="utf-8"))
        recorded = {k: (v["outcome"], v.get("category")) for k, v in ledger["outcomes"].items()}
        changed = sorted(k for k in set(recorded) | set(ledger_view) if recorded.get(k) != ledger_view.get(k))
        if changed:
            print(f"{len(changed)} scenarios differ from the ledger (update it with --write-ledger):", file=sys.stderr)
            for key in changed[:40]:
                print(f"  {key}: ledger {recorded.get(key)} now {ledger_view.get(key)}", file=sys.stderr)
            status = 1
    return status


if __name__ == "__main__":
    sys.exit(main())
