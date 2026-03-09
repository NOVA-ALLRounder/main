import type { Recommendation } from "@/lib/types";
import {
  N8N_EDITOR_BASE_URL,
  formatProvisionUpdatedAt,
  recommendationStatusToneClass,
  resolveRecommendationWorkflowUrl,
  type ProvisioningUiState,
  type SuggestionRow,
} from "@/features/launcher/support";

type DeriveSuggestionRecommendationsParams = {
  recs?: Recommendation[];
  watchRecommendationIds: Iterable<number>;
  watchRecommendationCache: Record<number, Recommendation>;
};

type DeriveSuggestionRowsParams = {
  suggestionRecs: Recommendation[];
  selectedIndex: number;
  provisioningUiByRecId: Record<number, ProvisioningUiState>;
};

export function deriveSuggestionRecommendations({
  recs,
  watchRecommendationIds,
  watchRecommendationCache,
}: DeriveSuggestionRecommendationsParams) {
  const pendingRecs = recs?.filter((rec) => rec.status === "pending") ?? [];
  const merged = new Map<number, Recommendation>();
  pendingRecs.forEach((rec) => merged.set(rec.id, rec));
  for (const id of watchRecommendationIds) {
    const live = recs?.find((rec) => rec.id === id);
    if (live) {
      merged.set(id, live);
      continue;
    }
    const cached = watchRecommendationCache[id];
    if (cached) {
      merged.set(id, cached);
    }
  }
  return Array.from(merged.values());
}

export function deriveSuggestionRows({
  suggestionRecs,
  selectedIndex,
  provisioningUiByRecId,
}: DeriveSuggestionRowsParams): SuggestionRow[] {
  const navigableItems = [
    ...suggestionRecs.map((rec) => ({ type: "recommendation" as const, data: rec, id: `rec-${rec.id}` })),
  ];
  return suggestionRecs.map((rec, idx) => {
    const isSelected = navigableItems[selectedIndex]?.id === `rec-${rec.id}`;
    const workflowUrl = resolveRecommendationWorkflowUrl(rec);
    const uiProvision = provisioningUiByRecId[rec.id];
    const statusLabel = formatRecommendationStatusLabel(rec, uiProvision);
    const canRetryProvision = uiProvision?.phase === "failed" || rec.status === "failed";
    const approvalBlockedReason =
      !rec.approval_ready && rec.approval_reasons.length > 0
        ? rec.approval_reasons.join(" ")
        : null;
    const canApprove =
      (rec.status === "pending" || canRetryProvision) && rec.approval_ready;
    const n8nTarget =
      workflowUrl ||
      ((uiProvision?.phase === "provisioning" ||
        rec.workflow_id?.startsWith("provisioning:") ||
        uiProvision?.detail === "requested" ||
        uiProvision?.detail === "created" ||
        canRetryProvision)
        ? N8N_EDITOR_BASE_URL
        : null);
    return {
      rec,
      idx,
      isSelected,
      workflowUrl,
      uiProvision,
      statusLabel,
      statusToneClass: recommendationStatusToneClass(rec, uiProvision),
      updatedAtLabel: uiProvision?.updatedAt
        ? formatProvisionUpdatedAt(uiProvision.updatedAt)
        : "",
      canRetryProvision,
      approvalBlockedReason,
      canApprove,
      n8nTarget,
    };
  });
}

function formatRecommendationStatusLabel(
  rec: Recommendation,
  uiState?: ProvisioningUiState
): string {
  if (uiState?.phase === "provisioning") {
    return "Provisioning (생성 중...)";
  }
  if (uiState?.phase === "failed") {
    return "Failed (재시도)";
  }
  if (rec.status === "failed") {
    return "Failed (재시도)";
  }
  if (rec.workflow_id?.startsWith("provisioning:")) {
    return "Provisioning (생성 중...)";
  }
  return rec.status;
}
