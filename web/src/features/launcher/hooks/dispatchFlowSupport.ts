import type { Dispatch, SetStateAction } from "react";

import {
    agentExecute,
    agentGoalRun,
    agentIntent,
    agentPlan,
    agentVerify,
    executeGoal,
    sendChatMessage,
} from "@/lib/api";
import type { ExecutionProfile } from "@/lib/types";
import {
    IN_PROGRESS_RUN_STATUSES,
    appendChatTranscript,
    isGoalRunEndpointUnavailable,
    isLegacyGoalFallbackEnabled,
    shouldTryNlChatFallback,
    summarizeGoalRunStatus,
    toGoalRunResultType,
    type ApprovalContext,
    type ComposerMode,
    type ExecutionSnapshot,
    type LauncherResult,
    type RunPhase,
} from "@/features/launcher/support";

type SetResults = Dispatch<SetStateAction<LauncherResult[]>>;
type SetString = Dispatch<SetStateAction<string | null>>;
type SetRunPhase = Dispatch<SetStateAction<RunPhase>>;

type SharedExecutionHandlers = {
    updateExecutionState: (snapshot: ExecutionSnapshot) => void;
    loadRunDiagnostics: (runId?: string | null) => Promise<void>;
    loadDodHistory: () => Promise<void>;
    triggerSuccess: () => void;
    setResults: SetResults;
    setInput: Dispatch<SetStateAction<string>>;
    setLastPlanId: SetString;
    setLastStatus: SetString;
    setRunPhase: SetRunPhase;
};

type ChatDispatchParams = {
    prompt: string;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
    setLoading: Dispatch<SetStateAction<boolean>>;
    setRunPhase: SetRunPhase;
    setPendingApproval: Dispatch<SetStateAction<ApprovalContext | null>>;
    setRecoveryActionBusyKey: Dispatch<SetStateAction<string | null>>;
    setResults: SetResults;
    setInput: Dispatch<SetStateAction<string>>;
    triggerSuccess: () => void;
    triggerError: () => void;
};

type GoalRunDispatchParams = SharedExecutionHandlers & {
    prompt: string;
    composerMode: ComposerMode;
    goalRunAvailable: boolean | null;
    setGoalRunAvailable: Dispatch<SetStateAction<boolean | null>>;
    pollRunStatusUntilTerminal: (runId: string, mode?: ComposerMode) => Promise<void>;
    triggerError: () => void;
};

type LegacyGoalFallbackParams = {
    prompt: string;
    composerMode: ComposerMode;
    setLastPlanId: SetString;
    setLastStatus: SetString;
    setRunSnapshot: Dispatch<SetStateAction<ExecutionSnapshot | null>>;
    setResults: SetResults;
    setRunPhase: SetRunPhase;
    setInput: Dispatch<SetStateAction<string>>;
    triggerSuccess: () => void;
    triggerError: () => void;
};

type PlanDispatchParams = SharedExecutionHandlers & {
    prompt: string;
    composerMode: ComposerMode;
    effectiveProfile: ExecutionProfile;
    fallbackToLegacyFromGoalRun: boolean;
    setPendingApproval: Dispatch<SetStateAction<ApprovalContext | null>>;
};

type DispatchErrorParams = {
    error: unknown;
    prompt: string;
    setResults: SetResults;
    setRunPhase: SetRunPhase;
    setInput: Dispatch<SetStateAction<string>>;
    triggerSuccess: () => void;
    triggerError: () => void;
};

