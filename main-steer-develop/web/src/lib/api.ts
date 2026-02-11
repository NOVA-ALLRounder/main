import axios from "axios";
import { invoke } from "@tauri-apps/api/core";
import {
    SystemStatusSchema,
    RoutineSchema,
    LogEntrySchema,
    RecommendationSchema,
    RecommendationMetricsSchema,
    ExecApprovalSchema,
    ExecAllowlistSchema,
    ExecResultSchema,
    RoutineRunSchema,
    QualityScoreRecordSchema,
    ConsistencyCheckSchema,
    SemanticVerificationSchema,
    PerformanceVerificationSchema,
    VisualVerifySchema,
    RuntimeVerifySchema,
    ReleaseBaselineSchema,
    ReleaseGateSchema,
    VerificationRunSchema,
    AgentIntentResponseSchema,
    AgentPlanResponseSchema,
    AgentExecuteResponseSchema,
    AgentVerifyResponseSchema,
    AgentApproveResponseSchema,
    ApprovalPolicySchema,
    NLRunMetricsSchema,
    NLRunSchema,
    ContextSelectionSchema,
    ProjectScanSchema,
    JudgmentSchema,
    type SystemStatus,
    type Routine,
    type LogEntry,
    type Recommendation,
    type RecommendationMetrics,
    type ExecApproval,
    type ExecAllowlistEntry,
    type ExecResult,
    type RoutineRun,
    type QualityScoreRecord,
    type ConsistencyCheck,
    type SemanticVerification,
    type PerformanceVerification,
    type VisualVerifyResult,
    type RuntimeVerifyResult,
    type ReleaseBaseline,
    type ReleaseGate,
    type VerificationRun,
    type AgentIntentResponse,
    type AgentPlanResponse,
    type AgentExecuteResponse,
    type AgentVerifyResponse,
    type AgentApproveResponse,
    type ApprovalPolicy,
    type NLRunMetrics,
    type NLRun,
    type ContextSelection,
    type ProjectScan,
    type Judgment,
    type QualityScore,
} from "./types";
import { z } from "zod";

export const API_BASE_URL =
    import.meta.env.VITE_API_BASE_URL?.replace(/\/$/, "") ??
    "http://127.0.0.1:5680/api";

const api = axios.create({
    baseURL: API_BASE_URL,
    timeout: 5000,
});

// Paranoid: Validate all responses with Zod
export async function fetchSystemStatus(): Promise<SystemStatus> {
    const { data } = await api.get("/status");
    return SystemStatusSchema.parse(data);
}

export async function fetchLogs(): Promise<LogEntry[]> {
    const { data } = await api.get("/logs");
    return z.array(LogEntrySchema).parse(data);
}

export async function fetchRoutines(): Promise<Routine[]> {
    const { data } = await api.get("/routines");
    return z.array(RoutineSchema).parse(data);
}

export async function createRoutine(name: string, cron: string, prompt: string): Promise<void> {
    await api.post("/routines", { name, cron_expression: cron, prompt });
}

export async function toggleRoutine(id: number, enabled: boolean): Promise<void> {
    await api.patch(`/routines/${id}`, { enabled });
}

// Workflows / Recommendations
export async function fetchRecommendations(): Promise<Recommendation[]> {
    const { data } = await api.get("/recommendations?status=all");
    return z.array(RecommendationSchema).parse(data);
}

export async function approveRecommendation(id: number): Promise<void> {
    await api.post(`/recommendations/${id}/approve`, undefined, { timeout: 20000 });
}

export async function rejectRecommendation(id: number): Promise<void> {
    await api.post(`/recommendations/${id}/reject`);
}

export async function laterRecommendation(id: number): Promise<void> {
    await api.post(`/recommendations/${id}/later`);
}

export async function restoreRecommendation(id: number): Promise<void> {
    await api.post(`/recommendations/${id}/restore`);
}

export async function fetchRecommendationMetrics(): Promise<RecommendationMetrics> {
    const { data } = await api.get("/recommendations/metrics");
    return RecommendationMetricsSchema.parse(data);
}

export async function fetchExecApprovals(status: string = "pending"): Promise<ExecApproval[]> {
    const { data } = await api.get(`/exec-approvals?status=${encodeURIComponent(status)}`);
    return z.array(ExecApprovalSchema).parse(data);
}

export async function approveExecApproval(id: string, resolvedBy?: string): Promise<void> {
    await api.post(`/exec-approvals/${id}/approve`, resolvedBy ? { resolved_by: resolvedBy } : undefined);
}

