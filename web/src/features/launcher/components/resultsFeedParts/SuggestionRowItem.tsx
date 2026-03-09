import type { SuggestionRowItemProps } from "@/features/launcher/components/resultsFeedParts/panelTypes";
import { SuggestionRowActions } from "@/features/launcher/components/resultsFeedParts/SuggestionRowActions";
import { SuggestionRowMeta } from "@/features/launcher/components/resultsFeedParts/SuggestionRowMeta";

export function SuggestionRowItem(props: SuggestionRowItemProps) {
  const { row, onSelectSuggestion } = props;

  return (
    <div
      className={`launcher-suggestion-row group mb-1 flex cursor-pointer items-center justify-between rounded-md border px-3 py-2 transition-all ${
        row.isSelected
          ? "border-blue-500/30 bg-blue-500/20"
          : "border-transparent hover:bg-white/5"
      }`}
      onClick={() => onSelectSuggestion(row.rec.id, row.idx)}
    >
      <SuggestionRowMeta
        row={row}
        approveErrors={props.approveErrors}
        onCopyProvisionOp={props.onCopyProvisionOp}
      />
      <SuggestionRowActions
        row={row}
        approveErrors={props.approveErrors}
        approvingIds={props.approvingIds}
        n8nOpenBusyKey={props.n8nOpenBusyKey}
        onPinSuggestion={props.onPinSuggestion}
        onOpenRecommendationTarget={props.onOpenRecommendationTarget}
        onApprove={props.onApprove}
      />
    </div>
  );
}
