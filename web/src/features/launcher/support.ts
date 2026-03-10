import axios from "axios";

import type { ExecutionProfile, Recommendation, TaskStageAssertion, TaskStageRun, TaskRunArtifact } from "@/lib/types";

export type LauncherResult = {
    type: "response" | "error";
    content: string;
};

export type ApprovalContext = {
    planId: string;
    action: string;
    message: string;
    riskLevel: string;
    policy: string;
};

export type RunPhase =
    | "idle"
    | "running"
    | "retrying"
    | "approval_required"
    | "manual_required"
    | "completed"
    | "failed";

export type ExecutionSnapshot = {
    status: string;
    runId: string | null;
    resumeToken: string | null;
    plannerComplete: boolean;
    executionComplete: boolean;
    businessComplete: boolean;
    verifyOk: boolean;
    verifyIssues: string[];
    completionScore: {
        score: number;
        label: string;
        pass: boolean;
        reasons: string[];
    } | null;
};

export type ComposerMode = "nl" | "chat" | "program";

export type QuickProgramAction = {
    key: string;
    label: string;
    prompt: string;
};

export type ManualResumeChecklist = {
    focusReady: boolean;
    manualStepDone: boolean;
    handsOffReady: boolean;
};

export type ExecutionProfileOption = {
    value: ExecutionProfile;
    label: string;
    hint: string;
};

export type StageTraceItem = {
    stage: TaskStageRun;
    assertions: TaskStageAssertion[];
    failed: TaskStageAssertion[];
    assertionTotal: number;
    assertionFailed: number;
};

export type RecoveryAction = {
    key: string;
    label: string;
    description: string;
    kind: "preflight_fix" | "artifact" | "guided_resume";
    fixAction?: string;
    path?: string;
    assertionKey?: string;
};

export type DodHistoryItem = {
    runId: string;
    createdAt: string;
    status: string;
    plannerComplete: boolean;
    executionComplete: boolean;
    businessComplete: boolean;
    assertionTotal: number;
    assertionFailed: number;
};

export type DodFailureTopItem = {
    key: string;
    count: number;
    sampleActual: string;
};

export type ArtifactGroupItem = {
    type: string;
    items: TaskRunArtifact[];
};

export type ArtifactSortMode = "newest" | "key" | "failed_first";

export type PendingDispatch = {
    prompt: string;
    executeAtMs: number;
};

export type ProvisioningUiState = {
    phase: "provisioning" | "failed";
    opId: number | null;
    detail?: string;
    updatedAt: number;
};

export type CoreBinaryKind = "bundle" | "workspace" | "custom" | "unknown";

export type ProfileRecommendation = {
    profile: ExecutionProfile;
    reason: string;
};

export type RunScore = {
    score: number;
    label: string;
    pass: boolean;
};

export type HudMeta = {
    label: string;
    chip: string;
    dot: string;
};

export type SuggestionRow = {
    rec: Recommendation;
    idx: number;
    isSelected: boolean;
    workflowUrl: string | null;
    uiProvision?: ProvisioningUiState;
    statusLabel: string;
    statusToneClass: string;
    updatedAtLabel: string;
    canRetryProvision: boolean;
    approvalBlockedReason: string | null;
    canApprove: boolean;
    n8nTarget: string | null;
};

const ABSOLUTE_ARTIFACT_RE = /\/(?:Users|tmp|var|private)\/[^\s"'|;,)]+/g;
const RELATIVE_ARTIFACT_RE = /\b(?:scenario_results|logs)\/[^\s"'|;,)]+/g;

const normalizeArtifactPathToken = (token: string) =>
    token.trim().replace(/[)\],.;:]+$/, "");

export const extractArtifactPaths = (...inputs: Array<string | null | undefined>): string[] => {
    const unique = new Set<string>();
    for (const input of inputs) {
        if (!input) continue;
        const abs = input.match(ABSOLUTE_ARTIFACT_RE) ?? [];
        const rel = input.match(RELATIVE_ARTIFACT_RE) ?? [];
        [...abs, ...rel]
            .map(normalizeArtifactPathToken)
            .filter((x) => x.length > 0)
            .forEach((x) => unique.add(x));
    }
    return Array.from(unique).slice(0, 3);
};

