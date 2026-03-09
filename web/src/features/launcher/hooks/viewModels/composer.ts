import {
    EXECUTION_PROFILE_OPTIONS,
    QUICK_CHAT_SUGGESTIONS,
    QUICK_NL_SUGGESTIONS,
    QUICK_PROGRAM_ACTIONS,
    profileLabel,
} from "@/features/launcher/support";
import type { PanelBuilderArgs } from "@/features/launcher/hooks/viewModels/shared";
import type {
    ComposerPanelViewModel,
    StatusBarViewModel,
} from "@/features/launcher/hooks/viewModels/types";

export function buildComposerPanelProps({
    runtime,
    actions,
}: PanelBuilderArgs): ComposerPanelViewModel {
    return {
        refs: {
            inputRef: runtime.inputRef,
        },
        ui: {
            composerMode: runtime.composerMode,
            input: runtime.input,
            loading: runtime.loading,
            isExecutionLocked: runtime.isExecutionLocked,
            showAdvancedControls: runtime.showAdvancedControls,
            showDetailPanel: runtime.showDetailPanel,
            hasDetailContent: runtime.hasDetailContent,
            suggestionCount: runtime.suggestionRecs.length,
            runScore: runtime.runScore,
            pendingDispatch: !!runtime.pendingDispatch,
            safeCountdownSeconds: runtime.safeCountdownSeconds,
            executionLockHint: runtime.executionLockHint,
        },
        profile: {
            executionProfileOptions: EXECUTION_PROFILE_OPTIONS,
            executionProfile: runtime.executionProfile,
            safeExecutionMode: runtime.safeExecutionMode,
            compactLayoutMode: runtime.compactLayoutMode,
            autoApplyRecommendedProfile: runtime.autoApplyRecommendedProfile,
            profileRecommendation: runtime.profileRecommendation,
            formatProfileLabel: profileLabel,
        },
        preflight: {
            showPanel: runtime.showPreflightPanel,
            checks: runtime.preflightChecks,
            ok: runtime.preflightOk,
            loading: runtime.preflightLoading,
            error: runtime.preflightError,
            checkedAt: runtime.preflightCheckedAt,
            activeApp: runtime.preflightActiveApp,
            showDetail: runtime.showPreflightDetail,
            focusBlocked: runtime.focusPreflightBlocked,
            accessibility: runtime.accessibilityPreflight,
            screenCapture: runtime.screenCapturePreflight,
            fixBusy: runtime.preflightFixBusy,
            fixMessage: runtime.preflightFixMessage,
        },
        quickActions: {
            nlSuggestions: runtime.compactLayoutMode
                ? QUICK_NL_SUGGESTIONS.slice(0, 2)
                : QUICK_NL_SUGGESTIONS,
            chatSuggestions: runtime.compactLayoutMode
                ? QUICK_CHAT_SUGGESTIONS.slice(0, 2)
                : QUICK_CHAT_SUGGESTIONS,
            programActions: runtime.compactLayoutMode
                ? QUICK_PROGRAM_ACTIONS.slice(0, 4)
                : QUICK_PROGRAM_ACTIONS,
        },
        runtime: {
            info: runtime.runtimeInfo,
            coreBinaryKind: runtime.coreBinaryKind,
        },
        handlers: {
            onModeSelect: actions.handleComposerModeSelect,
            onToggleAdvancedControls: () =>
                runtime.setShowAdvancedControls((prev) => !prev),
            onExecutionProfileSelect: runtime.setExecutionProfile,
            onApplyRecommendedProfile: () =>
                runtime.setExecutionProfile(runtime.profileRecommendation.profile),
            onToggleAutoApplyRecommendedProfile: () =>
                runtime.setAutoApplyRecommendedProfile((prev) => !prev),
            onToggleSafeExecutionMode: () =>
                runtime.setSafeExecutionMode((prev) => !prev),
            onToggleCompactLayoutMode: () =>
                runtime.setCompactLayoutMode((prev) => !prev),
            onInputChange: runtime.setInput,
            onInputPaste: actions.handleInputPaste,
            onInputKeyDown: actions.handleKeyDown,
            onCompositionStart: () => {
                runtime.composingSinceRef.current = Date.now();
                runtime.setIsComposing(true);
            },
            onCompositionEnd: () => {
                runtime.composingSinceRef.current = 0;
                runtime.setIsComposing(false);
            },
            onInputBlur: () => runtime.setIsComposing(false),
            onSend: actions.handleSend,
            onCancelPendingDispatch: actions.cancelPendingDispatch,
            onRunPreflightCheck: () => void actions.runPreflightCheck(false),
            onTogglePreflightDetail: () =>
                runtime.setShowPreflightDetail((prev) => !prev),
            onHandlePreflightFix: (action: string) =>
                void actions.handlePreflightFix(action),
            onSuggestionClick: actions.handleSuggestionClick,
            onQuickProgramAction: (action: (typeof QUICK_PROGRAM_ACTIONS)[number]) =>
                void actions.handleQuickProgramAction(action),
            onCycleMode: actions.handleCycleComposerMode,
            onApplyWebSearchTemplate: actions.handleApplyWebSearchTemplate,
            onApplySummaryTemplate: actions.handleApplySummaryTemplate,
            onTelegramListenerCommand: (command: "telegram listener start" | "telegram listener status") =>
                void actions.handleTelegramListenerCommand(command),
            onToggleDetailPanel: () => runtime.setShowDetailPanel((prev) => !prev),
        },
    };
}

export function buildStatusBarProps({
    runtime,
    actions,
}: PanelBuilderArgs): StatusBarViewModel {
    return {
        currentHud: runtime.currentHud,
        runId: runtime.runSnapshot?.runId ?? null,
        runStatus: runtime.runSnapshot?.status ?? null,
        runPhase: runtime.runPhase,
        safeExecutionMode: runtime.safeExecutionMode,
        runtimeInfoError: runtime.runtimeInfoError,
        isDevBundleMismatch: runtime.isDevBundleMismatch,
        onCopyRunId: (runId: string) => void actions.copyTextValue(runId, "run_id"),
    };
}
