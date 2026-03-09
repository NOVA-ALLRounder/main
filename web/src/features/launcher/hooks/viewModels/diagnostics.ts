import type { PanelBuilderArgs } from "@/features/launcher/hooks/viewModels/shared";
import type { DiagnosticsViewModel } from "@/features/launcher/hooks/viewModels/types";

export function buildDiagnosticsPanelProps({
    runtime,
    actions,
}: PanelBuilderArgs): DiagnosticsViewModel {
    return {
        visibility: {
            showDiagnostics: runtime.showDiagnostics,
        },
        telemetry: {
            lockMetrics: runtime.lockMetrics,
            lockMetricsError: runtime.lockMetricsError,
            runtimeInfo: runtime.runtimeInfo,
            runtimeInfoError: runtime.runtimeInfoError,
        },
        history: {
            dodHistory: runtime.dodHistory,
            dodHistoryLoading: runtime.dodHistoryLoading,
            dodFailureTop: runtime.dodFailureTop,
            dodItems: runtime.dodItems,
        },
        execution: {
            runPhase: runtime.runPhase,
            lastStatus: runtime.lastStatus,
            firstFailedArtifactPath: runtime.firstFailedArtifactPath,
            loading: runtime.loading,
            artifactOpenBusy: runtime.artifactOpenBusy,
            recoveryActionBusyKey: runtime.recoveryActionBusyKey,
        },
        stage: {
            stageRuns: runtime.stageRuns,
            stageAssertions: runtime.stageAssertions,
            taskRunArtifacts: runtime.taskRunArtifacts,
            artifactActionMessage: runtime.artifactActionMessage,
            recoveryAssertions: runtime.recoveryAssertions,
            stageTraceItems: runtime.stageTraceItems,
            failedAssertions: runtime.failedAssertions,
        },
        artifacts: {
            artifactTypeFilter: runtime.artifactTypeFilter,
            artifactTypeOptions: runtime.artifactTypeOptions,
            artifactFailedOnly: runtime.artifactFailedOnly,
            artifactSortMode: runtime.artifactSortMode,
            artifactSearchQuery: runtime.artifactSearchQuery,
            artifactGroups: runtime.artifactGroups,
            failedArtifactKeys: runtime.failedArtifactKeys,
            pinnedArtifactKeys: runtime.pinnedArtifactKeys,
        },
        handlers: {
            onRecoverFailureKey: runtime.recoveryActionForFailureKey,
            onRunRecoveryAction: (action: (typeof runtime.recoveryActions)[number]) =>
                void actions.runRecoveryAction(action),
            onGuidedRecovery: () => void actions.handleGuidedRecovery(),
            onOpenArtifactPath: (path: string) => void actions.openArtifactPath(path),
            onArtifactTypeFilterChange: runtime.setArtifactTypeFilter,
            onArtifactFailedOnlyToggle: () => runtime.setArtifactFailedOnly((prev) => !prev),
            onArtifactSortModeChange: runtime.setArtifactSortMode,
            onArtifactSearchQueryChange: runtime.setArtifactSearchQuery,
            onTogglePinArtifactKey: actions.togglePinArtifactKey,
            onCopyArtifactPayload: (artifact: (typeof runtime.taskRunArtifacts)[number]) =>
                void actions.copyArtifactPayload(artifact),
        },
    };
}
