import { useCallback, useEffect, useRef } from "react";
import type { Dispatch, SetStateAction } from "react";

import {
    agentApprove,
    agentExecute,
    agentVerify,
    fetchTaskRun,
} from "@/lib/api";
import type { ExecutionProfile, TaskRun } from "@/lib/types";
import {
    TERMINAL_RUN_STATUSES,
    summarizeGoalRunStatus,
    toGoalRunResultType,
    type ApprovalContext,
    type ComposerMode,
    type ExecutionSnapshot,
    type LauncherResult,
    type ManualResumeChecklist,
    type RecoveryAction,
    type RunPhase,
} from "@/features/launcher/support";

type UseLauncherRecoveryFlowParams = {
    safeExecutionMode: boolean;
    activeExecutionProfile: ExecutionProfile;
    recoveryActionBusyKey: string | null;
    lastPlanId: string | null;
    lastStatus: string | null;
    pendingApproval: ApprovalContext | null;
    runSnapshot: ExecutionSnapshot | null;
    manualChecklist: ManualResumeChecklist;
    firstFailedArtifactPath: string | null;
    recoveryActions: RecoveryAction[];
    loadRunDiagnostics: (runId?: string | null) => Promise<void>;
    loadDodHistory: () => Promise<void>;
    runPreflightCheck: (silent?: boolean) => Promise<boolean>;
    handlePreflightFix: (action: string) => Promise<void>;
    openArtifactPath: (path: string) => Promise<void>;
    recordRecoveryAction: (payload: {
        actionKey: string;
        status: "completed" | "failed";
        details: string;
        runId?: string | null;
        stageName?: string;
        expected?: string;
        actual?: string;
    }) => Promise<void>;
    updateExecutionState: (snapshot: ExecutionSnapshot) => void;
    triggerSuccess: () => void;
    triggerError: () => void;
    setApprovalBusy: Dispatch<SetStateAction<boolean>>;
    setRecoveryActionBusyKey: Dispatch<SetStateAction<string | null>>;
    setResults: Dispatch<SetStateAction<LauncherResult[]>>;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
    setRunPhase: Dispatch<SetStateAction<RunPhase>>;
    setLoading: Dispatch<SetStateAction<boolean>>;
    setLastStatus: Dispatch<SetStateAction<string | null>>;
    setPendingApproval: Dispatch<SetStateAction<ApprovalContext | null>>;
    setManualChecklist: Dispatch<SetStateAction<ManualResumeChecklist>>;
};

const toSnapshotFromTaskRun = (run: TaskRun): ExecutionSnapshot => {
    const statusLower = run.status.toLowerCase();
    const verifyOk = run.business_complete || statusLower === "business_completed";
    return {
        status: run.status,
        runId: run.run_id,
        resumeToken: null,
        plannerComplete: !!run.planner_complete,
        executionComplete: !!run.execution_complete,
        businessComplete: !!run.business_complete,
        verifyOk,
        verifyIssues: verifyOk ? [] : [`status=${run.status}`],
        completionScore: null,
    };
};

