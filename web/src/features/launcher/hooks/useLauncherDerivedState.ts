import type {
    AgentPreflightCheck,
    ExecutionProfile,
    Recommendation,
    RuntimeInfo,
    TaskRunArtifact,
    TaskStageAssertion,
    TaskStageRun,
} from "@/lib/types";
import {
    classifyCoreBinary,
    type ApprovalContext,
    type ArtifactSortMode,
    type ComposerMode,
    type CoreBinaryKind,
    type ExecutionSnapshot,
    type LauncherResult,
    type PendingDispatch,
    type ProvisioningUiState,
    type RunPhase,
} from "@/features/launcher/support";
import { deriveArtifactState } from "@/features/launcher/hooks/derived/artifacts";
import {
    deriveSuggestionRecommendations,
    deriveSuggestionRows,
} from "@/features/launcher/hooks/derived/recommendations";
import {
    deriveFocusPreflightState,
    deriveNextActionHint,
    deriveProfileRecommendation,
    deriveRecoveryActionForFailureKey,
    deriveRecoveryActions,
    deriveRunScore,
    deriveShellStatus,
} from "@/features/launcher/hooks/derived/runtimeStatus";

type UseLauncherDerivedStateParams = {
    recs?: Recommendation[];
    watchRecommendationIds: Iterable<number>;
    watchRecommendationCache: Record<number, Recommendation>;
    provisioningUiByRecId: Record<number, ProvisioningUiState>;
    runtimeInfo: RuntimeInfo | null;
    stageRuns: TaskStageRun[];
    stageAssertions: TaskStageAssertion[];
    taskRunArtifacts: TaskRunArtifact[];
    artifactTypeFilter: string;
    artifactFailedOnly: boolean;
    artifactSearchQuery: string;
    artifactSortMode: ArtifactSortMode;
    pinnedArtifactKeys: Set<string>;
    showAdvancedControls: boolean;
    preflightChecks: AgentPreflightCheck[];
    preflightOk: boolean | null;
    preflightLoading: boolean;
    preflightError: string | null;
    safeExecutionMode: boolean;
    executionProfile: ExecutionProfile;
    pendingApproval: ApprovalContext | null;
    lastStatus: string | null;
    lastPlanId: string | null;
    runSnapshot: ExecutionSnapshot | null;
    runPhase: RunPhase;
    loading: boolean;
    approvalBusy: boolean;
    pendingDispatch: PendingDispatch | null;
    dispatchBlockedReason: string | null;
    dispatchBlockedUntilMs: number | null;
    dispatchNowMs: number;
    composerMode: ComposerMode;
    showDetailPanel: boolean;
    results: LauncherResult[];
    selectedIndex: number;
};

export function useLauncherDerivedState({
    recs,
    watchRecommendationIds,
    watchRecommendationCache,
    provisioningUiByRecId,
    runtimeInfo,
    stageRuns,
    stageAssertions,
    taskRunArtifacts,
    artifactTypeFilter,
    artifactFailedOnly,
    artifactSearchQuery,
    artifactSortMode,
    pinnedArtifactKeys,
    showAdvancedControls,
    preflightChecks,
    preflightOk,
    preflightLoading,
    preflightError,
    safeExecutionMode,
    executionProfile,
    pendingApproval,
    lastStatus,
    lastPlanId,
    runSnapshot,
    runPhase,
    loading,
    approvalBusy,
    pendingDispatch,
    dispatchBlockedReason,
    dispatchBlockedUntilMs,
    dispatchNowMs,
    composerMode,
    showDetailPanel,
    results,
    selectedIndex,
}: UseLauncherDerivedStateParams) {
    const suggestionRecs = deriveSuggestionRecommendations({
        recs,
        watchRecommendationIds,
        watchRecommendationCache,
    });

    const coreBinaryKind: CoreBinaryKind = classifyCoreBinary(runtimeInfo?.binary_path);
    const isDevBundleMismatch =
        typeof import.meta !== "undefined" &&
        Boolean(import.meta.env.DEV) &&
        coreBinaryKind === "bundle";

    const {
        failedAssertions,
        stageTraceItems,
        recoveryAssertions,
        failedArtifactKeys,
        artifactTypeOptions,
        artifactGroups,
        firstFailedArtifactPath,
    } = deriveArtifactState({
        stageRuns,
        stageAssertions,
        taskRunArtifacts,
        artifactTypeFilter,
        artifactFailedOnly,
        artifactSearchQuery,
        artifactSortMode,
        pinnedArtifactKeys,
    });

    const focusState = deriveFocusPreflightState(preflightChecks);
    const profileRecommendation = deriveProfileRecommendation({
        preflightOk,
        ...focusState,
    });
    const recoveryActions = deriveRecoveryActions({
        preflightOk,
        failedAssertions,
        firstFailedArtifactPath,
        lastStatus,
        lastPlanId,
        pendingApproval,
        ...focusState,
    });
    const runScore = deriveRunScore({ runSnapshot, failedAssertions });
    const recoveryActionForFailureKey = deriveRecoveryActionForFailureKey;
    const nextActionHint = deriveNextActionHint({
        preflightOk,
        safeExecutionMode,
        executionProfile,
        profileRecommendation,
        pendingApproval,
        lastStatus,
        lastPlanId,
        runSnapshot,
        runPhase,
        failedAssertions,
        ...focusState,
    });
    const {
        currentHud,
        dodItems,
        hasDetailContent,
        shouldRenderDetailPanel,
        isExecutionLocked,
        safeCountdownSeconds,
        executionLockHint,
        showPreflightPanel,
    } = deriveShellStatus({
        runSnapshot,
        runPhase,
        results,
        suggestionCount: suggestionRecs.length,
        pendingApproval,
        lastStatus,
        lastPlanId,
        showDetailPanel,
        composerMode,
        loading,
        approvalBusy,
        safeExecutionMode,
        pendingDispatch,
        dispatchBlockedReason,
        dispatchBlockedUntilMs,
        dispatchNowMs,
        preflightLoading,
        preflightOk,
        preflightError,
        showAdvancedControls,
    });
    const navigableItems = [
        ...results.map((result, idx) => ({ type: "result" as const, data: result, id: `res-${idx}` })),
        ...suggestionRecs.map((rec) => ({
            type: "recommendation" as const,
            data: rec,
            id: `rec-${rec.id}`,
        })),
    ];
    const suggestionRows = deriveSuggestionRows({
        suggestionRecs,
        selectedIndex,
        provisioningUiByRecId,
    });

    return {
        suggestionRecs,
        coreBinaryKind,
        isDevBundleMismatch,
        failedAssertions,
        stageTraceItems,
        recoveryAssertions,
        failedArtifactKeys,
        artifactTypeOptions,
        artifactGroups,
        accessibilityPreflight: focusState.accessibilityPreflight,
        screenCapturePreflight: focusState.screenCapturePreflight,
        focusPreflightBlocked: focusState.focusPreflightBlocked,
        profileRecommendation,
        firstFailedArtifactPath,
        recoveryActions,
        runScore,
        recoveryActionForFailureKey,
        nextActionHint,
        currentHud,
        dodItems,
        hasDetailContent,
        shouldRenderDetailPanel,
        isExecutionLocked,
        safeCountdownSeconds,
        executionLockHint,
        showPreflightPanel,
        navigableItems,
        suggestionRows,
    };
}
