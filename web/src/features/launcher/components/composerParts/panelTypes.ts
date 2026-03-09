import type {
  ClipboardEventHandler,
  KeyboardEventHandler,
  RefObject,
} from "react";
import type {
  ComposerMode,
  CoreBinaryKind,
  ExecutionProfileOption,
  ProfileRecommendation,
  QuickProgramAction,
  RunScore,
} from "@/features/launcher/support";
import type {
  AgentPreflightCheck,
  ExecutionProfile,
  RuntimeInfo,
} from "@/lib/types";

export type TelegramListenerCommand =
  | "telegram listener start"
  | "telegram listener status";

export type LauncherComposerPanelProps = {
  refs: {
    inputRef: RefObject<HTMLInputElement | null>;
  };
  ui: {
    composerMode: ComposerMode;
    input: string;
    loading: boolean;
    isExecutionLocked: boolean;
    showAdvancedControls: boolean;
    showDetailPanel: boolean;
    hasDetailContent: boolean;
    suggestionCount: number;
    runScore: RunScore | null;
    pendingDispatch: boolean;
    safeCountdownSeconds: number;
    executionLockHint: string | null;
  };
  profile: {
    executionProfileOptions: ExecutionProfileOption[];
    executionProfile: ExecutionProfile;
    safeExecutionMode: boolean;
    compactLayoutMode: boolean;
    autoApplyRecommendedProfile: boolean;
    profileRecommendation: ProfileRecommendation;
    formatProfileLabel: (profile: ExecutionProfile) => string;
  };
  preflight: {
    showPanel: boolean;
    checks: AgentPreflightCheck[];
    ok: boolean | null;
    loading: boolean;
    error: string | null;
    checkedAt: string | null;
    activeApp: string | null;
    showDetail: boolean;
    focusBlocked: boolean;
    accessibility?: AgentPreflightCheck;
    screenCapture?: AgentPreflightCheck;
    fixBusy: string | null;
    fixMessage: string | null;
  };
  quickActions: {
    nlSuggestions: string[];
    chatSuggestions: string[];
    programActions: QuickProgramAction[];
  };
  runtime: {
    info: RuntimeInfo | null;
    coreBinaryKind: CoreBinaryKind;
  };
  handlers: {
    onModeSelect: (mode: ComposerMode) => void;
    onToggleAdvancedControls: () => void;
    onExecutionProfileSelect: (profile: ExecutionProfile) => void;
    onApplyRecommendedProfile: () => void;
    onToggleAutoApplyRecommendedProfile: () => void;
    onToggleSafeExecutionMode: () => void;
    onToggleCompactLayoutMode: () => void;
    onInputChange: (value: string) => void;
    onInputPaste: ClipboardEventHandler<HTMLInputElement>;
    onInputKeyDown: KeyboardEventHandler<HTMLInputElement>;
    onCompositionStart: () => void;
    onCompositionEnd: () => void;
    onInputBlur: () => void;
    onSend: () => void;
    onCancelPendingDispatch: () => void;
    onRunPreflightCheck: () => void;
    onTogglePreflightDetail: () => void;
    onHandlePreflightFix: (action: string) => void;
    onSuggestionClick: (suggestion: string) => void;
    onQuickProgramAction: (action: QuickProgramAction) => void;
    onCycleMode: () => void;
    onApplyWebSearchTemplate: () => void;
    onApplySummaryTemplate: () => void;
    onTelegramListenerCommand: (command: TelegramListenerCommand) => void;
    onToggleDetailPanel: () => void;
  };
};
