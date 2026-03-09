import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useCallback, useEffect, type Dispatch, type MutableRefObject, type RefObject, type SetStateAction } from "react";
import type { ExecutionProfile } from "@/lib/types";
import type {
    ComposerMode,
    LauncherResult,
    ManualResumeChecklist,
    PendingDispatch,
    RunPhase,
} from "@/features/launcher/support";

type WindowWithTauriMeta = Window & {
    __TAURI_METADATA__?: unknown;
    __TAURI__?: { metadata?: unknown };
    __TAURI_INTERNALS__?: { metadata?: unknown };
};

type UseLauncherShellParams = {
    inputRef: RefObject<HTMLInputElement | null>;
    scrollRef: RefObject<HTMLDivElement | null>;
    prevComposerModeRef: MutableRefObject<ComposerMode>;
    dispatchPromptRef: MutableRefObject<
        ((rawPrompt: string, bypassSafeCountdown?: boolean) => Promise<void>) | null
    >;
    selectedIndex: number;
    navigableItemsLength: number;
    resultsLength: number;
    suggestionCount: number;
    composerMode: ComposerMode;
    autoApplyRecommendedProfile: boolean;
    safeExecutionMode: boolean;
    compactLayoutMode: boolean;
    showAdvancedControls: boolean;
    pinnedArtifactKeys: Set<string>;
    executionProfile: ExecutionProfile;
    dispatchBlockedUntilMs: number | null;
    dispatchNowMs: number;
    pendingDispatch: PendingDispatch | null;
    pendingApprovalVisible: boolean;
    lastStatus: string | null;
    lastPlanId: string | null;
    runPhase: RunPhase;
    failedAssertionsCount: number;
    preflightOk: boolean | null;
    isExecutionLocked: boolean;
    showDetailPanel: boolean;
    hasDetailContent: boolean;
    shouldRenderDetailPanel: boolean;
    showPreflightPanel: boolean;
    showPreflightDetail: boolean;
    preflightError: string | null;
    executionLockHint: string | null;
    setResults: Dispatch<SetStateAction<LauncherResult[]>>;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
    setAutoApplyRecommendedProfile: Dispatch<SetStateAction<boolean>>;
    setExecutionProfile: Dispatch<SetStateAction<ExecutionProfile>>;
    setDispatchNowMs: Dispatch<SetStateAction<number>>;
    setDispatchBlockedUntilMs: Dispatch<SetStateAction<number | null>>;
    setDispatchBlockedReason: Dispatch<SetStateAction<string | null>>;
    setPendingDispatch: Dispatch<SetStateAction<PendingDispatch | null>>;
    setShowDiagnostics: Dispatch<SetStateAction<boolean>>;
    setManualChecklist: Dispatch<SetStateAction<ManualResumeChecklist>>;
    setSelectedIndex: Dispatch<SetStateAction<number>>;
    setIsComposing: Dispatch<SetStateAction<boolean>>;
    setComposerMode: Dispatch<SetStateAction<ComposerMode>>;
    setInput: Dispatch<SetStateAction<string>>;
};