export const artifactPathLabel = (path: string): string => {
    const normalized = path.replace(/\\/g, "/");
    const leaf = normalized.split("/").pop() ?? normalized;
    return leaf.length > 48 ? `${leaf.slice(0, 45)}...` : leaf;
};

export const normalizeDispatchPrompt = (value: string): string =>
    value.trim().replace(/\s+/g, " ").toLowerCase();

export const QUICK_PROGRAM_ACTIONS: QuickProgramAction[] = [
    {
        key: "calendar_front",
        label: "캘린더 열기",
        prompt: "Calendar를 열고 전면으로 가져오세요.",
    },
    {
        key: "notes_new",
        label: "새 메모",
        prompt: "Notes를 열고 새 메모를 만든 뒤 오늘 할 일 3줄을 입력하세요.",
    },
    {
        key: "mail_draft",
        label: "메일 초안",
        prompt: "Mail을 열고 새 이메일 초안을 만들고 제목과 본문을 작성하세요.",
    },
    {
        key: "finder_downloads",
        label: "다운로드 보기",
        prompt: "Finder를 열고 Downloads 폴더로 이동하세요.",
    },
    {
        key: "scenario_1",
        label: "시나리오 1",
        prompt: "Calendar를 열고 Notes를 열어 새 메모를 작성하고 Mail로 보낼 초안을 만드세요.",
    },
];

export const summarizeGoalRunStatus = (params: {
    mode: ComposerMode;
    status: string;
    runId: string;
    plannerComplete: boolean;
    executionComplete: boolean;
    businessComplete: boolean;
    summary?: string | null;
}) => {
    const {
        mode,
        status,
        runId,
        plannerComplete,
        executionComplete,
        businessComplete,
        summary,
    } = params;
    const lines = [
        `**Mode**: goal-run (${mode})`,
        `**Status**: ${status}`,
        `**Run ID**: ${runId}`,
        `**Planner Complete**: ${plannerComplete ? "yes" : "no"}`,
        `**Execution Complete**: ${executionComplete ? "yes" : "no"}`,
        `**Business Complete**: ${businessComplete ? "yes" : "no"}`,
        summary ? `**Summary**: ${summary}` : "",
    ];
    return lines.filter(Boolean).join("\n");
};

export const IN_PROGRESS_RUN_STATUSES = new Set([
    "accepted",
    "busy",
    "queued",
    "running",
    "started",
    "retrying",
    "business_incomplete",
]);

export const toGoalRunResultType = (
    status: string,
    businessComplete: boolean
): LauncherResult["type"] => {
    const statusLower = status.toLowerCase();
    const inProgress = IN_PROGRESS_RUN_STATUSES.has(statusLower);
    if (
        businessComplete ||
        inProgress ||
        statusLower === "approval_required" ||
        statusLower === "manual_required" ||
        statusLower === "business_completed"
    ) {
        return "response";
    }
    return "error";
};

export const preflightPermissionHint = (message: string): string | null => {
    const lower = message.toLowerCase();
    const likelyAutomationAuth =
        lower.includes("not authorized") ||
        lower.includes("permission denied") ||
        lower.includes("osstatus error -1002") ||
        lower.includes("(-1002)") ||
        lower.includes("osascript");
    if (!likelyAutomationAuth) return null;
    return [
        "권한 안내:",
        "- 시스템 설정 > 개인정보 보호 및 보안 > 손쉬운 사용: `AllvIa`, `Terminal` 허용",
        "- 시스템 설정 > 개인정보 보호 및 보안 > 자동화: `AllvIa`가 `Finder`/대상 앱 제어 허용",
        "- 시스템 설정 > 개인정보 보호 및 보안 > 화면 기록: `AllvIa`, `Terminal` 허용 후 앱 재시작",
    ].join("\n");
};

