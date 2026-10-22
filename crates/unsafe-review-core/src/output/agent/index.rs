//! Compact agent task index over one analysis (`agent-task-index/v1`,
//! issue #2311 PR1).
//!
//! An LLM integration needs the top bounded tasks without parsing Markdown
//! or loading every card packet. The [`TaskIndex`] lists one row per
//! actionable card (card-level fallback until #2274 action grouping lands),
//! each row carrying only the identities, vocabulary values, and commands a
//! model needs to select one task and fetch its packet. No source excerpts,
//! repair prose, test lists, history, prompts, secrets, or witness logs are
//! embedded: the packet behind [`AgentTask::packet_command`] owns those.
//!
//! Every machine value reuses canonical logic: readiness comes from the shared
//! domain readiness classifier with the same card-scoped repair input as the
//! agent packet (output audit #1687), movement from the card coverage block,
//! roles from scan-time classification, and action kinds from canonical
//! missing-evidence kinds.
//! Nothing here reclassifies data, and no second ranker exists: rows follow
//! the pipeline's card order, truncated only by an explicit cap.

use crate::domain::coverage::compute_agent_lsp_readiness;
use crate::domain::{AgentLspReadiness, ReviewCard};
use std::path::PathBuf;

/// Versioned schema identity for the task index contract.
pub const AGENT_TASK_INDEX_SCHEMA: &str = "unsafe-review/agent-task-index/v1";

/// Default hard cap on tasks per index. Truncation is always visible (see
/// [`TaskIndexTruncation`]) with an expansion path.
pub const DEFAULT_MAX_TASKS: usize = 20;

/// Closed role vocabulary for `--role` filtering. Matches scan-time
/// classification; `unknown` is the default bucket, never a suppression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRole {
    Production,
    Test,
    Generated,
    Unknown,
}

impl TaskRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskRole::Production => "production",
            TaskRole::Test => "test",
            TaskRole::Generated => "generated",
            TaskRole::Unknown => "unknown",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "production" => Ok(TaskRole::Production),
            "test" => Ok(TaskRole::Test),
            "generated" => Ok(TaskRole::Generated),
            "unknown" => Ok(TaskRole::Unknown),
            other => Err(format!(
                "unknown --role `{other}`; expected production, test, generated, or unknown"
            )),
        }
    }
}

/// Closed readiness vocabulary for `--readiness` filtering. Identical to the
/// canonical agent-readiness states (`ready`, `needs_human`,
/// `requires_witness_receipt`, `unsupported`); unknown values fail explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskReadiness {
    Ready,
    NeedsHuman,
    RequiresWitness,
    Unsupported,
}

impl TaskReadiness {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskReadiness::Ready => "ready",
            TaskReadiness::NeedsHuman => "needs_human",
            TaskReadiness::RequiresWitness => "requires_witness_receipt",
            TaskReadiness::Unsupported => "unsupported",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "ready" => Ok(TaskReadiness::Ready),
            "needs_human" => Ok(TaskReadiness::NeedsHuman),
            "requires_witness_receipt" => Ok(TaskReadiness::RequiresWitness),
            "unsupported" => Ok(TaskReadiness::Unsupported),
            other => Err(format!(
                "unknown --readiness `{other}`; expected ready, needs_human, requires_witness_receipt, or unsupported"
            )),
        }
    }

    fn of(state: AgentLspReadiness) -> Self {
        match state {
            AgentLspReadiness::Ready => TaskReadiness::Ready,
            AgentLspReadiness::NeedsHuman => TaskReadiness::NeedsHuman,
            AgentLspReadiness::RequiresWitnessReceipt => TaskReadiness::RequiresWitness,
            AgentLspReadiness::Unsupported => TaskReadiness::Unsupported,
        }
    }
}

/// Selection options for [`assemble_task_index`]. Filters never reclassify:
/// a filtered row is omitted, never rewritten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskIndexOptions {
    pub role: Option<TaskRole>,
    pub readiness: Option<TaskReadiness>,
    /// Select only `needs_human` tasks.
    pub human_only: bool,
    /// Select only cards on changed lines.
    pub changed_only: bool,
    pub max_tasks: usize,
}

