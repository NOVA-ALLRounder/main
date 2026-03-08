import { z } from "zod";

// System Status Schema
export const SystemStatusSchema = z.object({
    cpu_usage: z.number(),
    memory_used: z.number(),
    memory_total: z.number(),
});

// Log Entry Schema (for recent activity)
export const LogEntrySchema = z.object({
    timestamp: z.string(),
    level: z.string(),
    message: z.string(),
});

// Routine Summary Schema
export const RoutineSchema = z.object({
    id: z.number(),
    name: z.string(),
    cron_expression: z.string(),
    enabled: z.number().or(z.boolean()).transform(v => Boolean(v)),
    next_run: z.string().nullable(),
});

// Recommendation/Workflow Schema
export const RecommendationSchema = z.object({
    id: z.number(),
    title: z.string(),
    summary: z.string(),
    status: z.string(),
    confidence: z.number(),
    category: z.string().optional().default("unknown"),
    business_score: z.number().optional().default(0),
    evidence: z.array(z.string()).optional(), // [NEW] Explainability
    last_error: z.string().nullable().optional(),
    workflow_id: z.string().nullable().optional(),
    workflow_url: z.string().nullable().optional(),
    snoozed_until: z.string().nullable().optional(),
    approval_ready: z.boolean().optional().default(true),
    approval_reasons: z.array(z.string()).optional().default([]),
});

export const ApproveRecommendationResponseSchema = z.object({
    status: z.string(),
    id: z.string().nullable().optional(),
    workflow_id: z.string().nullable().optional(),
    workflow_url: z.string().nullable().optional(),
    provision_op_id: z.number().nullable().optional(),
    provision_status: z.string().nullable().optional(),
    provision_updated_at: z.string().nullable().optional(),
    provision_claim: z.string().nullable().optional(),
    approved_now: z.boolean().optional(),
    reused_existing: z.boolean().optional(),
    message: z.string().optional(),
});

export const RecommendationFeedbackResponseSchema = z.object({
    ok: z.boolean(),
    sentiment: z.string(),
    status: z.string(),
    message: z.string(),
    suppressed_similar: z.boolean(),
});

export const WorkflowProvisionOpSchema = z.object({
    id: z.number(),
    recommendation_id: z.number(),
    claim_token: z.string().nullable().optional(),
    status: z.string(),
    workflow_id: z.string().nullable().optional(),
    workflow_json: z.string().nullable().optional(),
    error: z.string().nullable().optional(),
    created_at: z.string(),
    updated_at: z.string(),
});

export const RecommendationMetricsSchema = z.object({
    total: z.number(),
    approved: z.number(),
    rejected: z.number(),
    failed: z.number(),
    pending: z.number(),
    later: z.number(),
    legacy_other: z.number().optional(),
    approval_rate: z.number(),
    last_created_at: z.string().nullable().optional(),
});

export const RecommendationReviewMetricsSchema = z.object({
    window_size: z.number(),
    total_events: z.number(),
    approve_actions: z.number(),
    reject_actions: z.number(),
    later_actions: z.number(),
    restore_actions: z.number(),
    feedback_positive: z.number(),
    feedback_refine: z.number(),
    feedback_negative: z.number(),
    failed_actions: z.number(),
    action_failure_rate: z.number(),
    non_positive_feedback_rate: z.number(),
    last_event_at: z.string().nullable().optional(),
});

export const RecommendationReviewEventRecordSchema = z.object({
    id: z.number(),
    created_at: z.string(),
    recommendation_id: z.number(),
    recommendation_title: z.string(),
    status_after: z.string().nullable().optional(),
    category: z.string().nullable().optional(),
    action: z.string(),
    actor: z.string().nullable().optional(),
    note: z.string().nullable().optional(),
    ok: z.boolean(),
    message: z.string().nullable().optional(),
});

