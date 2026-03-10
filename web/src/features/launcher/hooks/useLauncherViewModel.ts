import { useLauncherActions } from "@/features/launcher/hooks/useLauncherActions";
import {
    buildApprovalPanelProps,
    buildComposerPanelProps,
    buildDetailSummaryPanelProps,
    buildDiagnosticsPanelProps,
    buildLauncherRootProps,
    buildManualStepPanelProps,
    buildResultsFeedPanelProps,
    buildStatusBarProps,
} from "@/features/launcher/hooks/panelViewModels";
import type { LauncherViewModel } from "@/features/launcher/hooks/viewModels/types";
import { useLauncherRuntimeView } from "@/features/launcher/hooks/useLauncherRuntimeView";

export function useLauncherViewModel(): LauncherViewModel {
    const runtime = useLauncherRuntimeView();
    const actions = useLauncherActions(runtime.actionState, runtime.actionRefs);
    const panelArgs = { runtime, actions };

    return {
        rootProps: buildLauncherRootProps(panelArgs),
        detailPanelVisible: runtime.shouldRenderDetailPanel,
        composerPanelProps: buildComposerPanelProps(panelArgs),
        statusBarProps: buildStatusBarProps(panelArgs),
        detailSummaryPanelProps: buildDetailSummaryPanelProps(panelArgs),
        diagnosticsPanelProps: buildDiagnosticsPanelProps(panelArgs),
        approvalPanelProps: buildApprovalPanelProps(panelArgs),
        manualStepPanelProps: buildManualStepPanelProps(panelArgs),
        resultsFeedPanelProps: buildResultsFeedPanelProps(panelArgs),
    };
}
