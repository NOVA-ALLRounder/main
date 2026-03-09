import { useCallback, useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import axios from "axios";

import { fetchAgentPreflight, runAgentPreflightFix } from "@/lib/api";
import type { AgentPreflightCheck } from "@/lib/types";
import type { LauncherResult } from "@/features/launcher/support";
import { preflightPermissionHint } from "@/features/launcher/support";

type UseLauncherPreflightParams = {
    currentRunId: string | null;
    preflightOk: boolean | null;
    loadRunDiagnostics: (runId?: string | null) => Promise<void>;
    loadDodHistory: () => Promise<void>;
    setPreflightChecks: Dispatch<SetStateAction<AgentPreflightCheck[]>>;
    setPreflightOk: Dispatch<SetStateAction<boolean | null>>;
    setPreflightLoading: Dispatch<SetStateAction<boolean>>;
    setPreflightError: Dispatch<SetStateAction<string | null>>;
    setPreflightCheckedAt: Dispatch<SetStateAction<string | null>>;
    setPreflightActiveApp: Dispatch<SetStateAction<string | null>>;
    setShowPreflightDetail: Dispatch<SetStateAction<boolean>>;
    setPreflightFixBusy: Dispatch<SetStateAction<string | null>>;
    setPreflightFixMessage: Dispatch<SetStateAction<string | null>>;
    setResults: Dispatch<SetStateAction<LauncherResult[]>>;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
};

export function useLauncherPreflight({
    currentRunId,
    preflightOk,
    loadRunDiagnostics,
    loadDodHistory,
    setPreflightChecks,
    setPreflightOk,
    setPreflightLoading,
    setPreflightError,
    setPreflightCheckedAt,
    setPreflightActiveApp,
    setShowPreflightDetail,
    setPreflightFixBusy,
    setPreflightFixMessage,
    setResults,
    setShowDetailPanel,
}: UseLauncherPreflightParams) {
    const runPreflightCheck = useCallback(
        async (silent: boolean = false): Promise<boolean> => {
            setPreflightLoading(true);
            setPreflightError(null);
            if (!silent) {
                setPreflightFixMessage(null);
            }
            try {
                const preflight = await fetchAgentPreflight();
                setPreflightChecks(preflight.checks);
                setPreflightOk(preflight.ok);
                setPreflightCheckedAt(preflight.checked_at);
                setPreflightActiveApp(preflight.active_app ?? null);
                if (!preflight.ok) {
                    setShowPreflightDetail(true);
                    if (!silent) {
                        setResults([
                            {
                                type: "error",
                                content:
                                    "실행 전 점검 실패로 실행을 차단했습니다.\n- Accessibility/Screen/Focus 상태를 확인한 뒤 다시 실행하세요.\n- Focus Handoff가 실패하면 전용 데스크톱(또는 다른 사용자 세션)에서 실행하세요.",
                            },
                        ]);
                    }
                } else if (!silent) {
                    setShowPreflightDetail(false);
                }
                return preflight.ok;
            } catch (error) {
                if (axios.isAxiosError(error) && error.response?.status === 404) {
                    const nowIso = new Date().toISOString();
                    setPreflightChecks([
                        {
                            key: "legacy_core_preflight",
                            label: "Preflight API",
                            ok: true,
                            expected: "/api/agent/preflight",
                            actual: "legacy_core_mode",
                            message:
                                "Legacy core detected (preflight API unavailable). Proceeding without preflight gate.",
                        },
                    ]);
                    setPreflightOk(true);
                    setPreflightCheckedAt(nowIso);
                    setPreflightActiveApp(null);
                    setPreflightError(null);
                    if (!silent) {
                        setResults([
                            {
                                type: "response",
                                content:
                                    "⚠️ 실행 전 점검 API가 없는 코어 버전입니다. 이번 실행은 preflight 게이트 없이 진행합니다.",
                            },
                        ]);
                    }
                    return true;
                }
                const message = error instanceof Error ? error.message : String(error);
                setPreflightOk(false);
                setPreflightError(message);
                setShowPreflightDetail(true);
                if (!silent) {
                    setResults([
                        {
                            type: "error",
                            content: `실행 전 점검 API 호출 실패: ${message}`,
                        },
                    ]);
                }
                return false;
            } finally {
                setPreflightLoading(false);
            }
        },
        [
            setPreflightActiveApp,
            setPreflightCheckedAt,
            setPreflightChecks,
            setPreflightError,
            setPreflightFixMessage,
            setPreflightLoading,
            setPreflightOk,
            setResults,
            setShowPreflightDetail,
        ]
    );

    const handlePreflightFix = useCallback(
        async (action: string) => {
            setPreflightFixBusy(action);
            setPreflightFixMessage(null);
            try {
                const assertionKey = `recovery.preflight.${action}`;
                const fix = await runAgentPreflightFix(action, {
                    run_id: currentRunId,
                    stage_name: "recovery",
                    assertion_key: assertionKey,
                });
                const front = fix.active_app ? ` (front=${fix.active_app})` : "";
                const recordedMessage =
                    fix.recorded && fix.run_id
                        ? ` · 기록됨(run=${fix.run_id})`
                        : currentRunId
                          ? " · 기록 없음(run 미존재)"
                          : "";
                const hint = preflightPermissionHint(fix.message);
                setPreflightFixMessage(
                    hint
                        ? `${fix.message}${front}${recordedMessage}\n${hint}`
                        : `${fix.message}${front}${recordedMessage}`
                );
                if (fix.recorded && fix.run_id) {
                    await loadRunDiagnostics(fix.run_id);
                    await loadDodHistory();
                }
                await runPreflightCheck(true);
            } catch (error) {
                const message = error instanceof Error ? error.message : String(error);
                const hint = preflightPermissionHint(message);
                setPreflightFixMessage(
                    hint ? `자동 조치 실패: ${message}\n${hint}` : `자동 조치 실패: ${message}`
                );
            } finally {
                setPreflightFixBusy(null);
            }
        },
        [
            currentRunId,
            loadDodHistory,
            loadRunDiagnostics,
            runPreflightCheck,
            setPreflightFixBusy,
            setPreflightFixMessage,
        ]
    );

    useEffect(() => {
        void runPreflightCheck(true);
    }, [runPreflightCheck]);

    useEffect(() => {
        if (preflightOk === false) {
            setShowDetailPanel(true);
        }
    }, [preflightOk, setShowDetailPanel]);

    return {
        runPreflightCheck,
        handlePreflightFix,
    };
}