export const RequestMemoryRecordSchema = z.object({
    normalized_request: z.string(),
    memory_scope: z.string(),
    original_request: z.string(),
    request_signature: z.string().nullable().optional(),
    intent_json: z.string().nullable().optional(),
    intent_command: z.string().nullable().optional(),
    response_text: z.string().nullable().optional(),
    response_mode: z.string(),
    source: z.string(),
    confidence: z.number(),
    positive_feedback_count: z.number(),
    negative_feedback_count: z.number(),
    last_feedback_at: z.string().nullable().optional(),
    suppressed: z.boolean().optional().default(false),
    suppressed_reason: z.string().nullable().optional(),
    suppressed_at: z.string().nullable().optional(),
    use_count: z.number(),
    created_at: z.string(),
    updated_at: z.string(),
    last_used_at: z.string(),
});

export const ExecutionMemoryRecordSchema = z.object({
    id: z.number(),
    memory_scope: z.string(),
    intent_command: z.string(),
    params_key: z.string(),
    params_json: z.string().nullable().optional(),
    request_signature: z.string().nullable().optional(),
    response_text: z.string(),
    source: z.string(),
    tool_path: z.string(),
    freshness_ttl_seconds: z.number(),
    success: z.boolean(),
    positive_feedback_count: z.number(),
    negative_feedback_count: z.number(),
    last_feedback_at: z.string().nullable().optional(),
    suppressed: z.boolean().optional().default(false),
    suppressed_reason: z.string().nullable().optional(),
    suppressed_at: z.string().nullable().optional(),
    use_count: z.number(),
    created_at: z.string(),
    updated_at: z.string(),
    last_used_at: z.string(),
});

export const MemoryOpsMetricsSchema = z.object({
    request_active: z.number(),
    request_suppressed: z.number(),
    execution_active: z.number(),
    execution_suppressed: z.number(),
    last_request_used_at: z.string().nullable().optional(),
    last_execution_used_at: z.string().nullable().optional(),
});

export const MemoryAdminEventRecordSchema = z.object({
    id: z.number(),
    created_at: z.string(),
    kind: z.string(),
    action: z.string(),
    memory_scope: z.string().nullable().optional(),
    target_key: z.string(),
    reason: z.string().nullable().optional(),
    actor: z.string().nullable().optional(),
    ok: z.boolean(),
    message: z.string().nullable().optional(),
});

export const MemoryRecordsResponseSchema = z.object({
    metrics: MemoryOpsMetricsSchema,
    request_memory: z.array(RequestMemoryRecordSchema),
    execution_memory: z.array(ExecutionMemoryRecordSchema),
    recent_admin_events: z.array(MemoryAdminEventRecordSchema).optional().default([]),
});

export const MemoryAdminActionResponseSchema = z.object({
    ok: z.boolean(),
    kind: z.string(),
    action: z.string(),
    message: z.string(),
});

export const ChatRouteMetaSchema = z.object({
    route_kind: z.string(),
    outcome: z.string(),
    confidence: z.number().nullable().optional(),
    note: z.string().nullable().optional(),
    memory_scope: z.string().nullable().optional(),
    freshness_bypassed: z.boolean(),
    intent_memory_hit: z.boolean(),
    request_memory_hit: z.boolean(),
    execution_memory_hit: z.boolean(),
    deterministic_used: z.boolean(),
    llm_used: z.boolean(),
    ai_digest_used: z.boolean(),
    local_only: z.boolean(),
});

export const ChatMessageResponseSchema = z.object({
    response: z.string(),
    command: z.string().optional(),
    route_meta: ChatRouteMetaSchema.optional(),
});

export const LaunchOpsRouteBreakdownSchema = z.object({
    route_kind: z.string(),
    count: z.number(),
});

export const LaunchOpsEventRecordSchema = z.object({
    id: z.number(),
    created_at: z.string(),
    channel: z.string().nullable().optional(),
    memory_scope: z.string().nullable().optional(),
    message_preview: z.string(),
    route_kind: z.string(),
    command: z.string().nullable().optional(),
    outcome: z.string(),
    confidence: z.number().nullable().optional(),
    freshness_bypassed: z.boolean(),
    intent_memory_hit: z.boolean(),
    request_memory_hit: z.boolean(),
    execution_memory_hit: z.boolean(),
    deterministic_used: z.boolean(),
    llm_used: z.boolean(),
    ai_digest_used: z.boolean(),
    local_only: z.boolean(),
    note: z.string().nullable().optional(),
});