export async function rejectExecApproval(id: string, resolvedBy?: string): Promise<void> {
    await api.post(`/exec-approvals/${id}/reject`, resolvedBy ? { resolved_by: resolvedBy } : undefined);
}

export async function fetchExecAllowlist(limit: number = 100): Promise<ExecAllowlistEntry[]> {
    const { data } = await api.get(`/exec-allowlist?limit=${limit}`);
    return z.array(ExecAllowlistSchema).parse(data);
}

export async function addExecAllowlist(pattern: string, cwd?: string): Promise<void> {
    await api.post(`/exec-allowlist`, { pattern, cwd });
}

export async function removeExecAllowlist(id: number): Promise<void> {
    await api.delete(`/exec-allowlist/${id}`);
}

export async function fetchExecResults(limit: number = 100, status?: string): Promise<ExecResult[]> {
    const query = new URLSearchParams();
    query.set("limit", String(limit));
    if (status) query.set("status", status);
    const { data } = await api.get(`/exec-results?${query.toString()}`);
    return z.array(ExecResultSchema).parse(data);
}

export async function runExecResultsGuard(maxAgeSecs?: number, limit?: number): Promise<{ ok: boolean; scanned: number; timed_out: number; warnings: string[]; template: string }> {
    const payload: Record<string, number> = {};
    if (maxAgeSecs !== undefined) payload.max_age_secs = maxAgeSecs;
    if (limit !== undefined) payload.limit = limit;
    const { data } = await api.post(`/exec-results/guard`, payload);
    return data;
}

export async function fetchRoutineRuns(limit: number = 20): Promise<RoutineRun[]> {
    const { data } = await api.get(`/routine-runs?limit=${limit}`);
    return z.array(RoutineRunSchema).parse(data);
}

export async function fetchLatestQualityScore(): Promise<QualityScoreRecord | null> {
    const { data } = await api.get("/quality/latest");
    if (!data) return null;
    return QualityScoreRecordSchema.parse(data);
}

export async function calculateQualityScore(): Promise<QualityScoreRecord> {
    const { data } = await api.post("/quality/score");
    return QualityScoreRecordSchema.parse(data);
}

export async function fetchConsistencyCheck(): Promise<ConsistencyCheck> {
    const { data } = await api.post("/verify/consistency", {});
    return ConsistencyCheckSchema.parse(data);
}

export async function fetchSemanticVerification(): Promise<SemanticVerification> {
    const { data } = await api.post("/verify/semantic", {});
    return SemanticVerificationSchema.parse(data);
}

export type RuntimeVerifyOptions = {
    workdir?: string;
    run_backend?: boolean;
    run_frontend?: boolean;
    run_e2e?: boolean;
    run_build_checks?: boolean;
    backend_port?: number;
    frontend_port?: number;
    backend_health_path?: string;
};

export async function runRuntimeVerification(options: RuntimeVerifyOptions = {}): Promise<RuntimeVerifyResult> {
    const { data } = await api.post("/verify/runtime", options);
    return RuntimeVerifySchema.parse(data);
}

export type PerformanceVerifyOptions = {
    workdir?: string;
    max_files?: number;
};

export async function runPerformanceVerification(options: PerformanceVerifyOptions = {}): Promise<PerformanceVerification> {
    const { data } = await api.post("/verify/performance", options);
    return PerformanceVerificationSchema.parse(data);
}

export async function runVisualVerification(prompts: string[]): Promise<VisualVerifyResult> {
    const { data } = await api.post("/verify/visual", { prompts });
    return VisualVerifySchema.parse(data);
}

// Beta Features
export async function fetchSelectionContext(): Promise<ContextSelection> {
    const { data } = await api.get("/context/selection");
    return ContextSelectionSchema.parse(data);
}

export async function scanProject(maxFiles?: number, workdir?: string): Promise<ProjectScan> {
    const query = new URLSearchParams();
    if (maxFiles) query.set("max_files", String(maxFiles));
    if (workdir) query.set("workdir", workdir);
    const { data } = await api.get(`/project/scan?${query.toString()}`);
    return ProjectScanSchema.parse(data);
}

export async function runJudgment(
    workdir?: string,
    maxFiles?: number,
    runtime?: RuntimeVerifyResult,
    quality?: QualityScore,
    semantic?: SemanticVerification,
    performance?: PerformanceVerification,
): Promise<Judgment> {
    const payload = {
        workdir,
        max_files: maxFiles,
        runtime,
        quality,
        semantic,
        performance,
    };
    const { data } = await api.post("/judgment", payload);
    return JudgmentSchema.parse(data);
}

