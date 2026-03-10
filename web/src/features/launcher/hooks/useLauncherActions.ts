import { useEffect } from "react";
import { useLauncherDispatchFlow } from "@/features/launcher/hooks/useLauncherDispatchFlow";
import { useLauncherIo } from "@/features/launcher/hooks/useLauncherIo";
import { useLauncherPreflight } from "@/features/launcher/hooks/useLauncherPreflight";
import { useLauncherRecoveryFlow } from "@/features/launcher/hooks/useLauncherRecoveryFlow";
import { useLauncherShell } from "@/features/launcher/hooks/useLauncherShell";
import { useLauncherUiHandlers } from "@/features/launcher/hooks/useLauncherUiHandlers";
import { useRecommendationApprovalFlow } from "@/features/launcher/hooks/useRecommendationApprovalFlow";
import type { LauncherRuntimeView } from "@/features/launcher/hooks/useLauncherRuntimeView";

type LauncherRuntimeSource = Omit<LauncherRuntimeView, "actionState" | "actionRefs">;

export type UseLauncherActionRefs = Pick<
    LauncherRuntimeSource,
    | "composingSinceRef"
    | "dispatchPromptRef"
    | "inputRef"
    | "lastDispatchRef"
    | "prevComposerModeRef"
    | "scrollRef"
    | "sendThrottleRef"
>;

export type UseLauncherActionsState = Omit<LauncherRuntimeSource, keyof UseLauncherActionRefs>;