const buildExecutionSummary = (
    intentLabel: string,
    composerMode: ComposerMode,
    effectiveProfile: ExecutionProfile,
    fallbackToLegacyFromGoalRun: boolean,
    execRes: Awaited<ReturnType<typeof agentExecute>>,
    verifyRes: Awaited<ReturnType<typeof agentVerify>>
) => {
    const summaryLines = [
        fallbackToLegacyFromGoalRun ? `**Mode**: legacy fallback (${composerMode})` : "",
        intentLabel,
        `**Status**: ${execRes.status}`,
        `**Profile**: ${execRes.profile ?? effectiveProfile}${execRes.collision_policy ? ` (collision=${execRes.collision_policy})` : ""}`,
        execRes.run_id ? `**Run ID**: ${execRes.run_id}` : "",
        execRes.resume_token ? `**Resume Token**: ${execRes.resume_token}` : "",
        `**Planner Complete**: ${execRes.planner_complete ? "yes" : "no"}`,
        `**Execution Complete**: ${execRes.execution_complete ? "yes" : "no"}`,
        `**Business Complete**: ${execRes.business_complete ? "yes" : "no"}`,
        `**Verify**: ${verifyRes.ok ? "ok" : "issues"}`,
        execRes.completion_score
            ? `**Completion Score**: ${execRes.completion_score.score} (${execRes.completion_score.label})`
            : "",
        execRes.resume_from != null ? `**Next Step**: ${execRes.resume_from + 1}` : "",
    ];
    const dodChecks = execRes.stage_dod ?? [];
    const dodFailed = dodChecks.filter((item) => !item.passed);
    if (dodChecks.length > 0) {
        summaryLines.push(
            `**DoD Checks**: ${dodChecks.length - dodFailed.length}/${dodChecks.length} passed`
        );
    }
    const logLines = execRes.logs?.slice(0, 10).map((line) => `- ${line}`) ?? [];
    const verifyLines = verifyRes.issues?.length
        ? verifyRes.issues.map((issue) => `- ${issue}`)
        : [];
    const manualLines = execRes.manual_steps?.length
        ? execRes.manual_steps.map((step) => `- ${step}`)
        : [];
    const dodLines = dodChecks.slice(0, 12).map(
        (item) =>
            `- ${item.passed ? "✅" : "❌"} [${item.stage}] ${item.key} (expected=${item.expected}, actual=${item.actual})`
    );
    const dodFailLines = dodFailed
        .slice(0, 8)
        .map((item) => `- [${item.stage}] ${item.key} (${item.actual})`);

    return [
        summaryLines.filter(Boolean).join("\n"),
        logLines.length ? `\n**Logs**\n${logLines.join("\n")}` : "",
        verifyLines.length ? `\n**Verify Issues**\n${verifyLines.join("\n")}` : "",
        dodLines.length ? `\n**Stage DoD**\n${dodLines.join("\n")}` : "",
        dodFailLines.length ? `\n**DoD Failed**\n${dodFailLines.join("\n")}` : "",
        manualLines.length ? `\n**Manual Steps**\n${manualLines.join("\n")}` : "",
        execRes.status === "approval_required" && execRes.approval
            ? `\n**Approval Required**\n- Action: ${execRes.approval.action}\n- Risk: ${execRes.approval.risk_level}\n- Policy: ${execRes.approval.policy}\n- ${execRes.approval.message}`
            : "",
    ].join("\n");
};

export async function handleChatDispatch({
    prompt,
    setShowDetailPanel,
    setLoading,
    setRunPhase,
    setPendingApproval,
    setRecoveryActionBusyKey,
    setResults,
    setInput,
    triggerSuccess,
    triggerError,
}: ChatDispatchParams) {
    setShowDetailPanel(true);
    setLoading(true);
    setRunPhase("running");
    setPendingApproval(null);
    setRecoveryActionBusyKey(null);

    try {
        const res = await sendChatMessage(prompt);
        const content =
            typeof res.response === "string" && res.response.trim().length > 0
                ? res.response
                : "✅ 요청을 받았어요. 한 문장만 더 구체적으로 말해주면 바로 도와줄게요.";
        setResults((prev) => appendChatTranscript(prev, prompt, content, false));
        setInput("");
        setRunPhase("completed");
        triggerSuccess();
    } catch (error) {
        const message =
            error instanceof Error ? error.message : "Failed to reach chat agent.";
        setResults((prev) => appendChatTranscript(prev, prompt, message, true));
        setRunPhase("failed");
        triggerError();
    } finally {
        setLoading(false);
    }
}

