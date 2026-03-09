import { Zap } from "lucide-react";
import type { SuggestionRowMetaProps } from "@/features/launcher/components/resultsFeedParts/sectionTypes";

export function SuggestionRowMeta({
  row,
  approveErrors,
  onCopyProvisionOp,
}: SuggestionRowMetaProps) {
  const {
    rec,
    isSelected,
    uiProvision,
    statusLabel,
    statusToneClass,
    updatedAtLabel,
    approvalBlockedReason,
  } = row;

  return (
    <div className="flex items-center gap-3">
      <div
        className={`flex h-8 w-8 items-center justify-center rounded ${
          isSelected ? "bg-blue-500 text-white" : "bg-white/10 text-gray-400"
        }`}
      >
        <Zap className="h-4 w-4" />
      </div>
      <div>
        <div className={`text-sm font-medium ${isSelected ? "text-blue-100" : "text-gray-200"}`}>
          {rec.title}
        </div>
        <div className="line-clamp-1 text-xs text-gray-500">{rec.summary}</div>
        <div className="mt-1 flex items-center gap-1.5">
          <span className={`rounded-full border px-2 py-0.5 text-[10px] ${statusToneClass}`}>
            {statusLabel}
          </span>
          {updatedAtLabel ? <span className="text-[10px] text-gray-500">{updatedAtLabel}</span> : null}
        </div>
        {uiProvision?.opId != null && (
          <div className="mt-1 flex items-center gap-1.5">
            <span className="text-[10px] text-gray-400">op_id: {uiProvision.opId}</span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onCopyProvisionOp(uiProvision.opId!);
              }}
              className="rounded border border-white/15 px-1.5 py-0.5 text-[10px] text-gray-300 hover:bg-white/10"
            >
              복사
            </button>
          </div>
        )}
        {(approveErrors[rec.id] || uiProvision?.phase === "failed") && (
          <div className="mt-1 text-[10px] text-rose-300">
            {approveErrors[rec.id] || uiProvision?.detail}
          </div>
        )}
        {approvalBlockedReason && rec.status === "pending" && (
          <div className="mt-1 text-[10px] text-amber-300">{approvalBlockedReason}</div>
        )}
      </div>
    </div>
  );
}