export const QUICK_NL_SUGGESTIONS = [
    "오늘 받은 메일 5개 요약해줘",
    "노트에서 최근 TODO 정리해줘",
    "복잡 시나리오 1번 실행해줘",
    "텔레그램으로 실행 결과 요약 보내줘",
];

export const QUICK_CHAT_SUGGESTIONS = [
    "안녕? 오늘 우선순위 3개만 정리해줘",
    "방금 실행 결과를 한 줄로 설명해줘",
    "지금 가장 위험한 문제 하나만 알려줘",
    "다음에 뭘 하면 좋을지 3단계로 말해줘",
];

export const EXECUTION_PROFILE_OPTIONS: ExecutionProfileOption[] = [
    { value: "strict", label: "정확", hint: "충돌 시 중단" },
    { value: "test", label: "테스트", hint: "충돌 시 일시정지" },
    { value: "fast", label: "빠름", hint: "충돌 무시" },
];

export const profileLabel = (profile: ExecutionProfile): string => {
    const found = EXECUTION_PROFILE_OPTIONS.find((option) => option.value === profile);
    return found?.label ?? profile;
};

export const isGoalRunEndpointUnavailable = (error: unknown): boolean => {
    if (!axios.isAxiosError(error)) return false;
    const status = error.response?.status;
    if (status === 404 || status === 405 || status === 501) return true;
    const responseData = error.response?.data as { error?: unknown } | undefined;
    const text = [
        error.message ?? "",
        String(responseData ?? ""),
        String(responseData?.error ?? ""),
    ]
        .join(" ")
        .toLowerCase();
    return (
        text.includes("goal/run") ||
        text.includes("goal-run") ||
        text.includes("not found") ||
        text.includes("no route")
    );
};

export const isNlChatFallbackEnabled = (): boolean => {
    if (typeof import.meta === "undefined") return false;
    return import.meta.env.VITE_ENABLE_NL_CHAT_FALLBACK !== "0";
};

export const isLegacyGoalFallbackEnabled = (): boolean => {
    if (typeof import.meta === "undefined") return false;
    return import.meta.env.VITE_ENABLE_LEGACY_GOAL_FALLBACK === "1";
};

export const shouldTryNlChatFallback = (error: unknown): boolean => {
    if (!isNlChatFallbackEnabled()) return false;
    if (!axios.isAxiosError(error)) return false;
    const status = error.response?.status;
    if (status == null) return true;
    if (status === 404 || status === 405 || status === 501) return true;
    const payload = error.response?.data as
        | { error?: unknown; message?: unknown; detail?: unknown }
        | undefined;
    const text = [
        String(payload?.error ?? ""),
        String(payload?.message ?? ""),
        String(payload?.detail ?? ""),
        error.message ?? "",
    ]
        .join(" ")
        .toLowerCase();
    return (
        (text.includes("goal/run") || text.includes("goal-run")) &&
        (text.includes("not found") || text.includes("no route"))
    );
};

export const TERMINAL_RUN_STATUSES = new Set([
    "business_completed",
    "business_failed",
    "failed",
    "error",
    "blocked",
    "approval_required",
    "manual_required",
    "completed",
    "success",
]);

const normalizeLoopbackTarget = (raw: string): string => {
    const trimmed = raw.trim();
    if (!trimmed) return trimmed;
    try {
        const parsed = new URL(trimmed);
        if (
            parsed.hostname === "127.0.0.1" ||
            parsed.hostname === "0.0.0.0" ||
            parsed.hostname === "::1"
        ) {
            parsed.hostname = "localhost";
        }
        return parsed.toString().replace(/\/+$/, "");
    } catch {
        return trimmed.replace(/\/+$/, "");
    }
};