export async function tryGoalRunDispatch({
    prompt,
    composerMode,
    goalRunAvailable,
    setGoalRunAvailable,
    pollRunStatusUntilTerminal,
    updateExecutionState,
    loadRunDiagnostics,
    loadDodHistory,
    triggerSuccess,
    triggerError,
    setResults,
    setInput,
    setLastPlanId,
    setLastStatus,
    setRunPhase,
}: GoalRunDispatchParams): Promise<{ handled: boolean; fallbackToLegacyFromGoalRun: boolean }> {
    const useGoalRunPath =
        (composerMode === "nl" || composerMode === "program") && goalRunAvailable !== false;
    if (!useGoalRunPath) {
        return { handled: false, fallbackToLegacyFromGoalRun: false };
    }

    try {
        const goalRes = await agentGoalRun(prompt);
        if (goalRunAvailable !== true) {
            setGoalRunAvailable(true);
        }
        const goalStatusLower = goalRes.status.toLowerCase();
        const goalStatusInProgress = IN_PROGRESS_RUN_STATUSES.has(goalStatusLower);
        setLastPlanId(null);
        setLastStatus(goalRes.status);
        updateExecutionState({
            status: goalRes.status,
            runId: goalRes.run_id,
            resumeToken: null,
            plannerComplete: !!goalRes.planner_complete,
            executionComplete: !!goalRes.execution_complete,
            businessComplete: !!goalRes.business_complete,
            verifyOk: !!goalRes.business_complete,
            verifyIssues: goalRes.business_complete ? [] : ["business_complete=false"],
            completionScore: null,
        });
        await loadRunDiagnostics(goalRes.run_id);
        await loadDodHistory();
        const summary = summarizeGoalRunStatus({
            mode: composerMode,
            status: goalRes.status,
            runId: goalRes.run_id,
            plannerComplete: !!goalRes.planner_complete,
            executionComplete: !!goalRes.execution_complete,
            businessComplete: !!goalRes.business_complete,
            summary: goalRes.summary,
        });
        setResults([
            {
                type: toGoalRunResultType(goalRes.status, !!goalRes.business_complete),
                content: summary,
            },
        ]);
        setInput("");

        if (goalStatusLower === "approval_required") {
            setRunPhase("approval_required");
        } else if (goalStatusLower === "manual_required") {
            setRunPhase("manual_required");
        } else if (goalRes.business_complete || goalStatusLower === "business_completed") {
            setRunPhase("completed");
            triggerSuccess();
        } else if (goalStatusInProgress) {
            setRunPhase("running");
            void pollRunStatusUntilTerminal(goalRes.run_id, composerMode);
        } else {
            setRunPhase("failed");
            triggerError();
        }
        return { handled: true, fallbackToLegacyFromGoalRun: false };
    } catch (goalRunError) {
        if (!isGoalRunEndpointUnavailable(goalRunError)) {
            throw goalRunError;
        }
        setGoalRunAvailable(false);
        if (isLegacyGoalFallbackEnabled()) {
            console.warn(
                "goal-run endpoint unavailable. Falling back to legacy goal path.",
                goalRunError
            );
            return { handled: false, fallbackToLegacyFromGoalRun: true };
        }
        console.warn(
            "goal-run endpoint unavailable. Legacy fallback disabled; using intent/plan/execute path.",
            goalRunError
        );
        return { handled: false, fallbackToLegacyFromGoalRun: false };
    }
}

export async function tryLegacyGoalFallback({
    prompt,
    composerMode,
    setLastPlanId,
    setLastStatus,
    setRunSnapshot,
    setResults,
    setRunPhase,
    setInput,
    triggerSuccess,
    triggerError,
}: LegacyGoalFallbackParams): Promise<boolean> {
    try {
        const legacyRes = await executeGoal(prompt);
        const legacyStatus = (legacyRes.status || "started").toLowerCase();
        setLastPlanId(null);
        setLastStatus(legacyRes.status);
        setRunSnapshot({
            status: legacyRes.status || "started",
            runId: null,
            resumeToken: null,
            plannerComplete: legacyStatus !== "error",
            executionComplete: false,
            businessComplete: false,
            verifyOk: legacyStatus !== "error",
            verifyIssues: legacyStatus === "error" ? [legacyRes.message] : [],
            completionScore: null,
        });
        setResults([
            {
                type: legacyStatus === "error" ? "error" : "response",
                content: [
                    `**Mode**: legacy-goal (${composerMode})`,
                    `**Status**: ${legacyRes.status || "started"}`,
                    `**Message**: ${legacyRes.message || "Goal started."}`,
                ].join("\n"),
            },
        ]);
        if (legacyStatus === "error") {
            setRunPhase("failed");
            triggerError();
        } else {
            setRunPhase("running");
            triggerSuccess();
            setInput("");
        }
        return true;
    } catch (legacyGoalError) {
        console.warn(
            "legacy goal endpoint failed. Trying intent/plan/execute fallback.",
            legacyGoalError
        );
        return false;
    }
}

