import type { SuggestionRowsSectionProps } from "@/features/launcher/components/resultsFeedParts/sectionTypes";
import { SuggestionRowItem } from "@/features/launcher/components/resultsFeedParts/SuggestionRowItem";

export function SuggestionRowsSection(props: SuggestionRowsSectionProps) {
  if (props.suggestionRows.length === 0) return null;

  return (
    <div className="pt-1">
      <div className="px-1 py-1 text-[11px] font-semibold text-gray-500 uppercase tracking-wider mb-1">
        Suggestions
      </div>
      {props.suggestionRows.map((row) => (
        <SuggestionRowItem
          key={row.rec.id}
          row={row}
          approveErrors={props.approveErrors}
          approvingIds={props.approvingIds}
          n8nOpenBusyKey={props.n8nOpenBusyKey}
          onSelectSuggestion={props.onSelectSuggestion}
          onPinSuggestion={props.onPinSuggestion}
          onOpenRecommendationTarget={props.onOpenRecommendationTarget}
          onApprove={props.onApprove}
          onCopyProvisionOp={props.onCopyProvisionOp}
        />
      ))}
    </div>
  );
}