export const LaunchOpsMetricsSchema = z.object({
    window_size: z.number(),
    total_requests: z.number(),
    blocked_requests: z.number(),
    intent_memory_hits: z.number(),
    request_memory_hits: z.number(),
    execution_memory_hits: z.number(),
    cached_response_hit_rate: z.number(),
    deterministic_routes: z.number(),
    llm_routes: z.number(),
    ai_digest_routes: z.number(),
    ai_digest_auto_routes: z.number(),
    local_routes: z.number(),
    freshness_bypasses: z.number(),
    low_confidence_routes: z.number(),
    unknown_routes: z.number(),
    error_routes: z.number(),
    last_event_at: z.string().nullable().optional(),
    route_breakdown: z.array(LaunchOpsRouteBreakdownSchema),
});

export const NLRunMetricsSchema = z.object({
    total: z.number(),
    completed: z.number(),
    manual_required: z.number(),
    approval_required: z.number(),
    blocked: z.number(),
    error: z.number(),
    success_rate: z.number(),
});

export const ExecApprovalMetricsSchema = z.object({
    window_size: z.number(),
    total: z.number(),
    pending: z.number(),
    approved: z.number(),
    rejected: z.number(),
    expired_pending: z.number(),
    allow_once: z.number(),
    allow_always: z.number(),
    deny: z.number(),
    approval_rate: z.number(),
    oldest_pending_created_at: z.string().nullable().optional(),
    last_created_at: z.string().nullable().optional(),
    last_resolved_at: z.string().nullable().optional(),
});

export const LaunchOpsResponseSchema = z.object({
    chat_metrics: LaunchOpsMetricsSchema,
    memory_metrics: MemoryOpsMetricsSchema,
    nl_run_metrics: NLRunMetricsSchema,
    exec_approval_metrics: ExecApprovalMetricsSchema,
    recommendation_metrics: RecommendationMetricsSchema,
    recommendation_review_metrics: RecommendationReviewMetricsSchema,
    recent_events: z.array(LaunchOpsEventRecordSchema),
});

export const LaunchEvalCandidateSchema = z.object({
    id: z.string(),
    provenance: z.string(),
    source_kind: z.string(),
    scenario_kind: z.string(),
    title: z.string(),
    score: z.number(),
    command: z.string().nullable().optional(),
    request_message: z.string(),
    rationale: z.array(z.string()),
    yaml: z.string(),
});

export const LaunchEvalCandidateSnapshotSchema = z.object({
    generated_at: z.string(),
    output_path: z.string(),
    provenance_filter: z.string(),
    scenario_count: z.number(),
    candidate_ids: z.array(z.string()),
});

export const LaunchEvalCandidateSnapshotInfoSchema = z.object({
    output_path: z.string(),
    exists: z.boolean(),
    provenance_filter: z.string(),
    scenario_count: z.number(),
    updated_at: z.string().nullable().optional(),
    scenario_ids: z.array(z.string()),
});

export const ExecApprovalSchema = z.object({
    id: z.string(),
    command: z.string(),
    cwd: z.string().nullable().optional(),
    created_at: z.string(),
    expires_at: z.string(),
    status: z.string(),
    decision: z.string().nullable().optional(),
    resolved_at: z.string().nullable().optional(),
    resolved_by: z.string().nullable().optional(),
});

export const ExecAllowlistSchema = z.object({
    id: z.number(),
    pattern: z.string(),
    cwd: z.string().nullable().optional(),
    created_at: z.string(),
    last_used_at: z.string().nullable().optional(),
    uses_count: z.number(),
});