const buildExecutionSummaryContent = (
    execRes: Awaited<ReturnType<typeof agentExecute>>,
    verifyRes: Awaited<ReturnType<typeof agentVerify>>,
    profile: ExecutionProfile
) => {
    const summaryLines = [
        `**Status**: ${execRes.status}`,
        `**Profile**: ${execRes.profile ?? profile}${execRes.collision_policy ? ` (collision=${execRes.collision_policy})` : ""}`,
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
    const logLines = execRes.logs?.slice(0, 10).map((line) => `- ${line}`) ?? [];
    const verifyLines = verifyRes.issues?.length
        ? verifyRes.issues.map((issue) => `- ${issue}`)
        : [];
    const manualLines = execRes.manual_steps?.length
        ? execRes.manual_steps.map((step) => `- ${step}`)
        : [];
    return [
        summaryLines.filter(Boolean).join("\n"),
        logLines.length ? `\n**Logs**\n${logLines.join("\n")}` : "",
        verifyLines.length ? `\n**Verify Issues**\n${verifyLines.join("\n")}` : "",
        manualLines.length ? `\n**Manual Steps**\n${manualLines.join("\n")}` : "",
    ].join("\n");
};

export function useLauncherRecoveryFlow({
    safeExecutionMode,
    activeExecutionProfile,
    recoveryActionBusyKey,
    lastPlanId,
    lastStatus,
    pendingApproval,
    runSnapshot,
    manualChecklist,
    firstFailedArtifactPath,
    recoveryActions,
    loadRunDiagnostics,
    loadDodHistory,
    runPreflightCheck,
    handlePreflightFix,
    openArtifactPath,
    recordRecoveryAction,
    updateExecutionState,
    triggerSuccess,
    triggerError,
    setApprovalBusy,
    setRecoveryActionBusyKey,
    setResults,
    setShowDetailPanel,
    setRunPhase,
    setLoading,
    setLastStatus,
    setPendingApproval,
    setManualChecklist,
}: UseLauncherRecoveryFlowParams) {
    const runPollTokenRef = useRef(0);

    useEffect(() => {
        return () => {
            runPollTokenRef.current += 1;
        };
    }, []);

    const pollRunStatusUntilTerminal = useCallback(
        async (runId: string, mode: ComposerMode = "nl") => {
            if (!runId) return;
            const token = Date.now();
            runPollTokenRef.current = token;

            for (let i = 0; i < 60; i += 1) {
                if (runPollTokenRef.current !== token) return;
                try {
                    const run = await fetchTaskRun(runId);
                    if (runPollTokenRef.current !== token) return;
                    setLastStatus(run.status);
                    updateExecutionState(toSnapshotFromTaskRun(run));

                    if (i % 2 === 0) {
                        void loadRunDiagnostics(runId).catch((diagnosticsError) => {
                            console.warn("run diagnostics refresh failed", diagnosticsError);
                        });
                    }

                    const statusLower = run.status.toLowerCase();
                    if (TERMINAL_RUN_STATUSES.has(statusLower)) {
                        await Promise.all([
                            loadRunDiagnostics(runId),
                            loadDodHistory(),
                        ]);
                        const terminalSummary = summarizeGoalRunStatus({
                            mode,
                            status: run.status,
                            runId: run.run_id,
                            plannerComplete: !!run.planner_complete,
                            executionComplete: !!run.execution_complete,
                            businessComplete: !!run.business_complete,
                            summary:
                                statusLower === "business_completed"
                                    ? "goal completed and business checks passed"
                                    : undefined,
                        });
                        setResults([
                            {
                                type: toGoalRunResultType(run.status, !!run.business_complete),
                                content: terminalSummary,
                            },
                        ]);
                        if (statusLower === "business_completed") {
                            triggerSuccess();
                        } else if (!["approval_required", "manual_required"].includes(statusLower)) {
                            triggerError();
                        }
                        return;
                    }
                } catch (error) {
                    console.warn("run polling failed", error);
                }
                await new Promise((resolve) => window.setTimeout(resolve, 1500));
            }
        },
        [
            loadDodHistory,
            loadRunDiagnostics,
            setLastStatus,
            setResults,
            triggerError,
            triggerSuccess,
            updateExecutionState,
        ]
    );

    const executePlanAndRefresh = useCallback(
        async (
            planId: string,
            profile: ExecutionProfile,
            resumeToken?: string | null
        ) => {
            const execRes = await agentExecute(planId, profile, {
                resumeToken: resumeToken ?? null,
            });
            setLastStatus(execRes.status);
            const verifyRes = await agentVerify(planId);
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
            await Promise.all([
                loadRunDiagnostics(execRes.run_id),
                loadDodHistory(),
            ]);
            if (execRes.status === "approval_required" && execRes.approval?.action) {
                setPendingApproval({
                    planId,
                    action: execRes.approval.action,
                    message: execRes.approval.message,
                    riskLevel: execRes.approval.risk_level,
                    policy: execRes.approval.policy,
                });
            } else {
                setPendingApproval(null);
            }
            setResults([
                {
                    type: "response",
                    content: buildExecutionSummaryContent(execRes, verifyRes, profile),
                },
            ]);
        },
        [
            loadDodHistory,
            loadRunDiagnostics,
            setLastStatus,
            setPendingApproval,
            setResults,
            updateExecutionState,
        ]
    );

    const resumeExecution = useCallback(
        async (skipChecklist = false) => {
            if (!lastPlanId) return;
            const checklistReady =
                skipChecklist ||
                (manualChecklist.focusReady &&
                    manualChecklist.manualStepDone &&
                    manualChecklist.handsOffReady);
            if (!checklistReady) {
                setResults([
                    {
                        type: "error",
                        content:
                            "Resume 차단: 체크리스트 3개(포커스 복구/수동 단계 완료/입력 충돌 방지)를 모두 확인해야 합니다.",
                    },
                ]);
                setShowDetailPanel(true);
                triggerError();
                return;
            }
            if (skipChecklist) {
                setManualChecklist({
                    focusReady: true,
                    manualStepDone: true,
                    handsOffReady: true,
                });
            }
            const preflightReady = await runPreflightCheck(true);
            if (!preflightReady) {
                setResults([
                    {
                        type: "error",
                        content:
                            "Resume 차단: preflight가 통과되지 않았습니다. 포커스/권한 상태를 복구한 뒤 다시 시도하세요.",
                    },
                ]);
                setRunPhase("manual_required");
                setShowDetailPanel(true);
                triggerError();
                return;
            }
            setLoading(true);
            setRunPhase("retrying");
            try {
                const resumeProfile = safeExecutionMode ? "strict" : activeExecutionProfile;
                await executePlanAndRefresh(
                    lastPlanId,
                    resumeProfile,
                    runSnapshot?.resumeToken ?? null
                );
                triggerSuccess();
            } catch (error) {
                console.error("Resume failed", error);
                setRunPhase("failed");
                triggerError();
            } finally {
                setLoading(false);
            }
        },
        [
            activeExecutionProfile,
            executePlanAndRefresh,
            lastPlanId,
            manualChecklist,
            runPreflightCheck,
            runSnapshot?.resumeToken,
            safeExecutionMode,
            setLoading,
            setManualChecklist,
            setResults,
            setRunPhase,
            setShowDetailPanel,
            triggerError,
            triggerSuccess,
        ]
    );

    const handleResume = useCallback(async () => {
        await resumeExecution(false);
    }, [resumeExecution]);

    const handleGuidedRecovery = useCallback(async () => {
        if (firstFailedArtifactPath) {
            await openArtifactPath(firstFailedArtifactPath);
        }
        if (lastStatus === "manual_required" && lastPlanId && !pendingApproval) {
            await resumeExecution(true);
        }
    }, [
        firstFailedArtifactPath,
        lastPlanId,
        lastStatus,
        openArtifactPath,
        pendingApproval,
        resumeExecution,
    ]);

    const runRecoveryAction = useCallback(
        async (action: RecoveryAction) => {
            if (recoveryActionBusyKey) return;
            setRecoveryActionBusyKey(action.key);
            let status: "completed" | "failed" = "completed";
            let details = action.description;
            try {
                if (action.kind === "preflight_fix" && action.fixAction) {
                    await handlePreflightFix(action.fixAction);
                    details = `preflight_fix:${action.fixAction}`;
                } else if (action.kind === "artifact" && action.path) {
                    await openArtifactPath(action.path);
                    details = `artifact:${action.path}`;
                } else if (action.kind === "guided_resume") {
                    await handleGuidedRecovery();
                    details = "guided_resume";
                }
            } catch (error) {
                status = "failed";
                const message = error instanceof Error ? error.message : String(error);
                details = `${details} (${message})`;
            } finally {
                if (action.kind !== "preflight_fix") {
                    await recordRecoveryAction({
                        actionKey: action.assertionKey ?? `recovery.action.${action.key}`,
                        status,
                        details,
                        actual: status,
                    });
                }
                setRecoveryActionBusyKey(null);
            }
        },
        [
            handleGuidedRecovery,
            handlePreflightFix,
            openArtifactPath,
            recordRecoveryAction,
            recoveryActionBusyKey,
            setRecoveryActionBusyKey,
        ]
    );

    const handleOneClickRecovery = useCallback(async () => {
        if (recoveryActions.length === 0) return;
        await runRecoveryAction(recoveryActions[0]);
    }, [recoveryActions, runRecoveryAction]);

    const handleApprovalDecision = useCallback(
        async (decision: "allow_once" | "allow_always" | "deny") => {
            const approval = pendingApproval;
            if (!approval) return;
            setApprovalBusy(true);
            setRunPhase("approval_required");
            try {
                const approvalRes = await agentApprove(
                    approval.planId,
                    approval.action,
                    decision
                );
                if (decision === "deny" || approvalRes.status === "denied") {
                    setResults([
                        {
                            type: "response",
                            content: `**Approval Denied**\n- Action: ${approval.action}\n- Policy: ${approvalRes.policy}`,
                        },
                    ]);
                    setPendingApproval(null);
                    setRunPhase("failed");
                    triggerSuccess();
                    return;
                }
                const approvalProfile = safeExecutionMode ? "strict" : activeExecutionProfile;
                await executePlanAndRefresh(
                    approval.planId,
                    approvalProfile,
                    runSnapshot?.resumeToken ?? null
                );
                triggerSuccess();
            } catch (error) {
                console.error("Approval flow failed", error);
                setRunPhase("failed");
                triggerError();
            } finally {
                setApprovalBusy(false);
            }
        },
        [
            activeExecutionProfile,
            executePlanAndRefresh,
            pendingApproval,
            runSnapshot?.resumeToken,
            safeExecutionMode,
            setApprovalBusy,
            setPendingApproval,
            setResults,
            setRunPhase,
            triggerError,
            triggerSuccess,
        ]
    );

    return {
        pollRunStatusUntilTerminal,
        handleResume,
        handleGuidedRecovery,
        runRecoveryAction,
        handleOneClickRecovery,
        handleApprovalDecision,
    };
}
