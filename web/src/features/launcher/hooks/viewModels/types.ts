import type { ApprovalRequiredPanelProps } from "@/features/launcher/components/ApprovalRequiredPanel";
import type { DetailSummaryPanelProps } from "@/features/launcher/components/DetailSummaryPanel";
import type { DiagnosticsPanelProps } from "@/features/launcher/components/DiagnosticsPanel";
import type { LauncherComposerPanelProps } from "@/features/launcher/components/LauncherComposerPanel";
import type { LauncherStatusBarProps } from "@/features/launcher/components/LauncherStatusBar";
import type { ManualStepNeededPanelProps } from "@/features/launcher/components/ManualStepNeededPanel";
import type { ResultsFeedPanelProps } from "@/features/launcher/components/ResultsFeedPanel";
import type { LauncherRootProps } from "@/features/launcher/hooks/viewModels/root";

export type ComposerPanelViewModel = LauncherComposerPanelProps;
export type StatusBarViewModel = LauncherStatusBarProps;
export type DetailSummaryViewModel = DetailSummaryPanelProps;
export type DiagnosticsViewModel = DiagnosticsPanelProps;
export type ApprovalViewModel = ApprovalRequiredPanelProps | null;
export type ManualStepViewModel = ManualStepNeededPanelProps | null;
export type ResultsFeedViewModel = Omit<ResultsFeedPanelProps, "markdownComponents">;

export type LauncherViewModel = {
    rootProps: LauncherRootProps;
    detailPanelVisible: boolean;
    composerPanelProps: ComposerPanelViewModel;
    statusBarProps: StatusBarViewModel;
    detailSummaryPanelProps: DetailSummaryViewModel;
    diagnosticsPanelProps: DiagnosticsViewModel;
    approvalPanelProps: ApprovalViewModel;
    manualStepPanelProps: ManualStepViewModel;
    resultsFeedPanelProps: ResultsFeedViewModel;
};