export const ExecResultSchema = z.object({
    id: z.string(),
    command: z.string(),
    cwd: z.string().nullable().optional(),
    status: z.string(),
    output: z.string().nullable().optional(),
    error: z.string().nullable().optional(),
    created_at: z.string(),
    updated_at: z.string().nullable().optional(),
});

export const RoutineRunSchema = z.object({
    id: z.number(),
    routine_id: z.number(),
    routine_name: z.string().optional(),
    started_at: z.string(),
    finished_at: z.string().nullable().optional(),
    status: z.string(),
    error: z.string().nullable().optional(),
});

export const QualityScoreSchema = z.object({
    overall: z.number(),
    breakdown: z.record(z.string(), z.number()),
    issues: z.array(z.string()),
    strengths: z.array(z.string()),
    recommendation: z.string(),
    summary: z.string(),
});

export const QualityScoreRecordSchema = z.object({
    created_at: z.string(),
    score: QualityScoreSchema,
});

export const ConsistencyIssueSchema = z.object({
    path: z.string(),
    reason: z.string(),
    source: z.string(),
});

export const ConsistencyCheckSchema = z.object({
    ok: z.boolean(),
    issues: z.array(ConsistencyIssueSchema),
    backend_paths: z.array(z.string()),
    frontend_calls: z.array(z.object({
        path: z.string(),
        method: z.string().nullable().optional(),
        source: z.string(),
    })),
    summary: z.string(),
    template: z.string(),
});

export const SemanticIssueSchema = z.object({
    file: z.string(),
    reason: z.string(),
    severity: z.string(),
});

export const SemanticVerificationSchema = z.object({
    ok: z.boolean(),
    issues: z.array(SemanticIssueSchema),
    reason: z.string(),
    template: z.string(),
});

export const PerformanceMetricSchema = z.object({
    name: z.string(),
    value: z.number(),
    threshold: z.number(),
    ok: z.boolean(),
});

export const PerformanceVerificationSchema = z.object({
    ok: z.boolean(),
    metrics: z.array(PerformanceMetricSchema),
    reason: z.string(),
    template: z.string(),
});

export const VisualVerdictSchema = z.object({
    prompt: z.string(),
    ok: z.boolean(),
    response: z.string().nullable().optional(),
});

export const VisualVerifySchema = z.object({
    ok: z.boolean(),
    verdicts: z.array(VisualVerdictSchema),
});

export const RuntimeVerifySchema = z.object({
    backend_started: z.boolean(),
    backend_health: z.boolean(),
    backend_build_ok: z.boolean().nullable().optional(),
    frontend_started: z.boolean(),
    frontend_health: z.boolean(),
    frontend_build_ok: z.boolean().nullable().optional(),
    e2e_passed: z.boolean().nullable().optional(),
    issues: z.array(z.string()),
    logs: z.array(z.string()),
});

export const ReleaseBaselineSchema = z.object({
    created_at: z.string(),
    launch_ops: LaunchOpsMetricsSchema.optional(),
    nl_run_metrics: NLRunMetricsSchema.optional(),
    exec_approval_metrics: ExecApprovalMetricsSchema.optional(),
    recommendation_metrics: RecommendationMetricsSchema.optional(),
    recommendation_review_metrics: RecommendationReviewMetricsSchema.optional(),
    launch_eval: z.object({
        generated_at: z.string(),
        config_path: z.string().nullable().optional(),
        total: z.number(),
        passed: z.number(),
        failed: z.number(),
        failed_case_ids: z.array(z.string()),
    }).optional(),
    launch_eval_candidate_snapshot: LaunchEvalCandidateSnapshotInfoSchema.optional(),
    launch_eval_candidate_snapshot_refresh_error: z.string().nullable().optional(),
}).passthrough();

export const ReleaseGateSchema = z.object({
    ok: z.boolean(),
    regressions: z.array(z.string()),
    warnings: z.array(z.string()),
    current: ReleaseBaselineSchema.optional(),
    baseline: ReleaseBaselineSchema.optional(),
    template: z.string(),
}).passthrough();

