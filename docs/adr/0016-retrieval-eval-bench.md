---
type: ADR
title: "ADR-0016: Retrieval evaluation bench"
description: A Python harness drives the shipped tarn-mcp binary over MCP/stdio against BEIR corpora and scores it with pytrec_eval, measuring retrieval only.
tags: [adr, benchmarks, evaluation, retrieval, testing]
status: accepted
decided: 2026-08-28
recorded: 2026-08-28
timestamp: 2026-08-28
---

# ADR-0016. Measure retrieval quality with an out-of-tree bench against the real binary

- **Status**: Accepted
- **Decided**: 2026-08-28
- **Recorded**: 2026-08-28
- **Deciders**: repo owner

## Context and Problem Statement

Retrieval quality is the thing Tarn exists to provide, and until now nothing measured it.
[Chapter 10](../architecture/10_quality_requirements.md) said so outright — *"There are no SLOs and
no benchmarks in this repository"* — with query latency and vault-size scalability both marked
`unmeasured`.

Every retrieval decision already recorded rests on judgement alone. BM25 parameters
([ADR-0006](0006-rank-fusion-over-independent-scorers.md)), the trigram-Jaccard tag scorer
([ADR-0007](0007-trigram-jaccard-tag-scorer.md)), stemming by default
([ADR-0008](0008-stemming-tokenizer-by-default.md)), the section as the indexed unit
([ADR-0005](0005-section-as-the-index-unit.md)) — each changes what comes back for a query, and none
can be shown to have helped. A ranking change cannot be reviewed when no number moves.

## Decision Drivers

- A score must be comparable to something outside this repository, or it measures only itself.
- The bench must exercise what agents actually talk to, not a convenient inner API.
- Retrieval quality must be isolated from generation quality, which would otherwise dominate it.
- A number is worthless without provenance — the build and corpus that produced it.
- It must not add a Python toolchain to the path of every contributor's normal build and test.

## Considered Options

- A Rust `benches/` target using criterion, calling `TarnCore::search` directly.
- A Python harness driving the shipped `tarn-mcp` binary over MCP/stdio against BEIR corpora.
- A hand-curated vault with hand-written relevance judgments.
- Full RAG evaluation: retrieve, generate an answer, judge it with an LLM.

## Decision Outcome

Chosen option: **a Python harness driving the shipped binary over MCP/stdio against BEIR corpora,
scored with `pytrec_eval`, measuring retrieval only.** It is the only option that produces an
externally comparable number, and the only one that measures the surface an agent actually uses.

Five decisions travel together:

1. **Retrieval only.** Generation is deferred until an answering model is in the loop.
2. **BEIR as the corpus format.** Ten datasets in one uniform shape, so one adapter covers all of
   them and adding another is adding its name.
3. **Drive the real binary over MCP/stdio.** Not `TarnCore::search`.
4. **`pytrec_eval` for qrel-based scoring, with ragas's non-LLM context metrics as a second view.**
   The first compares document ids, the second compares retrieved *text*.
5. **A Python project at the repo root, out of the Rust build.** Scripts in `scripts/bench/`, datasets in
   `data/`, results in `target/benchmarks/`. Nothing is committed but the harness itself.

### Why the real binary

Everything between `TarnCore::search` and an agent's `tools/call` is part of retrieval quality:
`SearchQuery::parse` turning `tag:` and `folder:` prefixes into hard filters, the `limit` that caps
sections *before* grouping into notes, the RRF-scaled `score_threshold`, the note grouping itself.
A bench calling `TarnCore::search` would score a code path no client uses, and would have found
neither defect below.

### Why not generation

An answer's quality is a product of retrieval *and* the answering model. With generation in the
loop, a ranking regression and a prompt change are indistinguishable in the score. There is also no
canonical model to standardise on, so the number would not be comparable across time or machines.
The harness is shaped so the stage can be added later without rework.

### Why results are not committed

Scores belong to a machine and a build, not to a branch. Committing them would put latency figures
from someone's laptop into the repository's history and invite merge conflicts on generated numbers.
Instead each run is preserved under `target/benchmarks/runs/<run-id>/` with a manifest, and
`report.md` carries its own history table with a delta column — so movement is visible without git
tracking the numbers.

### Consequences

- Good, because the scifact score (nDCG@10 = 0.6618) sits on the published BM25 baseline of ≈ 0.665,
  which is simultaneously a measurement and a proof that the harness is wired correctly.
