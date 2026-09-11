# Sindri AI authoring evaluator

This directory tests Sindri's AI authoring boundary without launching or
integrating the editor. It is a standalone Python 3 tool with no third-party
packages.

It sends small, fixed Sindri authoring requests to an OpenAI-compatible chat
completions endpoint and checks the returned proposal structurally and
semantically. It reports individual failures, latency, and an optional JSON
result file.

The contract being exercised is documented in
[`docs/ai-authoring-protocol.md`](../../docs/ai-authoring-protocol.md).

## What it tests

- strict proposal shape and version binding
- use of stable scene identities
- supported operation selection
- exact component fields and asset references
- clarification instead of ambiguous name guessing
- rejection of unavailable capabilities and file mutation
- project-data prompt injection resistance
- proposal-local spawn aliases and parent relationships
- malformed response rate and request latency

This is not an editor test. It does not apply proposals, mutate a project, or
exercise the future Sindri host adapter.

## Offline self-test

Run the parser, validator, scorer, and loopback HTTP transport tests:

```sh
python3 -m unittest discover -s tools/ai-authoring-eval -p 'test_*.py' -v
```

The transport test starts a temporary server on localhost and does not contact
an external service.

List the included model tasks:

```sh
python3 tools/ai-authoring-eval/run.py --list
```

Test the scorer with the included recorded proposal:

```sh
python3 tools/ai-authoring-eval/run.py \
  --case rename_selected_entity \
  --responses tools/ai-authoring-eval/fixtures/passing
```

These checks prove the harness works. They do not measure model quality.

## Live local-model test

Start a local server that offers an OpenAI-compatible
`/v1/chat/completions` endpoint and make sure the desired model is installed.
For an Ollama server using its default address:

```sh
python3 tools/ai-authoring-eval/run.py --model YOUR_MODEL_ID
```

The default endpoint is:

```text
http://127.0.0.1:11434/v1/chat/completions
```

Run one task while getting the setup working:

```sh
python3 tools/ai-authoring-eval/run.py \
  --model YOUR_MODEL_ID \
  --case rename_selected_entity
```

Run the corpus three times and save the raw proposals and measurements:

```sh
python3 tools/ai-authoring-eval/run.py \
  --model YOUR_MODEL_ID \
  --repeat 3 \
  --output ai-results.json
```

Repeated passes matter. One lucky valid response is not enough evidence that a
model is reliable for authoring.

## Other compatible servers

Set the full chat-completions URL with an option or environment variable:

```sh
SINDRI_AI_ENDPOINT=http://127.0.0.1:1234/v1/chat/completions \
SINDRI_AI_MODEL=local-model \
python3 tools/ai-authoring-eval/run.py
```

For an endpoint that requires a key, prefer the environment instead of shell
history:

```sh
export SINDRI_AI_API_KEY='...'
python3 tools/ai-authoring-eval/run.py \
  --endpoint https://provider.example/v1/chat/completions \
  --model provider-model
```

The evaluator never prints or writes the API key. Cloud use is explicit; there
is no automatic fallback from the default local endpoint.

## Provider output modes

The default `schema` mode sends the full proposal JSON Schema through the
OpenAI-compatible `response_format` field:

```sh
python3 tools/ai-authoring-eval/run.py --model YOUR_MODEL_ID
```

If a server supports JSON mode but not JSON Schema:

```sh
python3 tools/ai-authoring-eval/run.py \
  --model YOUR_MODEL_ID \
  --response-mode json
```

Use `plain` only to measure a provider with no constrained-output support:

```sh
python3 tools/ai-authoring-eval/run.py \
  --model YOUR_MODEL_ID \
  --response-mode plain
```

All modes use the same strict host-side validator. Constraint support can help a
model produce JSON, but it never replaces validation.

## Recorded responses

`--responses DIRECTORY` skips inference. For each selected case, the evaluator
reads `DIRECTORY/<case-id>.json` as the model's proposal and runs the normal
validation and scoring path.

This is useful for:

- reproducing a failure without keeping a model loaded
- comparing provider outputs in source control or CI artifacts
- developing the corpus and scorer offline

Recorded files contain the proposal object itself, not an OpenAI chat completion
envelope.

## Results and exit status

The console prints one `PASS` or `FAIL` line per case and repetition. A failure
then shows protocol diagnostic codes or semantic expectation failures.

Exit codes are:

| Code | Meaning |
| --- | --- |
| `0` | Every selected run passed. |
| `1` | At least one model response failed validation or expectations. |
| `2` | The evaluator configuration or corpus is invalid. |

The JSON report supplied with `--output` includes raw model proposals. Treat it
as project data if future corpora contain private scene context.

## Interpreting a pass

A full pass means the model handled these small, constrained tasks under the
selected output mode. It does not prove that the model can author arbitrary
Sindri projects or that a proposed component payload would pass the future live
engine schemas.

Before recommending a model, record at least:

- exact model ID and quantization
- provider and provider version
- GPU and available VRAM
- response mode
- pass rate across repeated runs
- median and worst-case latency

The initial corpus is intentionally small. It should grow from real authoring
failures while remaining focused enough to diagnose what a model got wrong.

## Files

| File | Purpose |
| --- | --- |
| `run.py` | Provider client, corpus runner, scorer, and report writer. |
| `protocol.py` | Dependency-free strict proposal validator. |
| `proposal.schema.json` | Provider-facing protocol version 1 schema. |
| `cases.json` | Versioned model-quality corpus. |
| `fixtures/` | Recorded proposals for offline harness checks. |
| `test_protocol.py` | Parser and validator unit tests. |
| `test_runner.py` | Scoring and loopback transport tests. |

