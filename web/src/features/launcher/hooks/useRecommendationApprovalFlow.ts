import { useCallback, useRef, useState } from "react";
import type { Dispatch, SetStateAction } from "react";

import { approveRecommendation } from "@/lib/api";
import type { Recommendation } from "@/lib/types";
import {
    N8N_EDITOR_BASE_URL,
    resolveRecommendationWorkflowUrl,
    type LauncherResult,
    type ProvisioningUiState,
} from "@/features/launcher/support";
import {
    buildOpenEditorResult,
    buildWorkflowApprovedResult,
    buildWorkflowPendingResult,
    extractApproveErrorMessage,
    mapApproveErrorMessage,
} from "@/features/launcher/hooks/recommendationApprovalSupport";
import { monitorApprovedWorkflowWithPolling } from "@/features/launcher/hooks/recommendationApprovalMonitor";

type UseRecommendationApprovalFlowParams = {
    recs: Recommendation[] | undefined;
    refetch: () => Promise<unknown>;
    provisioningUiByRecId: Record<number, ProvisioningUiState>;
    setProvisioningUiState: (
        id: number,
        phase: ProvisioningUiState["phase"],
        options?: { opId?: number | null; detail?: string }
    ) => void;
    clearProvisioningUiState: (id: number) => void;
    addWatchRecommendation: (id: number, fallback?: Recommendation | null) => void;
    removeWatchRecommendation: (id: number) => void;
    setWatchRecommendationCache: Dispatch<
        SetStateAction<Record<number, Recommendation>>
    >;
    openExternalTarget: (target: string) => Promise<void>;
    triggerSuccess: () => void;
    triggerError: () => void;
    setResults: Dispatch<SetStateAction<LauncherResult[]>>;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
};