export const ReleaseReadinessTrendSummarySchema = z.object({
    compared_runs: z.number(),
    stable_ready_streak: z.number(),
    status_regressed: z.boolean(),
    snapshot_delta: z.number(),
    launch_eval_pass_rate_delta_pct: z.number(),
    http_e2e_pass_rate_delta_pct: z.number(),
    http_e2e_regressed: z.boolean(),
    blocker_delta: z.number(),
    advisory_delta: z.number(),
    warnings: z.array(z.string()),
    summary: z.string(),
});

export const HttpE2EStepResultSchema = z.object({
    name: z.string(),
    ok: z.boolean(),
    detail: z.string(),
});

export const HttpE2EHistoryEntrySchema = z.object({
    generated_at: z.string(),
    ok: z.boolean(),
    passed: z.number(),
    total: z.number(),
    report_json_path: z.string(),
    report_markdown_path: z.string(),
});

export const HttpE2EReportSchema = z.object({
    generated_at: z.string(),
    workdir: z.string(),
    report_json_path: z.string(),
    report_markdown_path: z.string(),
    api_base_url: z.string(),
    runtime_db_path: z.string(),
    digest_stub_url: z.string(),
    ok: z.boolean(),
    passed: z.number(),
    total: z.number(),
    steps: z.array(HttpE2EStepResultSchema),
});

export const ReleaseReadinessSchema = z.object({
    generated_at: z.string(),
    workdir: z.string(),
    config_path: z.string(),
    report_json_path: z.string(),
    report_markdown_path: z.string(),
    archived_history_json_path: z.string().nullable().optional(),
    archived_history_markdown_path: z.string().nullable().optional(),
    archived_launch_eval_json_path: z.string().nullable().optional(),
    archived_launch_eval_markdown_path: z.string().nullable().optional(),
    history_trend: ReleaseReadinessTrendSummarySchema.nullable().optional(),
    baseline_saved: z.boolean(),
    status: z.string(),
    ready_for_launch: z.boolean(),
    blockers: z.array(z.string()),
    advisories: z.array(z.string()),
    http_e2e: HttpE2EReportSchema.nullable().optional(),
    http_e2e_load_error: z.string().nullable().optional(),
    candidate_snapshot: LaunchEvalCandidateSnapshotSchema,
    launch_eval: z.object({
        generated_at: z.string(),
        config_path: z.string().nullable().optional(),
        db_path: z.string(),
        report_json_path: z.string(),
        report_markdown_path: z.string(),
        total: z.number(),
        passed: z.number(),
        failed: z.number(),
    }).passthrough(),
    release_gate: ReleaseGateSchema,
}).passthrough();

export const ReleaseReadinessHistoryEntrySchema = z.object({
    generated_at: z.string(),
    status: z.string(),
    ready_for_launch: z.boolean(),
    candidate_snapshot_count: z.number(),
    launch_eval_passed: z.number(),
    launch_eval_total: z.number(),
    http_e2e_ok: z.boolean().nullable().optional(),
    http_e2e_passed: z.number().nullable().optional(),
    http_e2e_total: z.number().nullable().optional(),
    blocker_count: z.number(),
    advisory_count: z.number(),
    report_json_path: z.string(),
    report_markdown_path: z.string(),
});

export const VerificationRunSchema = z.object({
    id: z.number(),
    created_at: z.string(),
    kind: z.string(),
    mode: z.string().optional(),
    status: z.string().optional(),
    ok: z.boolean(),
    summary: z.string(),
    details: z.string().nullable().optional(),
});

export const AgentIntentResponseSchema = z.object({
    session_id: z.string(),
    intent: z.string(),
    confidence: z.number(),
    slots: z.record(z.string(), z.string()),
    missing_slots: z.array(z.string()),
    follow_up: z.string().nullable().optional(),
});

export const AgentPlanStepSchema = z.object({
    step_id: z.string(),
    step_type: z.string(),
    description: z.string(),
    data: z.unknown(),
});

export const AgentPlanResponseSchema = z.object({
    plan_id: z.string(),
    intent: z.string(),
    steps: z.array(AgentPlanStepSchema),
    missing_slots: z.array(z.string()),
});