export function useLauncherShell({
    inputRef,
    scrollRef,
    prevComposerModeRef,
    dispatchPromptRef,
    selectedIndex,
    navigableItemsLength,
    resultsLength,
    suggestionCount,
    composerMode,
    autoApplyRecommendedProfile,
    safeExecutionMode,
    compactLayoutMode,
    showAdvancedControls,
    pinnedArtifactKeys,
    executionProfile,
    dispatchBlockedUntilMs,
    dispatchNowMs,
    pendingDispatch,
    pendingApprovalVisible,
    lastStatus,
    lastPlanId,
    runPhase,
    failedAssertionsCount,
    preflightOk,
    isExecutionLocked,
    showDetailPanel,
    hasDetailContent,
    shouldRenderDetailPanel,
    showPreflightPanel,
    showPreflightDetail,
    preflightError,
    executionLockHint,
    setResults,
    setShowDetailPanel,
    setAutoApplyRecommendedProfile,
    setExecutionProfile,
    setDispatchNowMs,
    setDispatchBlockedUntilMs,
    setDispatchBlockedReason,
    setPendingDispatch,
    setShowDiagnostics,
    setManualChecklist,
    setSelectedIndex,
    setIsComposing,
    setComposerMode,
    setInput,
}: UseLauncherShellParams) {
    useEffect(() => {
        inputRef.current?.focus();
    }, [inputRef]);

    useEffect(() => {
        if (composerMode === "chat" && prevComposerModeRef.current !== "chat") {
            setResults([]);
            setShowDetailPanel(true);
        }
        prevComposerModeRef.current = composerMode;
    }, [composerMode, prevComposerModeRef, setResults, setShowDetailPanel]);

    useEffect(() => {
        if (typeof window === "undefined") return;
        window.localStorage.setItem(
            "steer.auto_profile_apply",
            autoApplyRecommendedProfile ? "1" : "0"
        );
    }, [autoApplyRecommendedProfile]);

    useEffect(() => {
        if (typeof window === "undefined") return;
        window.localStorage.setItem(
            "steer.safe_execution_mode",
            safeExecutionMode ? "1" : "0"
        );
    }, [safeExecutionMode]);

    useEffect(() => {
        if (typeof window === "undefined") return;
        window.localStorage.setItem(
            "steer.compact_layout_mode",
            compactLayoutMode ? "1" : "0"
        );
    }, [compactLayoutMode]);

    useEffect(() => {
        if (typeof window === "undefined") return;
        window.localStorage.setItem(
            "steer.launcher_show_advanced",
            showAdvancedControls ? "1" : "0"
        );
    }, [showAdvancedControls]);

    useEffect(() => {
        if (typeof window === "undefined") return;
        window.localStorage.setItem(
            "steer.artifact_pins",
            JSON.stringify(Array.from(pinnedArtifactKeys))
        );
    }, [pinnedArtifactKeys]);

    useEffect(() => {
        if (!safeExecutionMode) return;
        if (executionProfile !== "strict") {
            setExecutionProfile("strict");
        }
        if (!autoApplyRecommendedProfile) {
            setAutoApplyRecommendedProfile(true);
        }
    }, [
        safeExecutionMode,
        executionProfile,
        autoApplyRecommendedProfile,
        setExecutionProfile,
        setAutoApplyRecommendedProfile,
    ]);

    useEffect(() => {
        if (!dispatchBlockedUntilMs) return;
        const timer = window.setInterval(() => {
            setDispatchNowMs(Date.now());
        }, 1000);
        return () => window.clearInterval(timer);
    }, [dispatchBlockedUntilMs, setDispatchNowMs]);

    useEffect(() => {
        if (!dispatchBlockedUntilMs) return;
        if (Date.now() >= dispatchBlockedUntilMs) {
            setDispatchBlockedUntilMs(null);
            setDispatchBlockedReason(null);
        }
    }, [dispatchBlockedUntilMs, dispatchNowMs, setDispatchBlockedReason, setDispatchBlockedUntilMs]);

    useEffect(() => {
        if (composerMode === "chat") {
            if (runPhase === "completed" || runPhase === "running") {
                setShowDetailPanel(true);
            }
            return;
        }
        if (
            pendingApprovalVisible ||
            lastStatus === "manual_required" ||
            runPhase === "failed" ||
            failedAssertionsCount > 0
        ) {
            setShowDetailPanel(true);
            return;
        }
        if (runPhase === "completed" && failedAssertionsCount === 0 && !pendingApprovalVisible) {
            setShowDetailPanel(false);
        }
    }, [
        composerMode,
        pendingApprovalVisible,
        lastStatus,
        runPhase,
        failedAssertionsCount,
        setShowDetailPanel,
    ]);

    useEffect(() => {
        if (lastStatus === "manual_required") {
            setManualChecklist({
                focusReady: false,
                manualStepDone: false,
                handsOffReady: false,
            });
        }
    }, [lastStatus, lastPlanId, setManualChecklist]);

    useEffect(() => {
        if (!pendingDispatch) return;
        if (dispatchNowMs < pendingDispatch.executeAtMs) return;
        const prompt = pendingDispatch.prompt;
        setPendingDispatch(null);
        setDispatchBlockedReason(null);
        setDispatchBlockedUntilMs(null);
        void dispatchPromptRef.current?.(prompt, true);
    }, [
        pendingDispatch,
        dispatchNowMs,
        dispatchPromptRef,
        setPendingDispatch,
        setDispatchBlockedReason,
        setDispatchBlockedUntilMs,
    ]);

    useEffect(() => {
        if (failedAssertionsCount > 0 || preflightOk === false) {
            setShowDiagnostics(true);
        }
    }, [failedAssertionsCount, preflightOk, setShowDiagnostics]);

    useEffect(() => {
        const tauriMeta =
            (window as WindowWithTauriMeta).__TAURI_METADATA__ ||
            (window as WindowWithTauriMeta).__TAURI__?.metadata ||
            (window as WindowWithTauriMeta).__TAURI_INTERNALS__?.metadata;
        if (!tauriMeta) {
            return;
        }

        const showOperationalPanels = composerMode !== "chat";
        const isExpanded = shouldRenderDetailPanel;
        const preflightExpanded =
            showOperationalPanels &&
            showPreflightPanel &&
            (showPreflightDetail || showAdvancedControls || preflightOk === false || !!preflightError || !!executionLockHint);
        const desiredWidth = compactLayoutMode ? 1080 : 1240;
        const maxViewportWidth =
            typeof window !== "undefined"
                ? Math.max(960, (window.screen?.availWidth ?? window.innerWidth) - 24)
                : desiredWidth;
        const targetWidth = Math.min(desiredWidth, maxViewportWidth);
        const targetHeight = isExpanded
            ? compactLayoutMode
                ? 640
                : 720
            : preflightExpanded
              ? compactLayoutMode
                  ? 252
                  : 292
              : compactLayoutMode
                ? 182
                : 214;

        void getCurrentWindow()
            .setSize(new LogicalSize(targetWidth, targetHeight))
            .catch((error) => {
                console.error("Failed to sync launcher window size:", error);
            });
    }, [
        showDetailPanel,
        hasDetailContent,
        shouldRenderDetailPanel,
        showPreflightPanel,
        showPreflightDetail,
        showAdvancedControls,
        preflightOk,
        preflightError,
        executionLockHint,
        compactLayoutMode,
        composerMode,
    ]);

    useEffect(() => {
        setSelectedIndex(0);
    }, [resultsLength, suggestionCount, setSelectedIndex]);

    useEffect(() => {
        if (scrollRef.current && navigableItemsLength > 0) {
            const selectedElement = scrollRef.current.children[selectedIndex];
            if (selectedElement) {
                selectedElement.scrollIntoView({
                    behavior: "smooth",
                    block: "nearest",
                });
            }
        }
    }, [scrollRef, selectedIndex, navigableItemsLength]);

    const handleBackgroundClick = useCallback(
        async (e: React.MouseEvent) => {
            if (e.target === e.currentTarget) {
                if (isExecutionLocked) return;
                try {
                    const tauriMeta =
                        (window as WindowWithTauriMeta).__TAURI_METADATA__ ||
                        (window as WindowWithTauriMeta).__TAURI__?.metadata ||
                        (window as WindowWithTauriMeta).__TAURI_INTERNALS__?.metadata;
                    if (tauriMeta) {
                        await getCurrentWindow().hide();
                    }
                } catch (error) {
                    console.error("Failed to hide window:", error);
                }
            }
        },
        [isExecutionLocked]
    );

    const handleComposerModeSelect = useCallback(
        (mode: ComposerMode) => {
            setIsComposing(false);
            setComposerMode(mode);
            if (mode === "chat") {
                setShowDetailPanel(true);
            }
        },
        [setComposerMode, setIsComposing, setShowDetailPanel]
    );

    const handleCycleComposerMode = useCallback(() => {
        setComposerMode((prev) => (prev === "nl" ? "chat" : prev === "chat" ? "program" : "nl"));
    }, [setComposerMode]);

    const handleApplyWebSearchTemplate = useCallback(() => {
        setComposerMode("nl");
        setInput((prev) => (prev.trim() ? `웹 검색: ${prev}` : "웹 검색: "));
        inputRef.current?.focus();
    }, [inputRef, setComposerMode, setInput]);

    const handleApplySummaryTemplate = useCallback(() => {
        setComposerMode("nl");
        setInput((prev) => (prev.trim() ? `요약해줘: ${prev}` : "요약해줘: "));
        inputRef.current?.focus();
    }, [inputRef, setComposerMode, setInput]);

    return {
        handleBackgroundClick,
        handleComposerModeSelect,
        handleCycleComposerMode,
        handleApplyWebSearchTemplate,
        handleApplySummaryTemplate,
    };
}