export async function handlePlanDispatch({
    prompt,
    composerMode,
    effectiveProfile,
    fallbackToLegacyFromGoalRun,
    updateExecutionState,
    loadRunDiagnostics,
    loadDodHistory,
    triggerSuccess,
    setResults,
    setInput,
    setLastPlanId,
    setLastStatus,
    setRunPhase,
    setPendingApproval,
}: PlanDispatchParams) {
    const intentRes = await agentIntent(prompt);
    if (intentRes.missing_slots && intentRes.missing_slots.length > 0) {
        const followUp = intentRes.follow_up || "추가 정보가 필요합니다.";
        setResults([
            {
                type: "response",
                content: `**추가 입력 필요**\n- Missing: ${intentRes.missing_slots.join(", ")}\n- ${followUp}`,
            },
        ]);
        setRunPhase("idle");
        return;
    }

    const planRes = await agentPlan(intentRes.session_id);
    setLastPlanId(planRes.plan_id);
    if (planRes.missing_slots?.length) {
        setResults([
            {
                type: "response",
                content: `**추가 입력 필요**\n- Missing: ${planRes.missing_slots.join(", ")}`,
            },
        ]);
        setRunPhase("idle");
        return;
    }

    const execRes = await agentExecute(planRes.plan_id, effectiveProfile);
    setLastStatus(execRes.status);
    const verifyRes = await agentVerify(planRes.plan_id);
    updateExecutionState({
        status: execRes.status,
        runId: execRes.run_id ?? null,
        resumeToken: execRes.resume_token ?? null,
        plannerComplete: !!execRes.planner_complete,
        executionComplete: !!execRes.execution_complete,
        businessComplete: !!execRes.business_complete,
        verifyOk: !!verifyRes.ok,
        verifyIssues: verifyRes.issues ?? [],
        completionScore: execRes.completion_score ?? null,
    });
    await loadRunDiagnostics(execRes.run_id);
    await loadDodHistory();

    if (execRes.status === "approval_required" && execRes.approval?.action) {
        setPendingApproval({
            planId: planRes.plan_id,
            action: execRes.approval.action,
            message: execRes.approval.message,
            riskLevel: execRes.approval.risk_level,
            policy: execRes.approval.policy,
        });
    }

    setResults([
        {
            type: "response",
            content: buildExecutionSummary(
                `**Intent**: ${intentRes.intent} (${Math.round(intentRes.confidence * 100)}%)`,
                composerMode,
                effectiveProfile,
                fallbackToLegacyFromGoalRun,
                execRes,
                verifyRes
            ),
        },
    ]);
    setInput("");
    triggerSuccess();
}

export async function handleDispatchError({
    error,
    prompt,
    setResults,
    setRunPhase,
    setInput,
    triggerSuccess,
    triggerError,
}: DispatchErrorParams) {
    console.error("Launcher send failed", error);
    const maybe = error as {
        message?: string;
        response?: {
            status?: number;
            data?: {
                error?: string;
                message?: string;
                detail?: string;
                lock_scope?: string;
                active_plan_id?: string;
            };
        };
    };
    const apiErr = maybe.response?.data?.error;
    if (
        typeof apiErr === "string" &&
        [
            "agent_execution_in_progress_global",
            "plan_execution_in_progress",
            "plan_execution_in_progress_db",
        ].includes(apiErr)
    ) {
        const scope = maybe.response?.data?.lock_scope ?? "plan";
        const activePlan = maybe.response?.data?.active_plan_id ?? "";
        const detail =
            typeof maybe.response?.data?.message === "string"
                ? maybe.response?.data?.message
                : "다른 실행이 진행 중입니다.";
        setResults([
            {
                type: "error",
                content: [
                    "**실행 충돌 감지**",
                    `- scope: ${scope}`,
                    activePlan ? `- active_plan_id: ${activePlan}` : "",
                    `- ${detail}`,
                ]
                    .filter(Boolean)
                    .join("\n"),
            },
        ]);
        setRunPhase("failed");
        triggerError();
        return;
    }
    if (shouldTryNlChatFallback(error)) {
        try {
            const res = await sendChatMessage(prompt);
            const content =
                typeof res.response === "string" && res.response.trim().length > 0
                    ? res.response
                    : "✅ 요청을 받았어요. 한 문장만 더 구체적으로 말해주면 바로 도와줄게요.";
            setResults([{ type: "response", content }]);
            setInput("");
            setRunPhase("completed");
            triggerSuccess();
            return;
        } catch {
            // fall through
        }
    }
    const statusCode = maybe.response?.status;
    const errorDetail =
        maybe.response?.data?.detail && typeof maybe.response.data.detail === "string"
            ? maybe.response.data.detail
            : "";
    const lowerErr =
        `${maybe.response?.data?.error ?? ""} ${errorDetail} ${maybe.message ?? ""}`.toLowerCase();
    const detailMsg =
        errorDetail ||
        maybe.response?.data?.message ||
        maybe.response?.data?.error ||
        maybe.message ||
        "Failed to reach agent.";
    const normalizedMsg =
        lowerErr.includes("screen capture unavailable") ||
        lowerErr.includes("permission missing")
            ? "화면 캡처 권한이 없어 실행이 중단됐습니다. 시스템 설정에서 AllvIa/Terminal의 화면 기록 권한을 켠 뒤 다시 시도하세요."
            : detailMsg;
    setResults([
        {
            type: "error",
            content: `실행 실패${statusCode ? ` (${statusCode})` : ""}: ${normalizedMsg}`,
        },
    ]);
    setRunPhase("failed");
    triggerError();
}
