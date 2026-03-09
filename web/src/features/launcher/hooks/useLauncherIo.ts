import { useCallback } from "react";
import type { Dispatch, SetStateAction } from "react";
import { emit } from "@tauri-apps/api/event";
import { getAllWindows } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

import { recordAgentRecoveryEvent } from "@/lib/api";
import type { TaskRunArtifact } from "@/lib/types";

type WindowWithTauriMeta = Window & {
    __TAURI_METADATA__?: unknown;
    __TAURI__?: { metadata?: unknown };
    __TAURI_INTERNALS__?: { metadata?: unknown };
};

type UseLauncherIoParams = {
    runId: string | null;
    fallbackRunId: string | null;
    setSuccessPulse: Dispatch<SetStateAction<boolean>>;
    setShake: Dispatch<SetStateAction<boolean>>;
    setArtifactActionMessage: Dispatch<SetStateAction<string | null>>;
    setArtifactOpenBusy: Dispatch<SetStateAction<string | null>>;
    setPinnedArtifactKeys: Dispatch<SetStateAction<Set<string>>>;
    loadRunDiagnostics: (runId?: string | null) => Promise<void>;
    loadDodHistory: () => Promise<void>;
};

export function useLauncherIo({
    runId,
    fallbackRunId,
    setSuccessPulse,
    setShake,
    setArtifactActionMessage,
    setArtifactOpenBusy,
    setPinnedArtifactKeys,
    loadRunDiagnostics,
    loadDodHistory,
}: UseLauncherIoParams) {
    const triggerSuccess = useCallback(() => {
        setSuccessPulse(true);
        setTimeout(() => setSuccessPulse(false), 1000);
    }, [setSuccessPulse]);

    const triggerError = useCallback(() => {
        setShake(true);
        setTimeout(() => setShake(false), 500);
    }, [setShake]);

    const handlePin = useCallback(
        async (content: string, title?: string) => {
            try {
                await emit("pin-data", {
                    type: "text",
                    content,
                    title: title || "Pinned from AllvIa",
                });

                const windows = await getAllWindows();
                const widgetWin = windows.find((w) => w.label === "widget");
                if (widgetWin) {
                    await widgetWin.show();
                }

                triggerSuccess();
            } catch (error) {
                console.error("Pin failed", error);
                triggerError();
            }
        },
        [triggerError, triggerSuccess]
    );

    const recordRecoveryAction = useCallback(
        async (payload: {
            actionKey: string;
            status: "completed" | "failed";
            details: string;
            runId?: string | null;
            stageName?: string;
            expected?: string;
            actual?: string;
        }) => {
            const targetRunId = payload.runId ?? runId ?? fallbackRunId ?? null;
            if (!targetRunId) return;
            try {
                const rec = await recordAgentRecoveryEvent({
                    run_id: targetRunId,
                    action_key: payload.actionKey,
                    status: payload.status,
                    details: payload.details,
                    stage_name: payload.stageName ?? "recovery",
                    expected: payload.expected ?? "completed",
                    actual: payload.actual ?? payload.status,
                });
                if (rec.recorded) {
                    await loadRunDiagnostics(rec.run_id);
                    await loadDodHistory();
                }
            } catch (error) {
                const message = error instanceof Error ? error.message : String(error);
                setArtifactActionMessage(`복구 기록 실패: ${message}`);
            }
        },
        [fallbackRunId, loadDodHistory, loadRunDiagnostics, runId, setArtifactActionMessage]
    );

    const openArtifactPath = useCallback(
        async (path: string) => {
            const candidate = path.trim();
            if (!candidate) return;
            setArtifactOpenBusy(candidate);
            setArtifactActionMessage(null);
            let actionStatus: "completed" | "failed" = "completed";
            let actionDetails = `artifact=${candidate}`;
            try {
                const tauriMeta =
                    (window as WindowWithTauriMeta).__TAURI_METADATA__ ||
                    (window as WindowWithTauriMeta).__TAURI__?.metadata ||
                    (window as WindowWithTauriMeta).__TAURI_INTERNALS__?.metadata;
                if (tauriMeta) {
                    const opened = await invoke<string>("open_artifact_path", { path: candidate });
                    actionDetails = `artifact opened: ${opened}`;
                    setArtifactActionMessage(`증거 열기 완료: ${opened}`);
                } else if (navigator?.clipboard?.writeText) {
                    await navigator.clipboard.writeText(candidate);
                    actionDetails = `artifact copied to clipboard: ${candidate}`;
                    setArtifactActionMessage(`웹 모드: 경로를 클립보드에 복사했습니다 (${candidate})`);
                } else {
                    actionDetails = `artifact manual open requested: ${candidate}`;
                    setArtifactActionMessage(`웹 모드: 경로를 수동으로 열어주세요 (${candidate})`);
                }
            } catch (error) {
                const message = error instanceof Error ? error.message : String(error);
                actionStatus = "failed";
                actionDetails = `artifact open failed: ${message}`;
                setArtifactActionMessage(`증거 열기 실패: ${message}`);
            } finally {
                await recordRecoveryAction({
                    actionKey: "recovery.artifact.open",
                    status: actionStatus,
                    details: actionDetails,
                    actual: actionStatus,
                });
                setArtifactOpenBusy(null);
            }
        },
        [recordRecoveryAction, setArtifactActionMessage, setArtifactOpenBusy]
    );

    const openExternalTarget = useCallback(async (target: string) => {
        const candidate = target.trim();
        if (!candidate) {
            throw new Error("empty target");
        }
        const tauriMeta =
            (window as WindowWithTauriMeta).__TAURI_METADATA__ ||
            (window as WindowWithTauriMeta).__TAURI__?.metadata ||
            (window as WindowWithTauriMeta).__TAURI_INTERNALS__?.metadata;
        if (tauriMeta) {
            await invoke<string>("open_external_target", { target: candidate });
            return;
        }
        const popup = window.open(candidate, "_blank", "noopener,noreferrer");
        if (!popup) {
            throw new Error("popup_blocked");
        }
    }, []);

    const copyTextValue = useCallback(
        async (value: string, label: string) => {
            const candidate = value.trim();
            if (!candidate) return;
            try {
                if (navigator?.clipboard?.writeText) {
                    await navigator.clipboard.writeText(candidate);
                    setArtifactActionMessage(`${label} 복사 완료: ${candidate}`);
                    return;
                }
                const el = document.createElement("textarea");
                el.value = candidate;
                el.style.position = "fixed";
                el.style.left = "-9999px";
                document.body.appendChild(el);
                el.focus();
                el.select();
                document.execCommand("copy");
                document.body.removeChild(el);
                setArtifactActionMessage(`${label} 복사 완료: ${candidate}`);
            } catch (error) {
                const message = error instanceof Error ? error.message : String(error);
                setArtifactActionMessage(`${label} 복사 실패: ${message}`);
            }
        },
        [setArtifactActionMessage]
    );

    const copyArtifactPayload = useCallback(
        async (artifact: TaskRunArtifact) => {
            const payload = JSON.stringify(
                {
                    run_id: artifact.run_id,
                    artifact_type: artifact.artifact_type,
                    artifact_key: artifact.artifact_key,
                    value: artifact.value,
                    metadata: artifact.metadata ?? null,
                    created_at: artifact.created_at,
                },
                null,
                2
            );
            try {
                if (navigator?.clipboard?.writeText) {
                    await navigator.clipboard.writeText(payload);
                    setArtifactActionMessage(`복사 완료: ${artifact.artifact_key}`);
                    return;
                }
                const el = document.createElement("textarea");
                el.value = payload;
                el.style.position = "fixed";
                el.style.left = "-9999px";
                document.body.appendChild(el);
                el.focus();
                el.select();
                document.execCommand("copy");
                document.body.removeChild(el);
                setArtifactActionMessage(`복사 완료: ${artifact.artifact_key}`);
            } catch (error) {
                const message = error instanceof Error ? error.message : String(error);
                setArtifactActionMessage(`복사 실패: ${message}`);
            }
        },
        [setArtifactActionMessage]
    );

    const togglePinArtifactKey = useCallback(
        (artifactKey: string) => {
            setPinnedArtifactKeys((prev) => {
                const next = new Set(prev);
                if (next.has(artifactKey)) {
                    next.delete(artifactKey);
                } else {
                    next.add(artifactKey);
                }
                return next;
            });
        },
        [setPinnedArtifactKeys]
    );

    return {
        triggerSuccess,
        triggerError,
        handlePin,
        recordRecoveryAction,
        openArtifactPath,
        openExternalTarget,
        copyTextValue,
        copyArtifactPayload,
        togglePinArtifactKey,
    };
}
