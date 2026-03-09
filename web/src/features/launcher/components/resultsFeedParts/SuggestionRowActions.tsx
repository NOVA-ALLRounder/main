import { Pin } from "lucide-react";
import type { SuggestionRowActionsProps } from "@/features/launcher/components/resultsFeedParts/sectionTypes";
import { getSuggestionApproveLabel } from "@/features/launcher/components/resultsFeedParts/labels";

export function SuggestionRowActions({
  row,
  approvingIds,
  n8nOpenBusyKey,
  onPinSuggestion,
  onOpenRecommendationTarget,
  onApprove,
  approveErrors,
}: SuggestionRowActionsProps) {
  const { rec, workflowUrl, canApprove, approvalBlockedReason, n8nTarget, isSelected } = row;
  const isApproving = approvingIds.has(rec.id);
  const n8nBusy = n8nOpenBusyKey === `rec:${rec.id}`;
  const approveLabel = getSuggestionApproveLabel(row, {
    isApproving,
    hasApproveError: !!approveErrors[rec.id],
  });

  return (
    <div className="flex items-center gap-2">
      <button
        onClick={(e) => {
          e.stopPropagation();
          onPinSuggestion(rec.summary, rec.title);
        }}
        className="rounded-md p-1.5 text-gray-400 opacity-0 transition-all group-hover:opacity-100 hover:bg-white/10 hover:text-white"
        title="Pin to Widget"
      >
        <Pin className="h-3 w-3" />
      </button>

      {n8nTarget && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onOpenRecommendationTarget(row);
          }}
          disabled={n8nBusy}
          className={`rounded border px-2 py-1 text-xs transition-colors ${
            isSelected
              ? "border-sky-400 bg-sky-500 text-white"
              : "border-sky-400/30 bg-sky-500/20 text-sky-200 hover:bg-sky-500/30"
          } ${n8nBusy ? "cursor-wait opacity-60" : ""}`}
        >
          {n8nBusy ? "열기…" : workflowUrl ? "n8n" : "n8n 홈"}
        </button>
      )}

      <button
        onClick={(e) => {
          e.stopPropagation();
          if (!canApprove) return;
          onApprove(rec.id);
        }}
        disabled={isApproving || !canApprove}
        title={approvalBlockedReason ?? undefined}
        className={`rounded border px-3 py-1.5 text-xs transition-colors ${
          isSelected
            ? "border-blue-400 bg-blue-500 text-white"
            : "border-white/10 bg-white/10 text-gray-200 hover:bg-white/20"
        } ${(isApproving || !canApprove) ? "cursor-not-allowed opacity-60" : ""}`}
      >
        {approveLabel}
      </button>

      <div className="rounded bg-white/5 px-2 py-1 text-[10px] text-gray-500">Enter</div>
    </div>
  );
}
