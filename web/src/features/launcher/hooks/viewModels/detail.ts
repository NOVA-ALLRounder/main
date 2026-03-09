import type { PanelBuilderArgs } from "@/features/launcher/hooks/viewModels/shared";
import type {
    ApprovalViewModel,
    DetailSummaryViewModel,
    ManualStepViewModel,
} from "@/features/launcher/hooks/viewModels/types";

export function buildDetailSummaryPanelProps({
    runtime,
    actions,
}: PanelBuilderArgs): DetailSummaryViewModel {
    return {
        runPhase: runtime.runPhase,
        runStatus: runtime.runSnapshot?.status,
        runScore: runtime.runScore,
        nextActionHint: runtime.nextActionHint,
        recoveryActions: runtime.recoveryActions,
        loading: runtime.loading,
        recoveryActionBusyKey: runtime.recoveryActionBusyKey,
        preflightFixBusy: runtime.preflightFixBusy,
        artifactOpenBusy: runtime.artifactOpenBusy,
        dodHistoryLoading: runtime.dodHistoryLoading,
        showDiagnostics: runtime.showDiagnostics,
        onOneClickRecovery: () => void actions.handleOneClickRecovery(),
        onRunRecoveryAction: (action: (typeof runtime.recoveryActions)[number]) =>
            void actions.runRecoveryAction(action),
        onLoadDodHistory: () => void runtime.loadDodHistory(),
        onToggleDiagnostics: () => runtime.setShowDiagnostics((prev) => !prev),
    };
}

export function buildApprovalPanelProps({
    runtime,
    actions,
}: PanelBuilderArgs): ApprovalViewModel {
    return runtime.pendingApproval
        ? {
              action: runtime.pendingApproval.action,
              riskLevel: runtime.pendingApproval.riskLevel,
              policy: runtime.pendingApproval.policy,
              message: runtime.pendingApproval.message,
              approvalBusy: runtime.approvalBusy,
              onDecision: actions.handleApprovalDecision,
          }
        : null;
}

export function buildManualStepPanelProps({
    runtime,
    actions,
}: PanelBuilderArgs): ManualStepViewModel {
    return runtime.lastStatus === "manual_required" &&
        runtime.lastPlanId &&
        !runtime.pendingApproval
        ? {
              manualChecklist: runtime.manualChecklist,
              loading: runtime.loading,
              artifactOpenBusy: runtime.artifactOpenBusy,
              firstFailedArtifactPath: runtime.firstFailedArtifactPath,
              onChecklistChange: (
                  key: keyof typeof runtime.manualChecklist,
                  checked: boolean
              ) =>
                  runtime.setManualChecklist((prev) => ({
                      ...prev,
                      [key]: checked,
                  })),
              onResume: actions.handleResume,
              onGuidedRecovery: () => void actions.handleGuidedRecovery(),
          }
        : null;
}
