import type { Dispatch, SetStateAction } from "react";

import {
    fetchRecommendations,
    fetchWorkflowProvisionOps,
} from "@/lib/api";
import type { Recommendation } from "@/lib/types";
import {
    APPROVAL_MONITOR_INTERVAL_MS,
    APPROVAL_MONITOR_MAX_ATTEMPTS,
    APPROVAL_MONITOR_PENDING_NOTICE_ATTEMPT,
    N8N_EDITOR_BASE_URL,
    resolveRecommendationWorkflowUrl,
    type LauncherResult,
    type ProvisioningUiState,
} from "@/features/launcher/support";
import {
    buildProvisionFailureResult,
    buildWorkflowApprovedResult,
    buildWorkflowPendingNoticeResult,
} from "@/features/launcher/hooks/recommendationApprovalSupport";

type RecommendationApprovalMonitorParams = {
    id: number;
    fallback?: Recommendation | null;
    initialProvisionOpId?: number | null;
    addWatchRecommendation: (id: number, fallback?: Recommendation | null) => void;
    removeWatchRecommendation: (id: number) => void;
    setWatchRecommendationCache: Dispatch<
        SetStateAction<Record<number, Recommendation>>
    >;
    setProvisioningUiState: (
        id: number,
        phase: ProvisioningUiState["phase"],
        options?: { opId?: number | null; detail?: string }
    ) => void;
    clearProvisioningUiState: (id: number) => void;
    openExternalTarget: (target: string) => Promise<void>;
    setApproveErrors: Dispatch<SetStateAction<Record<number, string>>>;
    setResults: Dispatch<SetStateAction<LauncherResult[]>>;
    setShowDetailPanel: Dispatch<SetStateAction<boolean>>;
    refetch: () => Promise<unknown>;
    triggerSuccess: () => void;
};

export async function monitorApprovedWorkflowWithPolling({
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
}: RecommendationApprovalMonitorParams) {
    addWatchRecommendation(id, fallback ?? null);
    let trackedProvisionOpId = initialProvisionOpId ?? null;
    let pendingNoticeShown = false;

    const reportProvisionFailure = (reason: string) => {
        setProvisioningUiState(id, "failed", {
            opId: trackedProvisionOpId,
            detail: reason,
        });
        setApproveErrors((prev) => ({
            ...prev,
            [id]: `워크플로 생성 실패: ${reason}`,
        }));
        setResults((prev) => [
            ...prev.slice(-6),
            buildProvisionFailureResult({
                recommendationId: id,
                reason,
                provisionOpId: trackedProvisionOpId,
                editorUrl: N8N_EDITOR_BASE_URL,
            }),
        ]);
        setShowDetailPanel(true);
    };

    try {
        for (let attempt = 0; attempt < APPROVAL_MONITOR_MAX_ATTEMPTS; attempt += 1) {
            const latest = await fetchRecommendations();
            const current = latest.find((rec) => rec.id === id) ?? null;
            if (current) {
                setWatchRecommendationCache((prev) => ({ ...prev, [id]: current }));
            }
            const currentStatus = current?.status?.toLowerCase() ?? "";
            if (currentStatus === "failed" || currentStatus === "rejected") {
                const reason =
                    current?.last_error?.trim() || "workflow provisioning failed";
                reportProvisionFailure(reason);
                removeWatchRecommendation(id);
                return;
            }

            let latestProvisionOp: Awaited<
                ReturnType<typeof fetchWorkflowProvisionOps>
            >[number] | null = null;
            try {
                const ops = await fetchWorkflowProvisionOps({
                    recommendationId: id,
                    limit: 10,
                });
                if (trackedProvisionOpId != null) {
                    latestProvisionOp =
                        ops.find((op) => op.id === trackedProvisionOpId) ?? ops[0] ?? null;
                } else {
                    latestProvisionOp = ops[0] ?? null;
                }
                if (latestProvisionOp) {
                    trackedProvisionOpId = latestProvisionOp.id;
                }
            } catch {
                // Ignore transient provision-op API failures and continue fallback polling.
            }

            const opStatus = latestProvisionOp?.status?.toLowerCase() ?? "";
            if (opStatus === "failed" || opStatus === "reconcile_needed") {
                const reason =
                    latestProvisionOp?.error?.trim() ||
                    current?.last_error?.trim() ||
                    "workflow provisioning failed";
                reportProvisionFailure(reason);
                removeWatchRecommendation(id);
                return;
            }

            const workflowIdFromOp = latestProvisionOp?.workflow_id?.trim() || null;
            const workflowUrl =
                resolveRecommendationWorkflowUrl(current, workflowIdFromOp) ||
                resolveRecommendationWorkflowUrl(fallback ?? null, workflowIdFromOp);
            const workflowId =
                workflowIdFromOp ||
                current?.workflow_id?.trim() ||
                fallback?.workflow_id?.trim() ||
                null;

            if (workflowUrl) {
                await openExternalTarget(workflowUrl);
                clearProvisioningUiState(id);
                setResults([
                    buildWorkflowApprovedResult({
                        recommendationId: id,
                        workflowId,
                        provisionOpId: trackedProvisionOpId,
                        workflowUrl,
                    }),
                ]);
                setShowDetailPanel(true);
                removeWatchRecommendation(id);
                triggerSuccess();
                return;
            }

            if (
                (opStatus === "requested" ||
                    opStatus === "provisioning" ||
                    opStatus === "created") &&
                attempt >= APPROVAL_MONITOR_PENDING_NOTICE_ATTEMPT &&
                !pendingNoticeShown
            ) {
                setProvisioningUiState(id, "provisioning", {
                    opId: trackedProvisionOpId,
                    detail: opStatus || "requested",
                });
                pendingNoticeShown = true;
                setResults([
                    buildWorkflowPendingNoticeResult({
                        recommendationId: id,
                        provisionOpId: trackedProvisionOpId,
                    }),
                ]);
            } else if (
                opStatus === "requested" ||
                opStatus === "provisioning" ||
                opStatus === "created"
            ) {
                setProvisioningUiState(id, "provisioning", {
                    opId: trackedProvisionOpId,
                    detail: opStatus,
                });
            }

            await new Promise((resolve) =>
                window.setTimeout(resolve, APPROVAL_MONITOR_INTERVAL_MS)
            );
        }
        reportProvisionFailure("workflow URL generation timeout");
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        reportProvisionFailure(`workflow monitor failed: ${message}`);
    } finally {
        void refetch();
    }
}
