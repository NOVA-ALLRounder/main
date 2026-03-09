import { useCallback } from "react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";

import type { ExecutionProfile, TaskStageAssertion, TaskStageRun } from "@/lib/types";
import {
    handleChatDispatch,
    handleDispatchError,
    handlePlanDispatch,
    tryGoalRunDispatch,
    tryLegacyGoalFallback,
} from "@/features/launcher/hooks/dispatchFlowSupport";
import {
    normalizeDispatchPrompt,
    profileLabel,
    type ApprovalContext,
    type ComposerMode,
    type ExecutionSnapshot,
    type LauncherResult,
    type PendingDispatch,
    type RunPhase,
} from "@/features/launcher/support";

type ProfileRecommendation = {
    profile: ExecutionProfile;
    reason: string;
};

type UseLauncherDispatchFlowParams = {
    composerMode: ComposerMode;
    safeExecutionMode: boolean;
    executionProfile: ExecutionProfile;
    autoApplyRecommendedProfile: boolean;
    profileRecommendation: ProfileRecommendation;
    loading: boolean;
    isExecutionLocked: boolean;
    pendingDispatch: PendingDispatch | null;
    goalRunAvailable: boolean | null;
    setGoalRunAvailable: Dispatch<SetStateAction<boolean | null>>;
    lastDispatchRef: MutableRefObject<{ promptKey: string; ts: number } | null>;
    pollRunStatusUntilTerminal: (runId: string, mode?: ComposerMode) => Promise<void>;
    runPreflightCheck: (silent?: boolean) => Promise<boolean>;
    updateExecutionState: (snapshot: ExecutionSnapshot) => void;
    loadRunDiagnostics: (runId?: string | null) => Promise<void>;
    loadDodHistory: () => Promise<void>;
    triggerSuccess: () => void;
    triggerError: () => void;
    setInput: Dispatch<SetStateAction<string>>;
    setExecutionProfile: Dispatch<SetStateAction<ExecutionProfile>>;
    setActiveExecutionProfile: Dispatch<SetStateAction<ExecutionProfile>>;
    setLastPlanId: Dispatch<SetStateAction<string | null>>;
    setLastStatus: Dispatch<SetStateAction<string | null>>;
    setRunPhase: Dispatch<SetStateAction<RunPhase>>;
    setDispatchBlockedReason: Dispatch<SetStateAction<string | null>>;
    setDispatchBlockedUntilMs: Dispatch<SetStateAction<number | null>>;
    setPendingDispatch: Dispatch<SetStateAction<PendingDispatch | null>>;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
    setLoading: Dispatch<SetStateAction<boolean>>;
    setPendingApproval: Dispatch<SetStateAction<ApprovalContext | null>>;
    setRecoveryActionBusyKey: Dispatch<SetStateAction<string | null>>;
    setResults: Dispatch<SetStateAction<LauncherResult[]>>;
    setRunSnapshot: Dispatch<SetStateAction<ExecutionSnapshot | null>>;
    setStageRuns: Dispatch<SetStateAction<TaskStageRun[]>>;
    setStageAssertions: Dispatch<SetStateAction<TaskStageAssertion[]>>;
    setArtifactActionMessage: Dispatch<SetStateAction<string | null>>;
};

