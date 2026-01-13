# ARI Virtual Research Pipeline

This project is a lightweight, local reference implementation of the 4-stage ARI loop described in
`analysis_report.md`. It generates a virtual research design, runs a simulated experiment, drafts a
LaTeX paper, and feeds the final output back into a small knowledge store.

## What this does
- Cognitive loop: load a small domain corpus, build an in-memory vector store, and generate a novel
  hypothesis via a simple LBD-style heuristic.
- Execution loop: run an agentic tree search over a virtual experiment to pick the best parameters.
- Publication loop: write a LaTeX paper draft, verify citations against the local corpus, and run a
  multi-persona review pass.
- Recursive loop: extract future-work seeds and ingest them as new knowledge.

## Quickstart
```bash
python run_ari.py --topic "virtual experiment design" --domain "computer-science"
```

Outputs are written under `outputs/` with:
- `paper.tex` (final LaTeX draft)
- `review.txt` (multi-persona review notes)
- `results.json` (experiment results and metadata)

## Notes
- No external APIs are used. All behavior is deterministic with the configured seed.
- The corpus in `data/corpus.json` is small and meant as a placeholder for real crawled data.