export type ReleaseGateOverrides = {
    perf_regression_pct?: number;
    quality_drop?: number;
};

export async function fetchReleaseGate(overrides?: ReleaseGateOverrides): Promise<ReleaseGate> {
    const payload: Record<string, number> = {};
    if (overrides?.perf_regression_pct !== undefined) {
        payload.perf_regression_pct = overrides.perf_regression_pct;
    }
    if (overrides?.quality_drop !== undefined) {
        payload.quality_drop = overrides.quality_drop;
    }
    const { data } = await api.post("/release/gate", payload);
    return ReleaseGateSchema.parse(data);
}

export async function setReleaseBaseline(options: PerformanceVerifyOptions = {}): Promise<ReleaseBaseline> {
    const { data } = await api.post("/release/baseline", options);
    return ReleaseBaselineSchema.parse(data);
}

export async function fetchVerificationRuns(limit: number = 20): Promise<VerificationRun[]> {
    const { data } = await api.get(`/verify/runs?limit=${limit}`);
    return z.array(VerificationRunSchema).parse(data);
}

export async function agentIntent(text: string): Promise<AgentIntentResponse> {
    const { data } = await api.post("/agent/intent", { text });
    return AgentIntentResponseSchema.parse(data);
}

export async function agentPlan(
    sessionId: string,
    slots?: Record<string, string>
): Promise<AgentPlanResponse> {
    const { data } = await api.post("/agent/plan", { session_id: sessionId, slots });
    return AgentPlanResponseSchema.parse(data);
}

export async function agentExecute(planId: string): Promise<AgentExecuteResponse> {
    const { data } = await api.post("/agent/execute", { plan_id: planId });
    return AgentExecuteResponseSchema.parse(data);
}

export async function agentVerify(planId: string): Promise<AgentVerifyResponse> {
    const { data } = await api.post("/agent/verify", { plan_id: planId });
    return AgentVerifyResponseSchema.parse(data);
}

export async function agentApprove(
    planId: string,
    action: string,
    decision?: string
): Promise<AgentApproveResponse> {
    const { data } = await api.post("/agent/approve", { plan_id: planId, action, decision });
    return AgentApproveResponseSchema.parse(data);
}

export async function fetchNlRuns(limit: number = 20): Promise<NLRun[]> {
    const { data } = await api.get(`/agent/nl-runs?limit=${limit}`);
    return z.array(NLRunSchema).parse(data);
}

export async function fetchNlRunMetrics(limit: number = 50): Promise<NLRunMetrics> {
    const { data } = await api.get(`/agent/nl-metrics?limit=${limit}`);
    return NLRunMetricsSchema.parse(data);
}

export async function fetchApprovalPolicies(limit: number = 20): Promise<ApprovalPolicy[]> {
    const { data } = await api.get(`/agent/approval-policies?limit=${limit}`);
    return z.array(ApprovalPolicySchema).parse(data);
}

export async function removeApprovalPolicy(policyKey: string): Promise<void> {
    await api.delete(`/agent/approval-policies/${encodeURIComponent(policyKey)}`);
}

export async function sendFeedback(
    goal: string,
    feedback: string,
    historySummary?: string
): Promise<{ action: string; new_goal?: string; message: string }> {
    const { data } = await api.post("/agent/feedback", {
        goal,
        feedback,
        history_summary: historySummary || undefined,
    });
    return data;
}

export async function executeGoal(goal: string): Promise<{ status: string; message: string }> {
    const { data } = await api.post("/agent/goal", { goal });
    return data;
}

export async function fetchCurrentGoal(): Promise<string> {
    const { data } = await api.get("/agent/goal/current");
    return typeof data?.goal === "string" ? data.goal : "";
}

export async function getHealth(): Promise<unknown> {
    const { data } = await api.get("/system/health");
    return data;
}

export async function getVersion(): Promise<{ core_version: string; build_profile: string; git_sha?: string | null }> {
    const { data } = await api.get("/version");
    return data;
}

export type RuntimeMode = {
    mode: "observe" | "copilot" | "autopilot";
    emergency_stop: boolean;
    allow_automation: boolean;
};

export type SystemPreflight = {
    ok: boolean;
    api_port: number;
    api_reachable: boolean;
    operation_mode: "observe" | "copilot" | "autopilot";
    emergency_stop: boolean;
    allow_automation: boolean;
    env_present: boolean;
    steer_home_writable: boolean;
    release_dir_writable: boolean;
    gmail_credentials_set: boolean;
    notion_ready: boolean;
    notes: string[];
};