export function useLauncherActions(state: UseLauncherActionsState, refs: UseLauncherActionRefs) {
    const {
        composingSinceRef,
        dispatchPromptRef,
        inputRef,
        lastDispatchRef,
        prevComposerModeRef,
        scrollRef,
        sendThrottleRef,
    } = refs;

    const { runPreflightCheck, handlePreflightFix } = useLauncherPreflight({
        currentRunId: state.runSnapshot?.runId ?? null,
        preflightOk: state.preflightOk,
        loadRunDiagnostics: state.loadRunDiagnostics,
        loadDodHistory: state.loadDodHistory,
        setPreflightChecks: state.setPreflightChecks,
        setPreflightOk: state.setPreflightOk,
        setPreflightLoading: state.setPreflightLoading,
        setPreflightError: state.setPreflightError,
        setPreflightCheckedAt: state.setPreflightCheckedAt,
        setPreflightActiveApp: state.setPreflightActiveApp,
        setShowPreflightDetail: state.setShowPreflightDetail,
        setPreflightFixBusy: state.setPreflightFixBusy,
        setPreflightFixMessage: state.setPreflightFixMessage,
        setResults: state.setResults,
        setShowDetailPanel: state.setShowDetailPanel,
    });

    const {
        triggerSuccess,
        triggerError,
        handlePin,
        recordRecoveryAction,
        openArtifactPath,
        openExternalTarget,
        copyTextValue,
        copyArtifactPayload,
        togglePinArtifactKey,
    } = useLauncherIo({
        runId: state.runSnapshot?.runId ?? null,
        fallbackRunId: state.dodHistory[0]?.runId ?? null,
        setSuccessPulse: state.setSuccessPulse,
        setShake: state.setShake,
        setArtifactActionMessage: state.setArtifactActionMessage,
        setArtifactOpenBusy: state.setArtifactOpenBusy,
        setPinnedArtifactKeys: state.setPinnedArtifactKeys,
        loadRunDiagnostics: state.loadRunDiagnostics,
        loadDodHistory: state.loadDodHistory,
    });

    const {
        approvingIds,
        approveErrors,
        n8nOpenBusyKey,
        handleApprove,
        openRecommendationTarget,
    } = useRecommendationApprovalFlow({
        recs: state.recs,
        refetch: state.refetch,
        provisioningUiByRecId: state.provisioningUiByRecId,
        setProvisioningUiState: state.setProvisioningUiState,
        clearProvisioningUiState: state.clearProvisioningUiState,
        addWatchRecommendation: state.addWatchRecommendation,
        removeWatchRecommendation: state.removeWatchRecommendation,
        setWatchRecommendationCache: state.setWatchRecommendationCache,
        openExternalTarget,
        triggerSuccess,
        triggerError,
        setResults: state.setResults,
        setShowDetailPanel: state.setShowDetailPanel,
    });

    const {
        pollRunStatusUntilTerminal,
        handleResume,
        handleGuidedRecovery,
        runRecoveryAction,
        handleOneClickRecovery,
        handleApprovalDecision,
    } = useLauncherRecoveryFlow({
        safeExecutionMode: state.safeExecutionMode,
        activeExecutionProfile: state.activeExecutionProfile,
        recoveryActionBusyKey: state.recoveryActionBusyKey,
        lastPlanId: state.lastPlanId,
        lastStatus: state.lastStatus,
        pendingApproval: state.pendingApproval,
        runSnapshot: state.runSnapshot,
        manualChecklist: state.manualChecklist,
        firstFailedArtifactPath: state.firstFailedArtifactPath,
        recoveryActions: state.recoveryActions,
        loadRunDiagnostics: state.loadRunDiagnostics,
        loadDodHistory: state.loadDodHistory,
        runPreflightCheck,
        handlePreflightFix,
        openArtifactPath,
        recordRecoveryAction,
        updateExecutionState: state.updateExecutionState,
        triggerSuccess,
        triggerError,
        setApprovalBusy: state.setApprovalBusy,
        setRecoveryActionBusyKey: state.setRecoveryActionBusyKey,
        setResults: state.setResults,
        setShowDetailPanel: state.setShowDetailPanel,
        setRunPhase: state.setRunPhase,
        setLoading: state.setLoading,
        setLastStatus: state.setLastStatus,
        setPendingApproval: state.setPendingApproval,
        setManualChecklist: state.setManualChecklist,
    });

    const { dispatchPrompt } = useLauncherDispatchFlow({
        composerMode: state.composerMode,
        safeExecutionMode: state.safeExecutionMode,
        executionProfile: state.executionProfile,
        autoApplyRecommendedProfile: state.autoApplyRecommendedProfile,
        profileRecommendation: state.profileRecommendation,
        loading: state.loading,
        isExecutionLocked: state.isExecutionLocked,
        pendingDispatch: state.pendingDispatch,
        goalRunAvailable: state.goalRunAvailable,
        setGoalRunAvailable: state.setGoalRunAvailable,
        lastDispatchRef,
        pollRunStatusUntilTerminal,
        runPreflightCheck,
        updateExecutionState: state.updateExecutionState,
        loadRunDiagnostics: state.loadRunDiagnostics,
        loadDodHistory: state.loadDodHistory,
        triggerSuccess,
        triggerError,
        setInput: state.setInput,
        setExecutionProfile: state.setExecutionProfile,
        setActiveExecutionProfile: state.setActiveExecutionProfile,
        setLastPlanId: state.setLastPlanId,
        setLastStatus: state.setLastStatus,
        setRunPhase: state.setRunPhase,
        setDispatchBlockedReason: state.setDispatchBlockedReason,
        setDispatchBlockedUntilMs: state.setDispatchBlockedUntilMs,
        setPendingDispatch: state.setPendingDispatch,
        setShowDetailPanel: state.setShowDetailPanel,
        setLoading: state.setLoading,
        setPendingApproval: state.setPendingApproval,
        setRecoveryActionBusyKey: state.setRecoveryActionBusyKey,
        setResults: state.setResults,
        setRunSnapshot: state.setRunSnapshot,
        setStageRuns: state.setStageRuns,
        setStageAssertions: state.setStageAssertions,
        setArtifactActionMessage: state.setArtifactActionMessage,
    });

    useEffect(() => {
        dispatchPromptRef.current = dispatchPrompt;
    }, [dispatchPrompt, dispatchPromptRef]);

    const {
        cancelPendingDispatch,
        handleSend,
        handleSuggestionClick,
        handleQuickProgramAction,
        handleTelegramListenerCommand,
        handleKeyDown,
    } = useLauncherUiHandlers({
        input: state.input,
        loading: state.loading,
        isExecutionLocked: state.isExecutionLocked,
        isComposing: state.isComposing,
        selectedIndex: state.selectedIndex,
        pendingDispatch: state.pendingDispatch,
        navigableItems: state.navigableItems,
        inputRef,
        composingSinceRef,
        sendThrottleRef,
        dispatchPrompt,
        handleApprove,
        setInput: state.setInput,
        setIsComposing: state.setIsComposing,
        setSelectedIndex: state.setSelectedIndex,
        setPendingDispatch: state.setPendingDispatch,
        setDispatchBlockedReason: state.setDispatchBlockedReason,
        setDispatchBlockedUntilMs: state.setDispatchBlockedUntilMs,
        setResults: state.setResults,
        setShowDetailPanel: state.setShowDetailPanel,
        setLoading: state.setLoading,
        setRunPhase: state.setRunPhase,
        setPendingApproval: state.setPendingApproval,
        setRecoveryActionBusyKey: state.setRecoveryActionBusyKey,
        triggerSuccess,
        triggerError,
    });

    const {
        handleBackgroundClick,
        handleComposerModeSelect,
        handleCycleComposerMode,
        handleApplyWebSearchTemplate,
        handleApplySummaryTemplate,
    } = useLauncherShell({
        inputRef,
        scrollRef,
        prevComposerModeRef,
        dispatchPromptRef,
        selectedIndex: state.selectedIndex,
        navigableItemsLength: state.navigableItems.length,
        resultsLength: state.results.length,
        suggestionCount: state.suggestionRecs.length,
        composerMode: state.composerMode,
        autoApplyRecommendedProfile: state.autoApplyRecommendedProfile,
        safeExecutionMode: state.safeExecutionMode,
        compactLayoutMode: state.compactLayoutMode,
        showAdvancedControls: state.showAdvancedControls,
        pinnedArtifactKeys: state.pinnedArtifactKeys,
        executionProfile: state.executionProfile,
        dispatchBlockedUntilMs: state.dispatchBlockedUntilMs,
        dispatchNowMs: state.dispatchNowMs,
        pendingDispatch: state.pendingDispatch,
        pendingApprovalVisible: !!state.pendingApproval,
        lastStatus: state.lastStatus,
        lastPlanId: state.lastPlanId,
        runPhase: state.runPhase,
        failedAssertionsCount: state.failedAssertions.length,
        preflightOk: state.preflightOk,
        isExecutionLocked: state.isExecutionLocked,
        showDetailPanel: state.showDetailPanel,
        hasDetailContent: state.hasDetailContent,
        shouldRenderDetailPanel: state.shouldRenderDetailPanel,
        showPreflightPanel: state.showPreflightPanel,
        showPreflightDetail: state.showPreflightDetail,
        preflightError: state.preflightError,
        executionLockHint: state.executionLockHint,
        setResults: state.setResults,
        setShowDetailPanel: state.setShowDetailPanel,
        setAutoApplyRecommendedProfile: state.setAutoApplyRecommendedProfile,
        setExecutionProfile: state.setExecutionProfile,
        setDispatchNowMs: state.setDispatchNowMs,
        setDispatchBlockedUntilMs: state.setDispatchBlockedUntilMs,
        setDispatchBlockedReason: state.setDispatchBlockedReason,
        setPendingDispatch: state.setPendingDispatch,
        setShowDiagnostics: state.setShowDiagnostics,
        setManualChecklist: state.setManualChecklist,
        setSelectedIndex: state.setSelectedIndex,
        setIsComposing: state.setIsComposing,
        setComposerMode: state.setComposerMode,
        setInput: state.setInput,
    });

    return {
        runPreflightCheck,
        handlePreflightFix,
        handlePin,
        openArtifactPath,
        openRecommendationTarget,
        copyTextValue,
        copyArtifactPayload,
        togglePinArtifactKey,
        approvingIds,
        approveErrors,
        n8nOpenBusyKey,
        handleApprove,
        handleResume,
        handleGuidedRecovery,
        runRecoveryAction,
        handleOneClickRecovery,
        handleApprovalDecision,
        cancelPendingDispatch,
        handleSend,
        handleSuggestionClick,
        handleQuickProgramAction,
        handleTelegramListenerCommand,
        handleKeyDown,
        handleBackgroundClick,
        handleComposerModeSelect,
        handleCycleComposerMode,
        handleApplyWebSearchTemplate,
        handleApplySummaryTemplate,
    };
}

export type LauncherActions = ReturnType<typeof useLauncherActions>;