export const ExecutionProfileSchema = z.enum(["strict", "test", "fast"]);

export const AgentExecuteResponseSchema = z.object({
    status: z.string(),
    logs: z.array(z.string()),
    approval: z
        .object({
            action: z.string(),
            message: z.string(),
            risk_level: z.string(),
            policy: z.string(),
        })
        .nullable()
        .optional(),
    manual_steps: z.array(z.string()).optional().default([]),
    resume_from: z.number().optional().nullable(),
    resume_token: z.string().optional().nullable(),
    run_id: z.string().nullable().optional(),
    planner_complete: z.boolean().optional().default(false),
    execution_complete: z.boolean().optional().default(false),
    business_complete: z.boolean().optional().default(false),
    completion_score: z
        .object({
            score: z.number(),
            label: z.string(),
            pass: z.boolean(),
            reasons: z.array(z.string()),
        })
        .nullable()
        .optional(),
    profile: ExecutionProfileSchema.optional().nullable(),
    collision_policy: z.string().optional().nullable(),
    stage_dod: z
        .array(
            z.object({
                stage: z.string(),
                key: z.string(),
                expected: z.string(),
                actual: z.string(),
                passed: z.boolean(),
                evidence: z.string().nullable().optional(),
            })
        )
        .optional()
        .default([]),
});

export const AgentVerifyResponseSchema = z.object({
    ok: z.boolean(),
    issues: z.array(z.string()),
});

export const AgentApproveResponseSchema = z.object({
    status: z.string(),
    requires_approval: z.boolean(),
    message: z.string(),
    risk_level: z.string(),
    policy: z.string(),
});

export const AgentGoalRunResponseSchema = z.object({
    run_id: z.string(),
    planner_complete: z.boolean(),
    execution_complete: z.boolean(),
    business_complete: z.boolean(),
    status: z.string(),
    summary: z.string().nullable().optional(),
});

export const ApprovalPolicySchema = z.object({
    policy_key: z.string(),
    decision: z.string(),
    updated_at: z.string(),
});

export const NLRunSchema = z.object({
    id: z.number(),
    created_at: z.string(),
    intent: z.string(),
    prompt: z.string(),
    status: z.string(),
    summary: z.string().nullable().optional(),
    details: z.string().nullable().optional(),
});

export const TaskRunSchema = z.object({
    run_id: z.string(),
    created_at: z.string(),
    finished_at: z.string().nullable().optional(),
    intent: z.string(),
    prompt: z.string(),
    planner_complete: z.boolean(),
    execution_complete: z.boolean(),
    business_complete: z.boolean(),
    status: z.string(),
    summary: z.string().nullable().optional(),
    details: z.string().nullable().optional(),
});

export const TaskStageRunSchema = z.object({
    id: z.number(),
    run_id: z.string(),
    stage_name: z.string(),
    stage_order: z.number(),
    status: z.string(),
    started_at: z.string(),
    finished_at: z.string(),
    details: z.string().nullable().optional(),
    retry_count: z.number().optional(),
    max_retries: z.number().optional(),
    next_retry_at: z.string().nullable().optional(),
});

export const TaskStageAssertionSchema = z.object({
    id: z.number(),
    run_id: z.string(),
    stage_name: z.string(),
    assertion_key: z.string(),
    expected: z.string(),
    actual: z.string(),
    passed: z.boolean(),
    evidence: z.string().nullable().optional(),
    created_at: z.string(),
});

export const TaskRunArtifactSchema = z.object({
    id: z.number(),
    run_id: z.string(),
    artifact_type: z.string(),
    artifact_key: z.string(),
    value: z.string(),
    metadata: z.string().nullable().optional(),
    created_at: z.string(),
});

export const AgentPreflightCheckSchema = z.object({
    key: z.string(),
    label: z.string(),
    ok: z.boolean(),
    expected: z.string().nullable().optional(),
    actual: z.string().nullable().optional(),
    message: z.string(),
});

