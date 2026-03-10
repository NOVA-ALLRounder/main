import { useRecommendations } from "@/lib/hooks";
import { useRecommendationProvisioningState } from "@/features/launcher/hooks/useRecommendationProvisioningState";
import { useLauncherDerivedState } from "@/features/launcher/hooks/useLauncherDerivedState";
import { useLauncherDiagnosticsState } from "@/features/launcher/hooks/useLauncherDiagnosticsState";
import { useLauncherExecutionState } from "@/features/launcher/hooks/useLauncherExecutionState";
import { useLauncherState } from "@/features/launcher/hooks/useLauncherState";

export function useLauncherRuntimeView() {
    const state = useLauncherState();
    const execution = useLauncherExecutionState();
    const { data: recs, refetch } = useRecommendations();
    const provisioning = useRecommendationProvisioningState();
    const {
        composingSinceRef,
        lastDispatchRef,
        dispatchPromptRef,
        prevComposerModeRef,
        sendThrottleRef,
        inputRef,
        scrollRef,
        ...stateWithoutRefs
    } = state;
    const diagnostics = useLauncherDiagnosticsState({
        showDiagnostics: state.showDiagnostics,
        activeRunId: execution.runSnapshot?.runId ?? null,
    });

    const derived = useLauncherDerivedState({
        recs,
        watchRecommendationIds: provisioning.watchRecommendationIds,
        watchRecommendationCache: provisioning.watchRecommendationCache,
        provisioningUiByRecId: provisioning.provisioningUiByRecId,
        runtimeInfo: diagnostics.runtimeInfo,
        stageRuns: diagnostics.stageRuns,
        stageAssertions: diagnostics.stageAssertions,
        taskRunArtifacts: diagnostics.taskRunArtifacts,
        artifactTypeFilter: state.artifactTypeFilter,
        artifactFailedOnly: state.artifactFailedOnly,
        artifactSearchQuery: state.artifactSearchQuery,
        artifactSortMode: state.artifactSortMode,
        pinnedArtifactKeys: state.pinnedArtifactKeys,
        showAdvancedControls: state.showAdvancedControls,
        preflightChecks: state.preflightChecks,
        preflightOk: state.preflightOk,
        preflightLoading: state.preflightLoading,
        preflightError: state.preflightError,
        safeExecutionMode: state.safeExecutionMode,
        executionProfile: state.executionProfile,
        pendingApproval: state.pendingApproval,
        lastStatus: execution.lastStatus,
        lastPlanId: execution.lastPlanId,
        runSnapshot: execution.runSnapshot,
        runPhase: execution.runPhase,
        loading: state.loading,
        approvalBusy: state.approvalBusy,
        pendingDispatch: state.pendingDispatch,
        dispatchBlockedReason: state.dispatchBlockedReason,
        dispatchBlockedUntilMs: state.dispatchBlockedUntilMs,
        dispatchNowMs: state.dispatchNowMs,
        composerMode: state.composerMode,
        showDetailPanel: state.showDetailPanel,
        results: state.results,
        selectedIndex: state.selectedIndex,
    });

    const actionRefs = {
        composingSinceRef,
        lastDispatchRef,
        dispatchPromptRef,
        prevComposerModeRef,
        sendThrottleRef,
        inputRef,
        scrollRef,
    };

    const actionState = {
        ...stateWithoutRefs,
        ...execution,
        recs,
        refetch,
        ...provisioning,
        ...diagnostics,
        ...derived,
    };

    return {
        ...actionState,
        ...actionRefs,
        actionState,
        actionRefs,
    };
}

export type LauncherRuntimeView = ReturnType<typeof useLauncherRuntimeView>;