export function useLauncherDispatchFlow({
    composerMode,
    safeExecutionMode,
    executionProfile,
    autoApplyRecommendedProfile,
    profileRecommendation,
    loading,
    isExecutionLocked,
    pendingDispatch,
    goalRunAvailable,
    setGoalRunAvailable,
    lastDispatchRef,
    pollRunStatusUntilTerminal,
    runPreflightCheck,
    updateExecutionState,
    loadRunDiagnostics,
    loadDodHistory,
    triggerSuccess,
    triggerError,
    setInput,
    setExecutionProfile,
    setActiveExecutionProfile,
    setLastPlanId,
    setLastStatus,
    setRunPhase,
    setDispatchBlockedReason,
    setDispatchBlockedUntilMs,
    setPendingDispatch,
    setShowDetailPanel,
    setLoading,
    setPendingApproval,
    setRecoveryActionBusyKey,
    setResults,
    setRunSnapshot,
    setStageRuns,
    setStageAssertions,
    setArtifactActionMessage,
}: UseLauncherDispatchFlowParams) {
    const dispatchPrompt = useCallback(
        async (rawPrompt: string, bypassSafeCountdown: boolean = false) => {
            const prompt = rawPrompt.trim();
            if (!prompt) return;
            const promptKey = normalizeDispatchPrompt(prompt);
            if (
                !bypassSafeCountdown &&
                pendingDispatch &&
                normalizeDispatchPrompt(pendingDispatch.prompt) === promptKey
            ) {
                setDispatchBlockedReason("중복 실행 차단");
                setDispatchBlockedUntilMs(pendingDispatch.executeAtMs);
                setResults([
                    {
                        type: "error",
                        content:
                            "이미 같은 요청이 안전 카운트다운 중입니다. 취소하거나 카운트다운 완료를 기다리세요.",
                    },
                ]);
                setShowDetailPanel(true);
                return;
            }
            if (composerMode !== "chat" && safeExecutionMode && !bypassSafeCountdown) {
                const executeAtMs = Date.now() + 3000;
                setPendingDispatch({ prompt, executeAtMs });
                setDispatchBlockedReason("안전 카운트다운");
                setDispatchBlockedUntilMs(executeAtMs);
                setResults([
                    {
                        type: "response",
                        content:
                            `**안전 실행 대기**\n- ${Math.max(
                                1,
                                Math.ceil((executeAtMs - Date.now()) / 1000)
                            )}초 후 자동 실행됩니다.\n- 취소 버튼으로 중단할 수 있습니다.`,
                    },
                ]);
                setShowDetailPanel(true);
                setRunPhase("idle");
                return;
            }
            if (composerMode !== "chat" && bypassSafeCountdown && pendingDispatch) {
                setPendingDispatch(null);
            }
            if (loading) {
                setDispatchBlockedReason("이전 요청 준비 중");
                setDispatchBlockedUntilMs(Date.now() + 4000);
                setResults([
                    {
                        type: "error",
                        content: "이전 요청을 준비 중입니다. 잠시 후 다시 실행하세요.",
                    },
                ]);
                setShowDetailPanel(true);
                return;
            }
            if (isExecutionLocked) {
                setDispatchBlockedReason("실행 잠금 상태");
                setDispatchBlockedUntilMs(null);
                setResults([
                    {
                        type: "error",
                        content:
                            "실행 중에는 새 요청을 받지 않습니다. 현재 실행이 끝난 뒤 다시 시도하세요.",
                    },
                ]);
                setShowDetailPanel(true);
                return;
            }
            const now = Date.now();
            const duplicateWindowMs = 12000;
            if (
                lastDispatchRef.current &&
                lastDispatchRef.current.promptKey === promptKey &&
                now - lastDispatchRef.current.ts < duplicateWindowMs
            ) {
                const retryAt = lastDispatchRef.current.ts + duplicateWindowMs;
                setDispatchBlockedReason("중복 실행 차단");
                setDispatchBlockedUntilMs(retryAt);
                setResults([
                    {
                        type: "error",
                        content:
                            "중복 실행 차단: 같은 요청이 방금 실행되었습니다. 결과 패널을 확인하거나 잠시 후 다시 실행하세요.",
                    },
                ]);
                setRunPhase("idle");
                setShowDetailPanel(true);
                return;
            }
            lastDispatchRef.current = { promptKey, ts: now };
            setDispatchBlockedReason(null);
            setDispatchBlockedUntilMs(null);

            if (composerMode === "chat") {
                await handleChatDispatch({
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
                });
                return;
            }

            const preflightReady = await runPreflightCheck(false);
            if (!preflightReady) {
                setDispatchBlockedReason("Preflight 차단");
                setDispatchBlockedUntilMs(null);
                setRunPhase("failed");
                triggerError();
                setShowDetailPanel(true);
                return;
            }

            let effectiveProfile = safeExecutionMode ? "strict" : executionProfile;
            if (
                !safeExecutionMode &&
                executionProfile === "strict" &&
                profileRecommendation.profile === "test"
            ) {
                if (autoApplyRecommendedProfile) {
                    effectiveProfile = profileRecommendation.profile;
                    setExecutionProfile(effectiveProfile);
                } else {
                    setDispatchBlockedReason("권장 프로필 불일치");
                    setDispatchBlockedUntilMs(null);
                    setResults([
                        {
                            type: "error",
                            content: `실행 차단: 현재 상태에서는 ${profileLabel(profileRecommendation.profile)} 프로필을 권장합니다.\n사유: ${profileRecommendation.reason}`,
                        },
                    ]);
                    setRunPhase("failed");
                    setShowDetailPanel(true);
                    triggerError();
                    return;
                }
            }

            setShowDetailPanel(true);
            setLoading(true);
            setRunPhase("running");
            setActiveExecutionProfile(effectiveProfile);
            setRunSnapshot(null);
            setStageRuns([]);
            setStageAssertions([]);
            setResults([]);
            setDispatchBlockedReason(null);
            setDispatchBlockedUntilMs(null);
            setArtifactActionMessage(null);
            setPendingApproval(null);
            setRecoveryActionBusyKey(null);

            if (prompt === "test_perf") {
                if (!import.meta.env.DEV) {
                    setResults([
                        {
                            type: "error",
                            content: "`test_perf`는 개발 모드에서만 사용할 수 있습니다.",
                        },
                    ]);
                    setShowDetailPanel(true);
                    setRunPhase("failed");
                    triggerError();
                    setLoading(false);
                    return;
                }
                const start = performance.now();
                const dummyItems: LauncherResult[] = Array.from({ length: 1000 }, (_, i) => ({
                    type: "response" as const,
                    content: `**Perf Item #${i + 1}**: This is a dummy item to test rendering performance. ${Math.random()}`,
                }));
                const end = performance.now();
                setResults(dummyItems);
                setInput("");
                triggerSuccess();
                setLoading(false);
                console.log(`[Perf] Generated 1000 items in ${(end - start).toFixed(2)}ms`);
                return;
            }

            try {
                const goalRunResult = await tryGoalRunDispatch({
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
                });
                if (goalRunResult.handled) {
                    return;
                }

                const fallbackToLegacyFromGoalRun =
                    goalRunResult.fallbackToLegacyFromGoalRun;
                if (fallbackToLegacyFromGoalRun) {
                    const legacyHandled = await tryLegacyGoalFallback({
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
                    });
                    if (legacyHandled) {
                        return;
                    }
                }

                await handlePlanDispatch({
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
                });
                return;
            } catch (error) {
                await handleDispatchError({
                    error,
                    prompt,
                    setResults,
                    setRunPhase,
                    setInput,
                    triggerSuccess,
                    triggerError,
                });
            } finally {
                setLoading(false);
            }
        },
        [
            autoApplyRecommendedProfile,
            composerMode,
            executionProfile,
            goalRunAvailable,
            isExecutionLocked,
            lastDispatchRef,
            loadDodHistory,
            loadRunDiagnostics,
            loading,
            pendingDispatch,
            pollRunStatusUntilTerminal,
            profileRecommendation,
            runPreflightCheck,
            safeExecutionMode,
            setActiveExecutionProfile,
            setArtifactActionMessage,
            setDispatchBlockedReason,
            setDispatchBlockedUntilMs,
            setExecutionProfile,
            setGoalRunAvailable,
            setInput,
            setLastPlanId,
            setLastStatus,
            setLoading,
            setPendingDispatch,
            setRecoveryActionBusyKey,
            setResults,
            setRunPhase,
            setRunSnapshot,
            setShowDetailPanel,
            setStageAssertions,
            setStageRuns,
            triggerError,
            triggerSuccess,
            updateExecutionState,
        ]
    );

    return {
        dispatchPrompt,
    };
}
