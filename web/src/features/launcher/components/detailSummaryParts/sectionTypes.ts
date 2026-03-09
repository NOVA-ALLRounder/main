import type { DetailSummaryPanelProps } from "@/features/launcher/components/detailSummaryParts/panelTypes";

export type SummaryCardsSectionProps = Pick<
  DetailSummaryPanelProps,
  "runPhase" | "runStatus" | "runScore" | "nextActionHint"
>;

export type RecoveryActionsSectionProps = Pick<
  DetailSummaryPanelProps,
  | "recoveryActions"
  | "loading"
  | "recoveryActionBusyKey"
  | "preflightFixBusy"
  | "artifactOpenBusy"
  | "onOneClickRecovery"
  | "onRunRecoveryAction"
>;

export type DetailSummaryFooterProps = Pick<
  DetailSummaryPanelProps,
  "dodHistoryLoading" | "showDiagnostics" | "onLoadDodHistory" | "onToggleDiagnostics"
>;