impl Default for TaskIndexOptions {
    fn default() -> Self {
        Self {
            role: None,
            readiness: None,
            human_only: false,
            changed_only: false,
            max_tasks: DEFAULT_MAX_TASKS,
        }
    }
}

/// One bounded task row: identities, closed-vocabulary values, and the exact
/// commands to fetch the packet and recheck the scope. Card-level fallback:
/// `task_id` and `subject_id` are the card id until #2274 grouping lands.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentTask {
    pub task_id: String,
    pub subject_id: String,
    pub card_id: String,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub class: String,
    pub priority: String,
    pub movement: String,
    pub baseline_state: String,
    pub role: String,
    pub mechanism: String,
    pub obligations: Vec<String>,
    pub action_kind: String,
    pub route_readiness: String,
    pub readiness_reasons: Vec<String>,
    pub agent_applicability: String,
    pub changed: bool,
    pub packet_command: String,
    pub recheck_command: String,
}

/// Visible truncation record: omitted counts and the expansion path. Always
/// present so a capped index can never pass as complete.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskIndexTruncation {
    pub truncated: bool,
    pub selected_tasks: usize,
    pub omitted_tasks: usize,
    pub expansion: String,
}

/// Compact task index for one analysis revision.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskIndex {
    pub schema_version: String,
    pub analysis_id: String,
    pub scope: String,
    pub changeset_digest: String,
    pub tasks: Vec<AgentTask>,
    pub truncation: TaskIndexTruncation,
    pub digest: String,
}

fn role_str(card: &ReviewCard) -> &'static str {
    match card.site.role {
        crate::domain::SourceRole::Production => "production",
        crate::domain::SourceRole::Test => "test",
        crate::domain::SourceRole::Generated => "generated",
        crate::domain::SourceRole::Unknown => "unknown",
    }
}

fn has_missing_kind(card: &ReviewCard, kind: &str) -> bool {
    card.missing.iter().any(|missing| missing.kind == kind)
}

/// Action kind from canonical missing-evidence kinds in repair order:
/// guard, contract, test (reach), witness, else human review. The vocabulary
/// matches #2311; the derivation matches the repair-queue buckets.
fn action_kind(card: &ReviewCard) -> &'static str {
    if has_missing_kind(card, "guard") {
        "guard"
    } else if has_missing_kind(card, "contract") {
        "contract"
    } else if has_missing_kind(card, "reach") {
        "test"
    } else if has_missing_kind(card, "witness") {
        "witness"
    } else {
        "human_review"
    }
}

/// Agent applicability from canonical readiness: an agent may take `Ready`
/// tasks, a human must take the rest, and witness-gated tasks wait on an
/// external receipt. Unsupported tasks are human-only with reasons: nothing
/// agent-actionable exists, but the finding stays visible.
fn applicability(state: AgentLspReadiness) -> &'static str {
    match state {
        AgentLspReadiness::Ready => "candidate",
        AgentLspReadiness::RequiresWitnessReceipt => "requires_witness",
        AgentLspReadiness::NeedsHuman | AgentLspReadiness::Unsupported => "human_only",
    }
}

fn task_matches(
    card: &ReviewCard,
    readiness: AgentLspReadiness,
    options: &TaskIndexOptions,
) -> bool {
    if options
        .role
        .is_some_and(|role| role.as_str() != role_str(card))
    {
        return false;
    }
    if options
        .readiness
        .is_some_and(|want| want != TaskReadiness::of(readiness))
    {
        return false;
    }
    if options.human_only && readiness != AgentLspReadiness::NeedsHuman {
        return false;
    }
    if options.changed_only && !card.site.changed {
        return false;
    }
    true
}

