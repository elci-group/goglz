# Product planning with Goglz

Generate a complete, human-readable strategy workspace without credentials:

```bash
goglz plan generate --topic "My product"
goglz plan revise --artifact prd --instruction "Add interview evidence and an owner"
goglz plan check
```

Artifacts are written to `.goglz/plan/`:

- PRD, TRD, MVP scope, and user flow
- human-readable brand system
- database schemas, including `padagonia_accounts` and `padagonia_events`
- monetisation, launch, user acquisition, and growth plans

Generation is non-destructive unless `--force` is supplied. Revision defaults
to an auditable local brief; add `--ai` to use the configured generative
revision client. `plan check` validates required artifacts, titles, Padagonia
schema support, and cross-plan controls such as owners, decisions, and success
metrics.