export const AgentPreflightResponseSchema = z.object({
    ok: z.boolean(),
    checks: z.array(AgentPreflightCheckSchema),
    active_app: z.string().nullable().optional(),
    checked_at: z.string(),
});

export const AgentPreflightFixResponseSchema = z.object({
    ok: z.boolean(),
    action: z.string(),
    message: z.string(),
    active_app: z.string().nullable().optional(),
    fixed_at: z.string(),
    recorded: z.boolean().optional().default(false),
    run_id: z.string().nullable().optional(),
    stage_name: z.string().nullable().optional(),
});

export const AgentRecoveryEventResponseSchema = z.object({
    ok: z.boolean(),
    recorded: z.boolean(),
    run_id: z.string(),
    stage_name: z.string(),
    action_key: z.string(),
    status: z.string(),
    recorded_at: z.string(),
    reason: z.string().nullable().optional(),
});

export const ContextSelectionSchema = z.object({
    found: z.boolean(),
    text: z.string(),
    error: z.string().optional(),
});

export const ProjectScanSchema = z.object({
    project_type: z.string(),
    files: z.array(z.string()),
    key_files: z.record(z.string(), z.string()),
});

export const JudgmentSchema = z.object({
    status: z.string(),
    reasons: z.array(z.string()),
    no_progress: z.boolean(),
    project_hash: z.string().nullable().optional(),
    consecutive_no_progress: z.number(),
});

export const LockMetricsSchema = z.object({
    acquired: z.number(),
    bypassed: z.number(),
    blocked: z.number(),
    stale_recovered: z.number(),
    rejected: z.number(),
});

export const RuntimeInfoSchema = z.object({
    service: z.string(),
    version: z.string(),
    profile: z.string(),
    pid: z.number(),
    api_port: z.number(),
    allow_no_key: z.boolean(),
    started_at: z.string(),
    binary_path: z.string().nullable().optional(),
    current_dir: z.string().nullable().optional(),
});

