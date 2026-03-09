import type { RecoveryAction, RunPhase } from "@/features/launcher/support";
import type { RunScoreLike } from "@/features/launcher/components/detailSummaryParts/sharedTypes";

export type DetailSummaryPanelProps = {
  runPhase: RunPhase;
  runStatus?: string | null;
  runScore: RunScoreLike;
  nextActionHint: string;
  recoveryActions: RecoveryAction[];
  loading: boolean;
  recoveryActionBusyKey: string | null;
  preflightFixBusy: string | null;
  artifactOpenBusy: string | null;
  dodHistoryLoading: boolean;
  showDiagnostics: boolean;
  onOneClickRecovery: () => void;
  onRunRecoveryAction: (action: RecoveryAction) => void;
  onLoadDodHistory: () => void;
  onToggleDiagnostics: () => void;
};