/// Assemble the task index over one analysis output: only actionable cards
/// become tasks, in pipeline order, filtered without reclassification and
/// truncated only by `max_tasks`. `changeset_digest` pins the analyzed
/// source state so a consumer can detect staleness; `recheck_command` names
/// the exact command that reproduces this index. Deterministic: row order is
/// the pipeline's card order and the digest covers the canonical encoding.
pub fn assemble_task_index(
    output: &crate::AnalyzeOutput,
    changeset_digest: &str,
    recheck_command: &str,
    packet_command_prefix: &str,
    options: &TaskIndexOptions,
) -> TaskIndex {
    let mut selected = Vec::new();
    let mut omitted = 0usize;
    for card in &output.cards {
        if !card.class.is_actionable() {
            continue;
        }
        let scoped_repairs = super::card_has_scoped_repairs(card);
        let readiness = compute_agent_lsp_readiness(card, scoped_repairs);
        if !task_matches(card, readiness.state, options) {
            continue;
        }
        if selected.len() >= options.max_tasks {
            omitted += 1;
            continue;
        }
        let coverage = card.coverage_block();
        let obligations: Vec<String> = card
            .obligations
            .iter()
            .map(|obligation| obligation.key.clone())
            .collect();
        selected.push(AgentTask {
            task_id: card.id.0.clone(),
            subject_id: card.id.0.clone(),
            card_id: card.id.0.clone(),
            file: card.site.location.file.clone(),
            line: card.site.location.line,
            column: card.site.location.column,
            class: card.class.as_str().to_string(),
            priority: card.priority.as_str().to_string(),
            movement: coverage.outcome_movement.as_str().to_string(),
            baseline_state: coverage.baseline_state.as_str().to_string(),
            role: role_str(card).to_string(),
            mechanism: card.operation.family.as_str().to_string(),
            obligations,
            action_kind: action_kind(card).to_string(),
            route_readiness: readiness.state.as_str().to_string(),
            readiness_reasons: readiness.reasons.clone(),
            agent_applicability: applicability(readiness.state).to_string(),
            changed: card.site.changed,
            packet_command: format!("{packet_command_prefix} {}", card.id.0),
            recheck_command: recheck_command.to_string(),
        });
    }
    let selected_tasks = selected.len();
    let mut index = TaskIndex {
        schema_version: AGENT_TASK_INDEX_SCHEMA.to_string(),
        analysis_id: output.analysis_identity.analysis_id.clone(),
        scope: output.scope.as_str().to_string(),
        changeset_digest: changeset_digest.to_string(),
        tasks: selected,
        truncation: TaskIndexTruncation {
            truncated: omitted > 0,
            selected_tasks,
            omitted_tasks: omitted,
            expansion: format!(
                "raise --max-tasks above {} to include the omitted rows",
                options.max_tasks
            ),
        },
        digest: String::new(),
    };
    let encoding = serde_json::to_string(&index).unwrap_or_default();
    index.digest = format!(
        "task-index-sha256:{}",
        crate::sha256_hex_of(encoding.as_bytes())
    );
    index
}