export function useRecommendationApprovalFlow({
    recs,
    refetch,
    provisioningUiByRecId,
    setProvisioningUiState,
    clearProvisioningUiState,
    addWatchRecommendation,
    removeWatchRecommendation,
    setWatchRecommendationCache,
    openExternalTarget,
    triggerSuccess,
    triggerError,
    setResults,
    setShowDetailPanel,
}: UseRecommendationApprovalFlowParams) {
    const [approvingIds, setApprovingIds] = useState<Set<number>>(new Set());
    const [approveErrors, setApproveErrors] = useState<Record<number, string>>({});
    const [n8nOpenBusyKey, setN8nOpenBusyKey] = useState<string | null>(null);
    const approvalMonitorIdsRef = useRef<Set<number>>(new Set());
    const approveCooldownsRef = useRef<Record<number, number>>({});

    const monitorApprovedWorkflow = useCallback(
        async (
            id: number,
            fallback?: Recommendation | null,
            initialProvisionOpId?: number | null
        ) => {
            if (approvalMonitorIdsRef.current.has(id)) return;
            approvalMonitorIdsRef.current.add(id);
            try {
                await monitorApprovedWorkflowWithPolling({
                    id,
                    fallback,
                    initialProvisionOpId,
                    addWatchRecommendation,
                    removeWatchRecommendation,
                    setWatchRecommendationCache,
                    setProvisioningUiState,
                    clearProvisioningUiState,
                    openExternalTarget,
                    setApproveErrors,
                    setResults,
                    setShowDetailPanel,
                    refetch,
                    triggerSuccess,
                });
            } finally {
                approvalMonitorIdsRef.current.delete(id);
            }
        },
        [
            addWatchRecommendation,
            clearProvisioningUiState,
            openExternalTarget,
            refetch,
            removeWatchRecommendation,
            setApproveErrors,
            setProvisioningUiState,
            setResults,
            setShowDetailPanel,
            setWatchRecommendationCache,
            triggerSuccess,
        ]
    );

    const handleApprove = useCallback(
        async (id: number) => {
            const sourceRec = (recs ?? []).find((rec) => rec.id === id) ?? null;
            if (sourceRec && !sourceRec.approval_ready) {
                const detail =
                    sourceRec.approval_reasons.join(" ") ||
                    "Needs more evidence before approval.";
                setApproveErrors((prev) => ({ ...prev, [id]: detail }));
                setResults([
                    {
                        type: "response",
                        content: [
                            "**승인 보류**",
                            `- recommendation_id: \`${id}\``,
                            `- 사유: ${detail}`,
                        ].join("\n"),
                    },
                ]);
                setShowDetailPanel(true);
                return;
            }

            const now = Date.now();
            const last = approveCooldownsRef.current[id] ?? 0;
            if (now - last < 3000) return;
            approveCooldownsRef.current[id] = now;

            setApproveErrors((prev) => {
                const next = { ...prev };
                delete next[id];
                return next;
            });
            setApprovingIds((prev) => {
                const next = new Set(prev);
                next.add(id);
                return next;
            });
            addWatchRecommendation(id, sourceRec);
            setProvisioningUiState(id, "provisioning", {
                opId: provisioningUiByRecId[id]?.opId ?? null,
                detail: "requested",
            });
            try {
                const approved = await approveRecommendation(id, "web_launcher");
                const workflowId =
                    approved.workflow_id?.trim() ||
                    approved.id?.trim() ||
                    sourceRec?.workflow_id?.trim() ||
                    null;
                const workflowUrl =
                    approved.workflow_url?.trim() ||
                    resolveRecommendationWorkflowUrl(sourceRec ?? null, workflowId);
                const provisionOpId = approved.provision_op_id ?? null;
                const provisionStatus = approved.provision_status?.trim() || null;

                if (workflowUrl) {
                    clearProvisioningUiState(id);
                    const busyKey = `approve:${id}`;
                    setN8nOpenBusyKey(busyKey);
                    try {
                        await openExternalTarget(workflowUrl);
                        setResults([
                            buildWorkflowApprovedResult({
                                recommendationId: id,
                                workflowId,
                                provisionOpId,
                                workflowUrl,
                            }),
                        ]);
                    } catch (openError) {
                        const openMsg =
                            openError instanceof Error ? openError.message : String(openError);
                        setResults([
                            buildWorkflowApprovedResult({
                                recommendationId: id,
                                workflowId,
                                provisionOpId,
                                workflowUrl,
                                openFailedReason: openMsg,
                            }),
                        ]);
                    } finally {
                        setShowDetailPanel(true);
                        setN8nOpenBusyKey(null);
                    }
                    removeWatchRecommendation(id);
                } else {
                    setProvisioningUiState(id, "provisioning", {
                        opId: provisionOpId,
                        detail: provisionStatus ?? "requested",
                    });
                    const busyKey = `approve:${id}`;
                    setN8nOpenBusyKey(busyKey);
                    let editorOpenNote =
                        "- n8n 편집기 홈을 먼저 열었습니다. workflow URL 준비 시 상세 페이지를 자동으로 엽니다.";
                    try {
                        await openExternalTarget(N8N_EDITOR_BASE_URL);
                    } catch (openError) {
                        const openMsg =
                            openError instanceof Error ? openError.message : String(openError);
                        editorOpenNote = `- n8n 편집기 자동 열기 실패: ${openMsg}`;
                    } finally {
                        setN8nOpenBusyKey(null);
                    }
                    setResults([
                        buildWorkflowPendingResult({
                            recommendationId: id,
                            provisionOpId,
                            provisionStatus,
                            editorOpenNote,
                        }),
                    ]);
                    setShowDetailPanel(true);
                    void monitorApprovedWorkflow(id, sourceRec, provisionOpId);
                }
                triggerSuccess();
            } catch (error) {
                console.error("Approve failed", error);
                triggerError();
                const mapped = mapApproveErrorMessage(
                    extractApproveErrorMessage(error)
                );
                setProvisioningUiState(id, "failed", { opId: null, detail: mapped });
                setApproveErrors((prev) => ({ ...prev, [id]: mapped }));
                setResults([
                    {
                        type: "response",
                        content: [
                            "**승인 요청 지연/실패 감지**",
                            `- recommendation_id: \`${id}\``,
                            `- 상세: ${mapped}`,
                            "- 백엔드 반영 여부를 확인하면서 workflow URL 생성을 추적합니다.",
                        ].join("\n"),
                    },
                ]);
                setShowDetailPanel(true);
                void monitorApprovedWorkflow(id, sourceRec, null);
            } finally {
                setApprovingIds((prev) => {
                    const next = new Set(prev);
                    next.delete(id);
                    return next;
                });
                void refetch();
            }
        },
        [
            addWatchRecommendation,
            clearProvisioningUiState,
            monitorApprovedWorkflow,
            openExternalTarget,
            provisioningUiByRecId,
            recs,
            refetch,
            removeWatchRecommendation,
            setProvisioningUiState,
            setResults,
            setShowDetailPanel,
            triggerError,
            triggerSuccess,
        ]
    );

    const openRecommendationTarget = useCallback(
        async (params: {
            rec: Recommendation;
            n8nTarget: string;
        }) => {
            const { rec, n8nTarget } = params;
            const busyKey = `rec:${rec.id}`;
            setN8nOpenBusyKey(busyKey);
            try {
                await openExternalTarget(n8nTarget);
                setResults([
                    buildOpenEditorResult({
                        recommendationId: rec.id,
                        workflowId: rec.workflow_id,
                        targetUrl: n8nTarget,
                    }),
                ]);
                setShowDetailPanel(true);
                removeWatchRecommendation(rec.id);
            } catch (openError) {
                const openMsg =
                    openError instanceof Error ? openError.message : String(openError);
                setApproveErrors((prev) => ({
                    ...prev,
                    [rec.id]: `n8n 열기 실패: ${openMsg}`,
                }));
            } finally {
                setN8nOpenBusyKey(null);
            }
        },
        [openExternalTarget, removeWatchRecommendation, setResults, setShowDetailPanel]
    );

    return {
        approvingIds,
        approveErrors,
        n8nOpenBusyKey,
        handleApprove,
        openRecommendationTarget,
    };
}