const normalizeSystemBase = (base: string): string[] => {
    const trimmed = base.replace(/\/$/, "");
    return trimmed.endsWith("/api") ? [trimmed] : [trimmed, `${trimmed}/api`];
};

async function getSystemEndpointWithFallback<T>(path: string): Promise<T> {
    const bases = Array.from(
        new Set([
            ...normalizeSystemBase(API_BASE_URL),
            "http://127.0.0.1:5680/api",
            "http://localhost:5680/api",
        ])
    );

    let lastError: unknown;
    for (const base of bases) {
        try {
            const { data } = await axios.get<T>(`${base}${path}`, {
                timeout: 6000,
            });
            return data;
        } catch (err) {
            lastError = err;
        }
    }
    throw lastError ?? new Error(`System endpoint failed: ${path}`);
}

export async function getRuntimeMode(): Promise<RuntimeMode> {
    return await getSystemEndpointWithFallback<RuntimeMode>("/system/mode");
}

export async function setRuntimeMode(mode: RuntimeMode["mode"]): Promise<RuntimeMode> {
    const { data } = await api.post("/system/mode", { mode });
    return data as RuntimeMode;
}

export async function setEmergencyStop(enabled: boolean): Promise<RuntimeMode> {
    const { data } = await api.post("/system/emergency-stop", { enabled });
    return data as RuntimeMode;
}

export async function getSystemPreflight(): Promise<SystemPreflight> {
    try {
        return await getSystemEndpointWithFallback<SystemPreflight>("/system/preflight");
    } catch {
        type HealthFallback = {
            api_port?: number;
            api_reachable?: boolean;
            gmail_credentials_set?: boolean;
            notion_ready?: boolean;
        };
        const [mode, health] = await Promise.all([
            getSystemEndpointWithFallback<RuntimeMode>("/system/mode").catch(() => ({
                mode: "autopilot" as const,
                emergency_stop: false,
                allow_automation: false,
            })),
            getSystemEndpointWithFallback<HealthFallback>("/system/health").catch(
                () => ({}) as HealthFallback
            ),
        ]);

        const gmailReady = Boolean(health.gmail_credentials_set);
        const notionReady = Boolean(health.notion_ready);
        const apiReachable = health.api_reachable ?? false;

        return {
            ok: apiReachable,
            api_port: health.api_port ?? 5680,
            api_reachable: apiReachable,
            operation_mode: mode.mode,
            emergency_stop: mode.emergency_stop,
            allow_automation: mode.allow_automation,
            env_present: true,
            steer_home_writable: true,
            release_dir_writable: true,
            gmail_credentials_set: gmailReady,
            notion_ready: notionReady,
            notes: apiReachable ? ["preflight endpoint fallback via health/mode"] : ["system endpoints unreachable"],
        };
    }
}

export async function analyzePatterns(): Promise<string[]> {
    const { data } = await api.post("/patterns/analyze");
    return z.array(z.string()).parse(data);
}

export async function sendChatMessage(message: string): Promise<{ response: string; command?: string; error_code?: string }> {
    const extractErrorText = (err: unknown): string => {
        if (!axios.isAxiosError(err)) return "Unknown error";
        const payload = err.response?.data;
        if (typeof payload === "string" && payload.trim()) return payload;
        if (payload && typeof payload === "object") {
            const obj = payload as Record<string, unknown>;
            const msg = obj.message ?? obj.error ?? obj.detail;
            if (typeof msg === "string" && msg.trim()) return msg;
        }
        if (err.message?.trim()) return err.message;
        return "Request failed";
    };

    const toUserFacingError = (text: string): string => {
        const lower = text.toLowerCase();
        if (lower.includes("rate limit") || lower.includes("rate_limit_exceeded") || lower.includes("429")) {
            return "API rate limit reached (429). Please retry shortly.";
        }
        if (lower.includes("credentials.json") || lower.includes("unauthorized") || lower.includes("api key")) {
            return `Credential/configuration error: ${text}`;
        }
        if (lower.includes("timeout")) {
            return "Request timed out. Please retry.";
        }
        return `Server error: ${text}`;
    };

    const withErrorCode = (payload: { response: string; command?: string }) => {
        const error_code = payload.command?.startsWith("error_")
            ? payload.command.replace(/^error_/, "").toUpperCase()
            : undefined;
        return { ...payload, error_code };
    };

    const normalizeBase = (base: string): string[] => {
        const trimmed = base.replace(/\/$/, "");
        return trimmed.endsWith("/api") ? [trimmed] : [trimmed, `${trimmed}/api`];
    };

    const bases = Array.from(
        new Set([
            ...normalizeBase(API_BASE_URL),
            "http://127.0.0.1:5680",
            "http://127.0.0.1:5680/api",
            "http://localhost:5680",
            "http://localhost:5680/api",
        ])
    );

    let lastError: unknown;
    for (const base of bases) {
        try {
            const { data } = await axios.post(`${base}/chat`, { message }, {
                timeout: 20000,
            });
            return withErrorCode(data as { response: string; command?: string });
        } catch (e) {
            lastError = e;
        }
    }

    try {
        const { data } = await api.post("/chat", { message });
        return withErrorCode(data as { response: string; command?: string });
    } catch (e) {
        try {
            const proxy = await invoke<{ response: string; command?: string }>("proxy_chat", { message });
            if (proxy?.response) {
                return withErrorCode(proxy);
            }
        } catch (proxyErr) {
            console.error("Tauri proxy_chat fallback failed:", proxyErr);
        }

        if (axios.isAxiosError(e)) {
            const primary = extractErrorText(e);
            const fallback = lastError ? extractErrorText(lastError) : "";
            const merged = fallback && fallback !== primary ? `${primary} | ${fallback}` : primary;
            console.error("Chat Error:", merged);
            return { response: toUserFacingError(merged), error_code: "NETWORK_ERROR" };
        }
        return { response: "Unknown Error", error_code: "UNKNOWN_ERROR" };
    }
}