/// Compact human rendering of the index: one line per task plus the
/// truncation record. JSON carries the full contract; this is the
/// same tasks in skimmable form.
pub fn render_task_index_human(index: &TaskIndex) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Task index {} (analysis {}, {} tasks, digest {}):\n",
        index.schema_version,
        index.analysis_id,
        index.tasks.len(),
        index.digest
    ));
    for task in &index.tasks {
        out.push_str(&format!(
            "- {} {}:{} {} {} {} [{}]\n",
            task.task_id,
            task.file.display(),
            task.line,
            task.class,
            task.action_kind,
            task.agent_applicability,
            task.route_readiness
        ));
    }
    if index.truncation.truncated {
        out.push_str(&format!(
            "- truncated: {} selected, {} omitted; {}\n",
            index.truncation.selected_tasks,
            index.truncation.omitted_tasks,
            index.truncation.expansion
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CardId, Confidence, ContractEvidence, DischargeEvidence, HazardKind, MissingEvidence,
        NextAction, OperationFamily, Priority, ProofPath, ReachEvidence, ReviewCard, ReviewClass,
        SourceLocation, SourceRole, UnsafeOperation, UnsafeSite, UnsafeSiteKind, WitnessEvidence,
        WitnessKind, WitnessRoute,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn card(
        id: &str,
        class: ReviewClass,
        role: SourceRole,
        changed: bool,
        missing: Vec<MissingEvidence>,
    ) -> ReviewCard {
        ReviewCard {
            id: CardId(id.to_string()),
            class,
            priority: Priority::High,
            confidence: Confidence::High,
            proof_path: ProofPath::SourceRouteOnly,
            site: UnsafeSite {
                location: SourceLocation {
                    file: "src/lib.rs".into(),
                    line: 1,
                    column: 1,
                },
                kind: UnsafeSiteKind::Operation,
                owner: Some("owner".to_string()),
                visibility: "private".to_string(),
                public_api_surface: false,
                changed,
                snippet: "unsafe { *ptr }".to_string(),
                role,
            },
            operation: UnsafeOperation {
                expression: "unsafe { *ptr }".to_string(),
                family: OperationFamily::RawPointerDeref,
                bound_name: None,
            },
            hazards: vec![HazardKind::PointerValidity],
            obligations: vec![],
            obligation_evidence: vec![],
            contract: ContractEvidence::missing(),
            discharge: DischargeEvidence::missing(),
            reach: ReachEvidence {
                state: "missing".to_string(),
                summary: "no tests".to_string(),
            },
            witness: WitnessEvidence::missing(),
            missing,
            routes: vec![WitnessRoute {
                kind: WitnessKind::Miri,
                reason: "test".to_string(),
                command: Some("cargo miri test".to_string()),
                required: false,
            }],
            next_action: NextAction {
                summary: "add guard".to_string(),
                verify_commands: vec!["cargo miri test".to_string()],
            },
            related_tests: vec![],
        }
    }

    fn missing(kind: &str) -> MissingEvidence {
        MissingEvidence {
            kind: kind.to_string(),
            message: format!("{kind} missing"),
        }
    }

    fn analyzed_output(cards: Vec<ReviewCard>) -> crate::AnalyzeOutput {
        crate::AnalyzeOutput {
            analysis_identity: crate::AnalysisIdentity::new("diff"),
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: crate::Scope::Diff,
            mode: crate::AnalysisMode::Draft,
            policy: crate::PolicyMode::Advisory,
            summary: crate::api::Summary::default(),
            cards,
            diff_scoped_files: BTreeSet::from([PathBuf::from("src/lib.rs")]),
            unresolved_diff_files: BTreeSet::new(),
            rejected_diff_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        }
    }

    fn assemble(cards: Vec<ReviewCard>, options: &TaskIndexOptions) -> TaskIndex {
        assemble_task_index(
            &analyzed_output(cards),
            "changeset-sha256:abc",
            "unsafe-review agent tasks --scope staged",
            "unsafe-review context --root .",
            options,
        )
    }

    fn assemble_from(output: &crate::AnalyzeOutput, options: &TaskIndexOptions) -> TaskIndex {
        assemble_task_index(
            output,
            "changeset-sha256:abc",
            "unsafe-review agent tasks --scope staged",
            "unsafe-review context --root .",
            options,
        )
    }

    #[test]
    fn index_selects_only_actionable_cards_with_identities() -> Result<(), String> {
        let cards = vec![
            card(
                "UR-a-c1",
                ReviewClass::GuardMissing,
                SourceRole::Production,
                true,
                vec![missing("guard")],
            ),
            card(
                "UR-a-c2",
                ReviewClass::GuardedAndWitnessed,
                SourceRole::Production,
                true,
                vec![],
            ),
        ];
        let index = assemble(cards, &TaskIndexOptions::default());
        if index.schema_version != AGENT_TASK_INDEX_SCHEMA {
            return Err("schema identity must survive assembly".to_string());
        }
        if index.tasks.len() != 1 {
            return Err("only the actionable card becomes a task".to_string());
        }
        let task = &index.tasks[0];
        if task.task_id != "UR-a-c1" || task.subject_id != "UR-a-c1" {
            return Err("card-level fallback uses the card id".to_string());
        }
        if task.action_kind != "guard" {
            return Err("guard-missing cards take guard actions".to_string());
        }
        if task.role != "production" || task.mechanism != "raw_pointer_deref" {
            return Err("role and mechanism must project".to_string());
        }
        if index.changeset_digest != "changeset-sha256:abc" {
            return Err("changeset digest pins the analyzed state".to_string());
        }
        if !index.digest.starts_with("task-index-sha256:") {
            return Err("digest must be namespaced".to_string());
        }
        if index.truncation.truncated {
            return Err("nothing is truncated below the cap".to_string());
        }
        let human = render_task_index_human(&index);
        if !human.contains("UR-a-c1") {
            return Err("human render must name the task".to_string());
        }
        Ok(())
    }

    #[test]
    fn index_is_deterministic_for_the_same_input() -> Result<(), String> {
        let output = analyzed_output(vec![card(
            "UR-b-c1",
            ReviewClass::ContractMissing,
            SourceRole::Test,
            true,
            vec![missing("contract")],
        )]);
        let first = assemble_from(&output, &TaskIndexOptions::default());
        let second = assemble_from(&output, &TaskIndexOptions::default());
        if first.digest != second.digest || first != second {
            return Err("same input must assemble identically".to_string());
        }
        Ok(())
    }

    #[test]
    fn filters_narrow_without_reclassifying() -> Result<(), String> {
        let cards = vec![
            card(
                "UR-c-c1",
                ReviewClass::GuardMissing,
                SourceRole::Production,
                true,
                vec![missing("guard")],
            ),
            card(
                "UR-c-c2",
                ReviewClass::ContractMissing,
                SourceRole::Test,
                false,
                vec![missing("contract")],
            ),
        ];
        let role = assemble(
            cards.clone(),
            &TaskIndexOptions {
                role: Some(TaskRole::Test),
                ..TaskIndexOptions::default()
            },
        );
        if role.tasks.len() != 1 || role.tasks[0].task_id != "UR-c-c2" {
            return Err("role filter must select without rewriting".to_string());
        }
        let changed = assemble(
            cards.clone(),
            &TaskIndexOptions {
                changed_only: true,
                ..TaskIndexOptions::default()
            },
        );
        if changed.tasks.len() != 1 || changed.tasks[0].task_id != "UR-c-c1" {
            return Err("changed filter must select changed sites".to_string());
        }
        // Witness-gated cards keep their readiness under every filter.
        let witness = assemble(
            vec![card(
                "UR-c-c3",
                ReviewClass::ReachableUnwitnessed,
                SourceRole::Production,
                true,
                vec![missing("witness")],
            )],
            &TaskIndexOptions::default(),
        );
        if witness.tasks.len() != 1 {
            return Err("witness-gated cards stay visible".to_string());
        }
        if witness.tasks[0].action_kind != "witness" {
            return Err("witness-missing cards take witness actions".to_string());
        }
        Ok(())
    }

    #[test]
    fn caps_truncate_visibly_with_expansion() -> Result<(), String> {
        let cards = vec![
            card(
                "UR-d-c1",
                ReviewClass::GuardMissing,
                SourceRole::Production,
                true,
                vec![missing("guard")],
            ),
            card(
                "UR-d-c2",
                ReviewClass::ContractMissing,
                SourceRole::Production,
                true,
                vec![missing("contract")],
            ),
        ];
        let index = assemble(
            cards,
            &TaskIndexOptions {
                max_tasks: 1,
                ..TaskIndexOptions::default()
            },
        );
        if index.tasks.len() != 1 {
            return Err("cap must bound the rows".to_string());
        }
        if !index.truncation.truncated || index.truncation.omitted_tasks != 1 {
            return Err("omitted rows must be counted".to_string());
        }
        if !index.truncation.expansion.contains("--max-tasks") {
            return Err("expansion path must name the flag".to_string());
        }
        Ok(())
    }

    #[test]
    fn closed_vocabularies_reject_unknown_values() -> Result<(), String> {
        if TaskRole::parse("prod").is_ok() {
            return Err("unknown roles must fail explicitly".to_string());
        }
        if TaskReadiness::parse("eventually").is_ok() {
            return Err("unknown readiness must fail explicitly".to_string());
        }
        Ok(())
    }
}
