import { useCallback } from "react";
import type { Dispatch, KeyboardEvent, RefObject, SetStateAction } from "react";

import { sendChatMessage } from "@/lib/api";
import type {
    ApprovalContext,
    LauncherResult,
    PendingDispatch,
    QuickProgramAction,
    RunPhase,
} from "@/features/launcher/support";

type NavigableItem = {
    id: string;
    type?: string;
    data?: unknown;
};

const getRecommendationId = (value: unknown): number | null => {
    if (!value || typeof value !== "object") return null;
    const maybe = value as { id?: unknown };
    return typeof maybe.id === "number" ? maybe.id : null;
};

type UseLauncherUiHandlersParams = {
    input: string;
    loading: boolean;
    isExecutionLocked: boolean;
    isComposing: boolean;
    selectedIndex: number;
    pendingDispatch: PendingDispatch | null;
    navigableItems: NavigableItem[];
    inputRef: RefObject<HTMLInputElement | null>;
    composingSinceRef: RefObject<number>;
    sendThrottleRef: RefObject<number>;
    dispatchPrompt: (prompt: string, bypassSafeCountdown?: boolean) => Promise<void>;
    handleApprove: (id: number) => Promise<void>;
    setInput: Dispatch<SetStateAction<string>>;
    setIsComposing: Dispatch<SetStateAction<boolean>>;
    setSelectedIndex: Dispatch<SetStateAction<number>>;
    setPendingDispatch: Dispatch<SetStateAction<PendingDispatch | null>>;
    setDispatchBlockedReason: Dispatch<SetStateAction<string | null>>;
    setDispatchBlockedUntilMs: Dispatch<SetStateAction<number | null>>;
    setResults: Dispatch<SetStateAction<LauncherResult[]>>;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
    setLoading: Dispatch<SetStateAction<boolean>>;
    setRunPhase: Dispatch<SetStateAction<RunPhase>>;
    setPendingApproval: Dispatch<SetStateAction<ApprovalContext | null>>;
    setRecoveryActionBusyKey: Dispatch<SetStateAction<string | null>>;
    triggerSuccess: () => void;
    triggerError: () => void;
};

