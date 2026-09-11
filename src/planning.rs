//! Product planning workspace support for `goglz plan`.
//!
//! The local generator is deliberately deterministic: teams can create a useful
//! baseline without credentials, then opt into AI revision with `--ai`.

use crate::ai_client::AiClient;
use crate::error::{GoglzError, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub const ARTIFACTS: &[(&str, &str)] = &[
    ("prd", "Product Requirements Document"),
    ("trd", "Technical Requirements Document"),
    ("mvp", "MVP Scope"),
    ("userflow", "User Flow"),
    ("brand", "Human-readable Brand System"),
    ("database", "Database Schemas"),
    ("monetisation", "Monetisation Plan"),
    ("launch", "Launch Plan"),
    ("acquisition", "User Acquisition Plan"),
    ("growth", "Growth Plan"),
];

pub struct PlanOptions {
    pub artifact: Option<String>,
    pub instruction: String,
    pub ai: bool,
}

pub struct CheckResult {
    pub name: String,
    pub passed: bool,
}
pub struct CheckReport {
    pub passed: bool,
    pub results: Vec<CheckResult>,
}

fn plan_dir(root: &Path) -> PathBuf {
    root.join(".goglz").join("plan")
}
fn artifact_path(root: &Path, slug: &str) -> PathBuf {
    plan_dir(root).join(format!("{slug}.md"))
}

pub fn generate(root: &Path, topic: Option<&str>, force: bool) -> Result<usize> {
    fs::create_dir_all(plan_dir(root))?;
    let topic = topic.unwrap_or("Goglz");
    let mut count = 0;
    for (slug, title) in ARTIFACTS {
        let path = artifact_path(root, slug);
        if path.exists() && !force {
            continue;
        }
        fs::write(path, template(slug, title, topic))?;
        count += 1;
    }
    Ok(count)
}

pub async fn revise<F>(root: &Path, options: &PlanOptions, client: F) -> Result<usize>
where
    F: FnOnce() -> Result<AiClient>,
{
    let slugs: Vec<&str> = match options.artifact.as_deref() {
        Some(slug) => {
            if !ARTIFACTS.iter().any(|(known, _)| *known == slug) {
                return Err(GoglzError::ProcessingFailed(format!(
                    "unknown planning artifact: {slug}"
                )));
            }
            vec![slug]
        }
        None => ARTIFACTS.iter().map(|(slug, _)| *slug).collect(),
    };
    let ai_client = if options.ai { Some(client()?) } else { None };
    let mut count = 0;
    for slug in slugs {
        let path = artifact_path(root, slug);
        if !path.exists() {
            continue;
        }
        let original = fs::read_to_string(&path)?;
        let revised = if options.ai {
            let ai = ai_client.as_ref().expect("AI client initialized");
            ai.revise_document(&format!(
                "Revise this product-planning artifact. Instruction: {}\n\n{}",
                options.instruction, original
            ))
            .await?
        } else {
            format!(
                "{}\n\n## Revision brief\n\n{}\n",
                original.trim_end(),
                options.instruction
            )
        };
        fs::write(path, revised)?;
        count += 1;
    }
    Ok(count)
}

pub fn check(root: &Path) -> Result<CheckReport> {
    let mut results = Vec::new();
    let mut contents = String::new();
    for (slug, title) in ARTIFACTS {
        let path = artifact_path(root, slug);
        let exists = path.is_file();
        results.push(CheckResult {
            name: format!("{slug}.md exists"),
            passed: exists,
        });
        let file_contents = if exists {
            fs::read_to_string(path)?
        } else {
            String::new()
        };
        if exists {
            contents.push_str(&file_contents);
        }
        if exists {
            results.push(CheckResult {
                name: format!("{slug}.md has title"),
                passed: file_contents.contains(title),
            });
        }
    }
    for term in [
        "Padagonia",
        "padagonia_accounts",
        "success metric",
        "owner",
        "decision",
    ] {
        results.push(CheckResult {
            name: format!("plan mentions {term}"),
            passed: contents.to_lowercase().contains(&term.to_lowercase()),
        });
    }
    let passed = results.iter().all(|result| result.passed);
    Ok(CheckReport { passed, results })
}

fn template(slug: &str, title: &str, topic: &str) -> String {
    let body = match slug {
        "prd" => "## Problem\nTeams need one auditable place to turn an idea into a shippable, measurable product.\n\n## Users and jobs\n- Founder: align decisions and priorities.\n- Product team: turn evidence into requirements.\n- Engineer: understand constraints and acceptance criteria.\n\n## Success metrics\n- Activation: a new team creates and checks a complete plan in one session.\n- Quality: 100% of required artifacts pass checks.\n- Outcome: interview-backed problem and measurable first release.\n\n## Requirements\n- Generate a complete baseline locally.\n- Revise individual artifacts with a brief or AI.\n- Preserve decisions, owners, assumptions, and open questions.",
        "trd" => "## Architecture\nRust CLI, deterministic file-backed artifacts, optional AI revision, and no network requirement for generation or checks.\n\n## Interfaces\n`plan generate`, `plan revise`, and `plan check` are stable user-facing commands.\n\n## Non-functional requirements\n- Safe by default: do not overwrite existing artifacts without `--force`.\n- Reproducible output and clear failures.\n- Keep secrets out of generated documents.",
        "mvp" => "## In scope\n- Ten linked planning artifacts.\n- Local generation and revision briefs.\n- Optional generative AI revision.\n- Automated completeness and consistency checks.\n\n## Out of scope\nBilling execution, CRM sync, analytics ingestion, and multi-user editing.\n\n## Acceptance criteria\nA user can generate, revise one document, and receive a passing check report without credentials.",
        "userflow" => "## Happy path\n1. Run `goglz plan generate`.\n2. Read PRD and MVP; record decisions and owners.\n3. Revise a focused artifact with an instruction.\n4. Run `goglz plan check`.\n5. Share the checked plan and launch the first experiment.\n\n## Failure paths\nMissing artifact blocks checks; unknown artifact fails fast; AI failure leaves the original content untouched.",
        "brand" => "## Personality\nWarm, precise, optimistic, and useful. Goglz speaks like a sharp teammate who makes ambiguity smaller.\n\n## Voice\nUse plain language, short sentences, and concrete verbs. Explain specialist terms. Never manufacture certainty.\n\n## Visual direction\nQuiet dark ink, warm paper, one electric accent, generous spacing, and accessible contrast.\n\n## Do\nSay what matters, why it matters, and what happens next.\n\n## Avoid\nHype, empty growth language, jargon walls, and manipulative urgency.",
        "database" => "## Principles\nUse stable IDs, explicit ownership, append-only decisions, and timestamps.\n\n## Core tables\n- `workspaces(id, name, created_at)`\n- `users(id, workspace_id, role, created_at)`\n- `artifacts(id, workspace_id, kind, version, status, body, updated_at)`\n- `decisions(id, artifact_id, decision, rationale, owner_id, created_at)`\n- `experiments(id, workspace_id, hypothesis, metric, status, owner_id)`\n- `events(id, workspace_id, event_name, actor_id, occurred_at, metadata_json)`\n\n## Padagonia support\n- `padagonia_accounts(id, workspace_id, external_id, region, consent_status, created_at)`\n- `padagonia_events(id, padagonia_account_id, event_name, payload_json, occurred_at)`\nPadagonia is an explicit integration boundary: isolate external identifiers, record consent, and never use its payload as a substitute for product truth.\n\n## Invariants\nEvery artifact has one workspace; every decision has an owner; external IDs are unique per integration.",
        "monetisation" => "## Model\nStart with a free planning workspace, then charge for team collaboration and governed AI revision.\n\n## Packaging\n- Free: one workspace, local generation, checks.\n- Team: shared plans, revision history, exports, and usage allowance.\n- Business: SSO, audit retention, controls, and support.\n\n## Guardrails\nShow value before asking for payment; publish limits; no dark patterns. Track conversion, retention, gross margin, and support burden.",
        "launch" => "## Gates\n- Problem evidence and named design partners.\n- Passing plan checks and threat-model review.\n- Instrumented activation and support route.\n\n## Sequence\nPrivate alpha → design-partner pilot → public beta → paid conversion. Each stage has an owner, entry criteria, exit criteria, and rollback plan.\n\n## Launch checks\nConfirm docs, onboarding, pricing, status page, feedback loop, and incident response before announcing.",
        "acquisition" => "## ICP\nSmall product teams and founder-led technical businesses that lose decisions in documents and chat.\n\n## Channels\nFounder-led demos, practical planning templates, communities, partner referrals, and search content.\n\n## Experiment card\nEvery channel test records a hypothesis, audience, owner, budget, landing action, activation metric, and stop rule. Avoid buying reach before message-market fit.",
        "growth" => "## Loop\nA team generates a plan, checks it, shares a useful artifact, and invites collaborators who add decisions and experiments.\n\n## Metrics\nNorth star: teams reaching a checked, decision-ready plan. Guardrails: retention, revision quality, AI cost per active workspace, complaint rate, and privacy incidents.\n\n## Review cadence\nWeekly experiment review; monthly cohort review; quarterly strategy revision. Every change has a decision owner and evidence.",
        _ => "",
    };
    format!("# {title}\n\n> Generated by Goglz for **{topic}**. Treat assumptions as hypotheses and assign an owner before acting.\n\n{body}\n\n## Open questions\n- What evidence would change this plan?\n- Who owns the next decision?\n- What is the smallest reversible experiment?\n")
}
