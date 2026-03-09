import type {
  LockMetrics,
  RuntimeInfo,
  TaskRunArtifact,
  TaskStageAssertion,
  TaskStageRun,
} from "@/lib/types";
import type {
  ArtifactGroupItem,
  ArtifactSortMode,
  DodFailureTopItem,
  DodHistoryItem,
  RecoveryAction,
  RunPhase,
  StageTraceItem,
} from "@/features/launcher/support";
import type { DodItemLike } from "@/features/launcher/components/diagnosticsParts/sharedTypes";

export type DiagnosticsPanelProps = {
  visibility: {
    showDiagnostics: boolean;
  };
  telemetry: {
    lockMetrics: LockMetrics | null;
    lockMetricsError: string | null;
    runtimeInfo: RuntimeInfo | null;
    runtimeInfoError: string | null;
  };
  history: {
    dodHistory: DodHistoryItem[];
    dodHistoryLoading: boolean;
    dodFailureTop: DodFailureTopItem[];
    dodItems: DodItemLike[];
  };
  execution: {
    runPhase: RunPhase;
    lastStatus: string | null;
    firstFailedArtifactPath: string | null;
    loading: boolean;
    artifactOpenBusy: string | null;
    recoveryActionBusyKey: string | null;
  };
  stage: {
    stageRuns: TaskStageRun[];
    stageAssertions: TaskStageAssertion[];
    taskRunArtifacts: TaskRunArtifact[];
    artifactActionMessage: string | null;
    recoveryAssertions: TaskStageAssertion[];
    stageTraceItems: StageTraceItem[];
    failedAssertions: TaskStageAssertion[];
  };
  artifacts: {
    artifactTypeFilter: string;
    artifactTypeOptions: string[];
    artifactFailedOnly: boolean;
    artifactSortMode: ArtifactSortMode;
    artifactSearchQuery: string;
    artifactGroups: ArtifactGroupItem[];
    failedArtifactKeys: Set<string>;
    pinnedArtifactKeys: Set<string>;
  };
  handlers: {
    onRecoverFailureKey: (failureKey: string) => RecoveryAction | null;
    onRunRecoveryAction: (action: RecoveryAction) => void;
    onGuidedRecovery: () => void;
    onOpenArtifactPath: (path: string) => void;
    onArtifactTypeFilterChange: (value: string) => void;
    onArtifactFailedOnlyToggle: () => void;
    onArtifactSortModeChange: (value: ArtifactSortMode) => void;
    onArtifactSearchQueryChange: (value: string) => void;
    onTogglePinArtifactKey: (artifactKey: string) => void;
    onCopyArtifactPayload: (artifact: TaskRunArtifact) => void;
  };
};