export function useLauncherUiHandlers({
    input,
    loading,
    isExecutionLocked,
    isComposing,
    selectedIndex,
    pendingDispatch,
    navigableItems,
    inputRef,
    composingSinceRef,
    sendThrottleRef,
    dispatchPrompt,
    handleApprove,
    setInput,
    setIsComposing,
    setSelectedIndex,
    setPendingDispatch,
    setDispatchBlockedReason,
    setDispatchBlockedUntilMs,
    setResults,
    setShowDetailPanel,
    setLoading,
    setRunPhase,
    setPendingApproval,
    setRecoveryActionBusyKey,
    triggerSuccess,
    triggerError,
}: UseLauncherUiHandlersParams) {
    const cancelPendingDispatch = useCallback(() => {
        if (!pendingDispatch) return;
        setPendingDispatch(null);
        setDispatchBlockedReason("안전 카운트다운 취소");
        setDispatchBlockedUntilMs(null);
        setResults([
            {
                type: "response",
                content: "**안전 실행 취소됨**\n- 자동 실행을 취소했습니다.",
            },
        ]);
        setShowDetailPanel(true);
    }, [
        pendingDispatch,
        setPendingDispatch,
        setDispatchBlockedReason,
        setDispatchBlockedUntilMs,
        setResults,
        setShowDetailPanel,
    ]);

    const handleSend = useCallback(async () => {
        const prompt = input.trim();
        if (!prompt || loading || isExecutionLocked) return;
        if (isComposing) {
            const composingMs = Date.now() - (composingSinceRef.current || Date.now());
            if (composingMs < 1500) return;
            setIsComposing(false);
            composingSinceRef.current = 0;
        }
        if (isComposing) return;
        const now = Date.now();
        if (now - sendThrottleRef.current < 650) return;
        sendThrottleRef.current = now;
        await dispatchPrompt(prompt);
    }, [
        input,
        loading,
        isExecutionLocked,
        isComposing,
        composingSinceRef,
        setIsComposing,
        sendThrottleRef,
        dispatchPrompt,
    ]);

    const handleSuggestionClick = useCallback(
        (suggestion: string) => {
            setIsComposing(false);
            setInput(suggestion);
            inputRef.current?.focus();
        },
        [inputRef, setInput, setIsComposing]
    );

    const handleQuickProgramAction = useCallback(
        async (action: QuickProgramAction) => {
            await dispatchPrompt(action.prompt);
        },
        [dispatchPrompt]
    );

    const handleTelegramListenerCommand = useCallback(
        async (command: "telegram listener start" | "telegram listener status") => {
            if (loading || isExecutionLocked) return;
            setShowDetailPanel(true);
            setLoading(true);
            setRunPhase("running");
            setPendingApproval(null);
            setRecoveryActionBusyKey(null);
            try {
                const res = await sendChatMessage(command);
                setResults([
                    {
                        type: "response",
                        content: [
                            "**Telegram Listener**",
                            `- 요청: \`${command}\``,
                            `- 응답: ${res.response}`,
                        ].join("\n"),
                    },
                ]);
                setRunPhase("completed");
                triggerSuccess();
            } catch (error) {
                const message =
                    error instanceof Error ? error.message : "Telegram listener command failed.";
                setResults([{ type: "error", content: message }]);
                setRunPhase("failed");
                triggerError();
            } finally {
                setLoading(false);
            }
        },
        [
            loading,
            isExecutionLocked,
            setShowDetailPanel,
            setLoading,
            setRunPhase,
            setPendingApproval,
            setRecoveryActionBusyKey,
            setResults,
            triggerSuccess,
            triggerError,
        ]
    );

    const handleKeyDown = useCallback(
        async (e: KeyboardEvent<HTMLInputElement>) => {
            const nativeEvent = e.nativeEvent as globalThis.KeyboardEvent;
            const composingMs = Date.now() - (composingSinceRef.current || Date.now());
            const composingHot = isComposing && composingMs < 1500;
            if (composingHot || nativeEvent.isComposing || nativeEvent.keyCode === 229) {
                return;
            }
            if (isComposing && !composingHot) {
                setIsComposing(false);
                composingSinceRef.current = 0;
            }
            if (isExecutionLocked && e.key === "Enter") {
                e.preventDefault();
                return;
            }
            if (e.key === "ArrowDown") {
                if (navigableItems.length === 0) return;
                e.preventDefault();
                setSelectedIndex((prev) => (prev + 1) % navigableItems.length);
            } else if (e.key === "ArrowUp") {
                if (navigableItems.length === 0) return;
                e.preventDefault();
                setSelectedIndex(
                    (prev) => (prev - 1 + navigableItems.length) % navigableItems.length
                );
            } else if (e.key === "Enter") {
                e.preventDefault();
                if (input.trim()) {
                    await handleSend();
                    return;
                }
                if (navigableItems.length > 0) {
                    const selected = navigableItems[selectedIndex];
                    if (selected && selected.type === "recommendation") {
                        const recommendationId = getRecommendationId(selected.data);
                        if (recommendationId != null) {
                            await handleApprove(recommendationId);
                        }
                    }
                }
            }
        },
        [
            composingSinceRef,
            handleApprove,
            handleSend,
            input,
            isComposing,
            isExecutionLocked,
            navigableItems,
            selectedIndex,
            setIsComposing,
            setSelectedIndex,
        ]
    );

    return {
        cancelPendingDispatch,
        handleSend,
        handleSuggestionClick,
        handleQuickProgramAction,
        handleTelegramListenerCommand,
        handleKeyDown,
    };
}