- Good, because two defects surfaced on the first run against a real build — see *Confirmation*.
- Good, because `query latency` and `vault-size scalability` stop being `unmeasured`.
- Bad, because the repository now has a second language toolchain. It is confined: `uv` is required
  only for `make bench`, and `make build`/`test`/`lint`/`ci` are untouched.
- Bad, because BEIR corpora are large downloads and `data/` reaches gigabytes on tier C.
- Accepted limitation: **tiers B and C are not currently reachable.** Cold indexing is O(N²) in vault
  size — `InMemoryIndex` re-serialises all four state files on every note, which
  [ADR-0011](0011-synchronous-review-pass-before-serving.md) already records. scifact's 5,183 notes
  take ~5m30s; fiqa is eleven times larger. The bench measures the ceiling rather than pretending
  it is not there.
- Accepted limitation: English only, since there is one global analysis chain.

### Confirmation

The harness runs under `make bench` and is verified two ways.

**Against an external baseline.** BM25 on scifact is widely reported at nDCG@10 ≈ 0.665. A score in
that neighbourhood means the plumbing is right; a score near zero means the hit parsing or the vault
conversion is broken, not that Tarn is bad. This check is what separates a working harness from one
that merely runs.

**By what it found.** On its first execution against a real build it surfaced two defects that the
test suite does not cover, both of which change an advertised contract and so need their own
decisions before code:

- A `/` inside a heading makes the section unaddressable. `# The Sequence Alignment/Map format and
  SAMtools` is one heading, but the section path is joined with `/` and nothing escapes a `/` within
  a heading, so the index reports `heading_path` as two fake levels and
  `tarn://note/{path}#{section}` will not dereference it. 95 distinct sections across scifact's
  top-10 hits. nDCG is unaffected — the right note was still returned — which is precisely why the
  text-level second view exists.
- `serverInfo` advertises `rmcp 1.2.0` rather than `tarn 0.9.1`, because rmcp fills it with
  `Implementation::from_build_env()`, resolving `CARGO_PKG_*` at rmcp's own compile time.

## Pros and Cons of the Options

### A Rust `benches/` target using criterion

- Good, because it stays in one toolchain and one `cargo` invocation.
- Good, because criterion is the idiomatic Rust answer for timing.
- Bad, because criterion measures latency, not ranking quality; nDCG is not what it is for.
- Bad, because it would call `TarnCore::search` and skip query parsing, limits, thresholds and note
  grouping — scoring a path no client takes.
- Bad, because the corpus and qrels would still have to come from somewhere, and BEIR tooling is
  Python.

### A Python harness driving the real binary (chosen)

- Good, because it measures the exact surface an agent uses, over the exact transport.
- Good, because BEIR qrels are human relevance judgments, so the score is externally comparable.
- Good, because `pytrec_eval` is the standard TREC tool, not a reimplementation.
- Neutral, because it needs `uv`, which is confined to `make bench`.
- Bad, because a second language toolchain is a maintenance surface.

### A hand-curated vault with hand-written judgments

- Good, because it could mirror a real Obsidian vault, multi-section notes and all.
- Bad, because a score against self-authored judgments is comparable to nothing.
- Bad, because relevance judgments are expensive and drift with the author's intent.

### Full RAG evaluation with an LLM judge

- Good, because it measures what a user ultimately experiences.
- Bad, because generation quality confounds retrieval quality — the thing being measured here.
- Bad, because it needs an API key and a network call, so the bench stops being runnable offline.
- Bad, because there is no canonical answering model, so scores are not comparable across runs.

## More Information

- [scripts/bench/README.md](../../scripts/bench/README.md) — the operational guide: pipeline, catalogue, metric
  interpretation, and the manifest fields.
- [ADR-0005](0005-section-as-the-index-unit.md), [ADR-0006](0006-rank-fusion-over-independent-scorers.md),
  [ADR-0011](0011-synchronous-review-pass-before-serving.md) — the decisions this bench measures.
- [Chapter 10](../architecture/10_quality_requirements.md) — where the quality scenarios now point.
- **Revisit when**: an answering loop exists (add the generation stage and the LLM-judged metrics);
  the O(N²) cold index is fixed (tiers B and C become reachable); or a vector or graph retriever
  lands (it joins the existing fan-out, and arguana becomes the interesting number).