export const N8N_EDITOR_BASE_URL = (() => {
    if (typeof import.meta !== "undefined") {
        const raw = import.meta.env.VITE_N8N_EDITOR_URL as string | undefined;
        const trimmed = raw?.trim();
        if (trimmed) return normalizeLoopbackTarget(trimmed);
    }
    return normalizeLoopbackTarget("http://localhost:5678");
})();

export const resolveRecommendationWorkflowUrl = (
    rec?: Pick<Recommendation, "workflow_url" | "workflow_id"> | null,
    workflowIdFallback?: string | null
): string | null => {
    const explicitUrl = rec?.workflow_url?.trim();
    if (explicitUrl) return normalizeLoopbackTarget(explicitUrl);
    const workflowId = rec?.workflow_id?.trim() || workflowIdFallback?.trim();
    if (!workflowId) return null;
    if (workflowId.startsWith("provisioning:")) return null;
    return `${N8N_EDITOR_BASE_URL}/workflow/${encodeURIComponent(workflowId)}`;
};

export const APPROVAL_MONITOR_MAX_ATTEMPTS = 200;
export const APPROVAL_MONITOR_INTERVAL_MS = 1800;
export const APPROVAL_MONITOR_PENDING_NOTICE_ATTEMPT = 10;
export const CHAT_TRANSCRIPT_MAX_ITEMS = 14;

export const formatRecommendationStatusLabel = (
    rec: Recommendation,
    uiState?: ProvisioningUiState
): string => {
    if (uiState?.phase === "provisioning") {
        return "Provisioning (생성 중...)";
    }
    if (uiState?.phase === "failed") {
        return "Failed (재시도)";
    }
    if (rec.status === "failed") {
        return "Failed (재시도)";
    }
    if (rec.workflow_id?.startsWith("provisioning:")) {
        return "Provisioning (생성 중...)";
    }
    return rec.status;
};

export const recommendationStatusToneClass = (
    rec: Recommendation,
    uiState?: ProvisioningUiState
): string => {
    const label = formatRecommendationStatusLabel(rec, uiState).toLowerCase();
    if (label.includes("provisioning")) {
        return "border-sky-400/40 bg-sky-500/15 text-sky-200";
    }
    if (label.includes("failed")) {
        return "border-rose-400/40 bg-rose-500/15 text-rose-200";
    }
    if (label.includes("approved") || label.includes("success")) {
        return "border-emerald-400/40 bg-emerald-500/15 text-emerald-200";
    }
    return "border-white/20 bg-white/5 text-gray-300";
};

export const formatProvisionUpdatedAt = (updatedAt?: number): string => {
    if (!updatedAt) return "";
    const deltaMs = Date.now() - updatedAt;
    if (deltaMs < 10_000) return "just now";
    if (deltaMs < 60_000) return `${Math.floor(deltaMs / 1000)}s ago`;
    if (deltaMs < 3_600_000) return `${Math.floor(deltaMs / 60_000)}m ago`;
    return `${Math.floor(deltaMs / 3_600_000)}h ago`;
};

export const appendChatTranscript = (
    prev: LauncherResult[],
    prompt: string,
    reply: string,
    isError: boolean
): LauncherResult[] => {
    const next: LauncherResult[] = [
        ...prev,
        { type: "response", content: `**🙋 요청**\n${prompt}` },
        {
            type: isError ? "error" : "response",
            content: `${isError ? "**⚠️ 오류**" : "**🤖 답변**"}\n${reply}`,
        },
    ];
    return next.slice(-CHAT_TRANSCRIPT_MAX_ITEMS);
};

export const classifyCoreBinary = (binaryPath?: string | null): CoreBinaryKind => {
    const normalized = binaryPath?.trim() ?? "";
    if (!normalized) return "unknown";
    if (
        normalized.includes("/Applications/AllvIa.app/Contents/MacOS/core") ||
        normalized.includes("/Applications/Steer OS.app/Contents/MacOS/core")
    ) {
        return "bundle";
    }
    if (normalized.includes("/local-os-agent/")) return "workspace";
    return "custom";
};
