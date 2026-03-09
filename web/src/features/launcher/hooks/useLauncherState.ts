import { useRef, useState } from "react";

import type { AgentPreflightCheck, ExecutionProfile } from "@/lib/types";
import type {
    ApprovalContext,
    ArtifactSortMode,
    ComposerMode,
    LauncherResult,
    ManualResumeChecklist,
    PendingDispatch,
} from "@/features/launcher/support";

const loadBooleanPref = (key: string, defaultValue: boolean) => {
    if (typeof window === "undefined") return defaultValue;
    const raw = window.localStorage.getItem(key);
    return raw == null ? defaultValue : raw === "1";
};

const loadPinnedArtifactKeys = () => {
    if (typeof window === "undefined") return new Set<string>();
    const raw = window.localStorage.getItem("steer.artifact_pins");
    if (!raw) return new Set<string>();
    try {
        const parsed = JSON.parse(raw) as string[];
        if (!Array.isArray(parsed)) return new Set<string>();
        return new Set(parsed.filter((x) => typeof x === "string"));
    } catch {
        return new Set<string>();
    }
};

export function useLauncherState() {
    const [input, setInput] = useState("");
    const [isComposing, setIsComposing] = useState(false);
    const [composerMode, setComposerMode] = useState<ComposerMode>("nl");
    const [executionProfile, setExecutionProfile] =
        useState<ExecutionProfile>("strict");
    const [activeExecutionProfile, setActiveExecutionProfile] =
        useState<ExecutionProfile>("strict");
    const [autoApplyRecommendedProfile, setAutoApplyRecommendedProfile] =
        useState<boolean>(() => loadBooleanPref("steer.auto_profile_apply", true));
    const [safeExecutionMode, setSafeExecutionMode] =
        useState<boolean>(() => loadBooleanPref("steer.safe_execution_mode", true));
    const [compactLayoutMode, setCompactLayoutMode] =
        useState<boolean>(() => loadBooleanPref("steer.compact_layout_mode", true));
    const [showAdvancedControls, setShowAdvancedControls] = useState(false);
    const [showDetailPanel, setShowDetailPanel] = useState(false);
    const [results, setResults] = useState<LauncherResult[]>([]);
    const [loading, setLoading] = useState(false);
    const [selectedIndex, setSelectedIndex] = useState(0);
    const [successPulse, setSuccessPulse] = useState(false);
    const [shake, setShake] = useState(false);
    const [pendingApproval, setPendingApproval] = useState<ApprovalContext | null>(null);
    const [approvalBusy, setApprovalBusy] = useState(false);
    const [dispatchBlockedReason, setDispatchBlockedReason] = useState<string | null>(null);
    const [dispatchBlockedUntilMs, setDispatchBlockedUntilMs] = useState<number | null>(null);
    const [dispatchNowMs, setDispatchNowMs] = useState<number>(Date.now());
    const [pendingDispatch, setPendingDispatch] = useState<PendingDispatch | null>(null);
    const [goalRunAvailable, setGoalRunAvailable] = useState<boolean | null>(null);
    const [artifactTypeFilter, setArtifactTypeFilter] = useState<string>("all");
    const [artifactFailedOnly, setArtifactFailedOnly] = useState(false);
    const [artifactSearchQuery, setArtifactSearchQuery] = useState("");
    const [artifactSortMode, setArtifactSortMode] =
        useState<ArtifactSortMode>("failed_first");
    const [pinnedArtifactKeys, setPinnedArtifactKeys] =
        useState<Set<string>>(loadPinnedArtifactKeys);
    const [preflightChecks, setPreflightChecks] = useState<AgentPreflightCheck[]>([]);
    const [preflightOk, setPreflightOk] = useState<boolean | null>(null);
    const [preflightLoading, setPreflightLoading] = useState(false);
    const [preflightError, setPreflightError] = useState<string | null>(null);
    const [preflightCheckedAt, setPreflightCheckedAt] = useState<string | null>(null);
    const [preflightActiveApp, setPreflightActiveApp] = useState<string | null>(null);
    const [showPreflightDetail, setShowPreflightDetail] = useState(false);
    const [preflightFixBusy, setPreflightFixBusy] = useState<string | null>(null);
    const [preflightFixMessage, setPreflightFixMessage] = useState<string | null>(null);
    const [showDiagnostics, setShowDiagnostics] = useState(false);
    const [artifactOpenBusy, setArtifactOpenBusy] = useState<string | null>(null);
    const [artifactActionMessage, setArtifactActionMessage] = useState<string | null>(null);
    const [recoveryActionBusyKey, setRecoveryActionBusyKey] = useState<string | null>(null);
    const [manualChecklist, setManualChecklist] = useState<ManualResumeChecklist>({
        focusReady: false,
        manualStepDone: false,
        handsOffReady: false,
    });

    const composingSinceRef = useRef<number>(0);
    const lastDispatchRef = useRef<{ promptKey: string; ts: number } | null>(null);
    const dispatchPromptRef = useRef<
        ((rawPrompt: string, bypassSafeCountdown?: boolean) => Promise<void>) | null
    >(null);
    const prevComposerModeRef = useRef<ComposerMode>("nl");
    const sendThrottleRef = useRef<number>(0);
    const inputRef = useRef<HTMLInputElement>(null);
    const scrollRef = useRef<HTMLDivElement>(null);

    return {
        input,
        setInput,
        isComposing,
        setIsComposing,
        composerMode,
        setComposerMode,
        executionProfile,
        setExecutionProfile,
        activeExecutionProfile,
        setActiveExecutionProfile,
        autoApplyRecommendedProfile,
        setAutoApplyRecommendedProfile,
        safeExecutionMode,
        setSafeExecutionMode,
        compactLayoutMode,
        setCompactLayoutMode,
        showAdvancedControls,
        setShowAdvancedControls,
        showDetailPanel,
        setShowDetailPanel,
        results,
        setResults,
        loading,
        setLoading,
        selectedIndex,
        setSelectedIndex,
        successPulse,
        setSuccessPulse,
        shake,
        setShake,
        pendingApproval,
        setPendingApproval,
        approvalBusy,
        setApprovalBusy,
        dispatchBlockedReason,
        setDispatchBlockedReason,
        dispatchBlockedUntilMs,
        setDispatchBlockedUntilMs,
        dispatchNowMs,
        setDispatchNowMs,
        pendingDispatch,
        setPendingDispatch,
        goalRunAvailable,
        setGoalRunAvailable,
        artifactTypeFilter,
        setArtifactTypeFilter,
        artifactFailedOnly,
        setArtifactFailedOnly,
        artifactSearchQuery,
        setArtifactSearchQuery,
        artifactSortMode,
        setArtifactSortMode,
        pinnedArtifactKeys,
        setPinnedArtifactKeys,
        preflightChecks,
        setPreflightChecks,
        preflightOk,
        setPreflightOk,
        preflightLoading,
        setPreflightLoading,
        preflightError,
        setPreflightError,
        preflightCheckedAt,
        setPreflightCheckedAt,
        preflightActiveApp,
        setPreflightActiveApp,
        showPreflightDetail,
        setShowPreflightDetail,
        preflightFixBusy,
        setPreflightFixBusy,
        preflightFixMessage,
        setPreflightFixMessage,
        showDiagnostics,
        setShowDiagnostics,
        artifactOpenBusy,
        setArtifactOpenBusy,
        artifactActionMessage,
        setArtifactActionMessage,
        recoveryActionBusyKey,
        setRecoveryActionBusyKey,
        manualChecklist,
        setManualChecklist,
        composingSinceRef,
        lastDispatchRef,
        dispatchPromptRef,
        prevComposerModeRef,
        sendThrottleRef,
        inputRef,
        scrollRef,
    };
}