export async function sendJarvisCommand(text: string): Promise<{ success: boolean; message: string; data?: unknown }> {
    try {
        const { data } = await api.post("/jarvis", { text });
        return data;
    } catch (e) {
        if (axios.isAxiosError(e)) {
            const detail = typeof e.response?.data === "string"
                ? e.response.data
                : (e.response?.data as { message?: string; error?: string } | undefined)?.message
                    ?? (e.response?.data as { message?: string; error?: string } | undefined)?.error
                    ?? e.message;
            console.error("JARVIS Error:", detail);
            return { success: false, message: `Network or Server Error: ${detail}` };
        }
        return { success: false, message: "Unknown Error" };
    }
}

export interface JarvisMessage {
    text: string;
    session_key?: string;
    metadata?: Record<string, string>;
}

export interface JarvisResponse {
    success: boolean;
    message: string;
    data?: unknown;
}

export interface JarvisSkill {
    name: string;
    description: string;
    version: string;
    actions: string[];
    eligible: boolean;
    reason?: string;
    platform?: string;
    tags: string[];
}

export interface JarvisSession {
    session_key: string;
    state: string;
    created_at: string | number | null;
    last_activity: string | number | null;
}

export async function sendJarvisWebMessage(message: JarvisMessage): Promise<JarvisResponse> {
    try {
        const { data } = await api.post("/jarvis/channels/web/message", message);
        return data;
    } catch (e) {
        if (axios.isAxiosError(e)) {
            console.error("JARVIS Web Channel Error:", e.response?.data || e.message);
            return { success: false, message: "Failed to send message through web channel" };
        }
        return { success: false, message: "Unknown Error" };
    }
}

export async function fetchJarvisSkills(): Promise<JarvisSkill[]> {
    try {
        const { data } = await api.get("/jarvis/skills");
        return data.skills || [];
    } catch (e) {
        console.error("Failed to fetch JARVIS skills:", e);
        return [];
    }
}

export async function executeJarvisSkill(
    skillName: string,
    action: string,
    params: Record<string, unknown>
): Promise<JarvisResponse> {
    try {
        const { data } = await api.post(`/jarvis/skills/${skillName}/execute`, {
            action,
            params
        });
        return data;
    } catch (e) {
        if (axios.isAxiosError(e)) {
            console.error("JARVIS Skill Execution Error:", e.response?.data || e.message);
            return { success: false, message: `Failed to execute ${skillName}.${action}` };
        }
        return { success: false, message: "Unknown Error" };
    }
}

export async function fetchJarvisSessions(): Promise<JarvisSession[]> {
    try {
        const { data } = await api.get("/jarvis/sessions");
        return data.sessions || [];
    } catch (e) {
        console.error("Failed to fetch JARVIS sessions:", e);
        return [];
    }
}

export async function startJarvisAutonomous(): Promise<JarvisResponse> {
    try {
        const { data } = await api.post("/jarvis/autonomous/start");
        return data;
    } catch (e) {
        if (axios.isAxiosError(e)) {
            console.error("JARVIS Autonomous Start Error:", e.response?.data || e.message);
            return { success: false, message: "Failed to start autonomous mode" };
        }
        return { success: false, message: "Unknown Error" };
    }
}