export type SystemStatus = z.infer<typeof SystemStatusSchema>;
export type LogEntry = z.infer<typeof LogEntrySchema>;
export type Routine = z.infer<typeof RoutineSchema>;
export type Recommendation = z.infer<typeof RecommendationSchema>;
export type ApproveRecommendationResponse = z.infer<typeof ApproveRecommendationResponseSchema>;
export type RecommendationFeedbackResponse = z.infer<typeof RecommendationFeedbackResponseSchema>;
export type WorkflowProvisionOp = z.infer<typeof WorkflowProvisionOpSchema>;
export type RecommendationMetrics = z.infer<typeof RecommendationMetricsSchema>;
export type RecommendationReviewMetrics = z.infer<typeof RecommendationReviewMetricsSchema>;
export type RecommendationReviewEventRecord = z.infer<typeof RecommendationReviewEventRecordSchema>;
export type RequestMemoryRecord = z.infer<typeof RequestMemoryRecordSchema>;
export type ExecutionMemoryRecord = z.infer<typeof ExecutionMemoryRecordSchema>;
export type MemoryOpsMetrics = z.infer<typeof MemoryOpsMetricsSchema>;
export type MemoryAdminEventRecord = z.infer<typeof MemoryAdminEventRecordSchema>;
export type MemoryRecordsResponse = z.infer<typeof MemoryRecordsResponseSchema>;
export type MemoryAdminActionResponse = z.infer<typeof MemoryAdminActionResponseSchema>;
export type LaunchOpsRouteBreakdown = z.infer<typeof LaunchOpsRouteBreakdownSchema>;
export type LaunchOpsEventRecord = z.infer<typeof LaunchOpsEventRecordSchema>;
export type ChatRouteMeta = z.infer<typeof ChatRouteMetaSchema>;
export type ChatMessageResponse = z.infer<typeof ChatMessageResponseSchema>;
export type LaunchOpsMetrics = z.infer<typeof LaunchOpsMetricsSchema>;
export type LaunchOpsResponse = z.infer<typeof LaunchOpsResponseSchema>;
export type LaunchEvalCandidate = z.infer<typeof LaunchEvalCandidateSchema>;
export type LaunchEvalCandidateSnapshot = z.infer<typeof LaunchEvalCandidateSnapshotSchema>;
export type LaunchEvalCandidateSnapshotInfo = z.infer<typeof LaunchEvalCandidateSnapshotInfoSchema>;
export type ExecApproval = z.infer<typeof ExecApprovalSchema>;
export type ExecAllowlistEntry = z.infer<typeof ExecAllowlistSchema>;
export type ExecResult = z.infer<typeof ExecResultSchema>;
export type RoutineRun = z.infer<typeof RoutineRunSchema>;
export type QualityScore = z.infer<typeof QualityScoreSchema>;
export type QualityScoreRecord = z.infer<typeof QualityScoreRecordSchema>;
export type ConsistencyCheck = z.infer<typeof ConsistencyCheckSchema>;
export type SemanticVerification = z.infer<typeof SemanticVerificationSchema>;
export type PerformanceVerification = z.infer<typeof PerformanceVerificationSchema>;
export type VisualVerifyResult = z.infer<typeof VisualVerifySchema>;
export type RuntimeVerifyResult = z.infer<typeof RuntimeVerifySchema>;
export type ReleaseBaseline = z.infer<typeof ReleaseBaselineSchema>;
export type ReleaseGate = z.infer<typeof ReleaseGateSchema>;
export type ReleaseReadinessTrendSummary = z.infer<typeof ReleaseReadinessTrendSummarySchema>;
export type ReleaseReadiness = z.infer<typeof ReleaseReadinessSchema>;
export type ReleaseReadinessHistoryEntry = z.infer<typeof ReleaseReadinessHistoryEntrySchema>;
export type HttpE2EReport = z.infer<typeof HttpE2EReportSchema>;
export type HttpE2EHistoryEntry = z.infer<typeof HttpE2EHistoryEntrySchema>;
export type VerificationRun = z.infer<typeof VerificationRunSchema>;
export type AgentIntentResponse = z.infer<typeof AgentIntentResponseSchema>;
export type AgentPlanResponse = z.infer<typeof AgentPlanResponseSchema>;
export type ExecutionProfile = z.infer<typeof ExecutionProfileSchema>;
export type AgentExecuteResponse = z.infer<typeof AgentExecuteResponseSchema>;
export type AgentVerifyResponse = z.infer<typeof AgentVerifyResponseSchema>;
export type AgentApproveResponse = z.infer<typeof AgentApproveResponseSchema>;
export type AgentGoalRunResponse = z.infer<typeof AgentGoalRunResponseSchema>;
export type ApprovalPolicy = z.infer<typeof ApprovalPolicySchema>;
export type NLRunMetrics = z.infer<typeof NLRunMetricsSchema>;
export type NLRun = z.infer<typeof NLRunSchema>;
export type TaskRun = z.infer<typeof TaskRunSchema>;
export type TaskStageRun = z.infer<typeof TaskStageRunSchema>;
export type TaskStageAssertion = z.infer<typeof TaskStageAssertionSchema>;
export type TaskRunArtifact = z.infer<typeof TaskRunArtifactSchema>;
export type AgentPreflightCheck = z.infer<typeof AgentPreflightCheckSchema>;
export type AgentPreflightResponse = z.infer<typeof AgentPreflightResponseSchema>;
export type AgentPreflightFixResponse = z.infer<typeof AgentPreflightFixResponseSchema>;
export type AgentRecoveryEventResponse = z.infer<typeof AgentRecoveryEventResponseSchema>;
export type ContextSelection = z.infer<typeof ContextSelectionSchema>;
export type ProjectScan = z.infer<typeof ProjectScanSchema>;
export type Judgment = z.infer<typeof JudgmentSchema>;
export type LockMetrics = z.infer<typeof LockMetricsSchema>;
export type RuntimeInfo = z.infer<typeof RuntimeInfoSchema>;
