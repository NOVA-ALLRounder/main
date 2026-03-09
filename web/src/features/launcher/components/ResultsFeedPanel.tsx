import {
    type ResultsFeedPanelProps,
    EmptyResultsState,
    ResultMessagesSection,
    SuggestionRowsSection,
} from "@/features/launcher/components/ResultsFeedSections";

export type { ResultsFeedPanelProps } from "@/features/launcher/components/ResultsFeedSections";

export function ResultsFeedPanel(props: ResultsFeedPanelProps) {
    return (
        <div ref={props.scrollRef} className="launcher-feed max-h-[240px] overflow-y-auto p-3 space-y-2">
            <ResultMessagesSection
                results={props.results}
                navigableItems={props.navigableItems}
                selectedIndex={props.selectedIndex}
                markdownComponents={props.markdownComponents}
                onPinResult={props.onPinResult}
            />
            <SuggestionRowsSection
                suggestionRows={props.suggestionRows}
                approveErrors={props.approveErrors}
                approvingIds={props.approvingIds}
                n8nOpenBusyKey={props.n8nOpenBusyKey}
                onSelectSuggestion={props.onSelectSuggestion}
                onPinSuggestion={props.onPinSuggestion}
                onOpenRecommendationTarget={props.onOpenRecommendationTarget}
                onApprove={props.onApprove}
                onCopyProvisionOp={props.onCopyProvisionOp}
            />
            <EmptyResultsState
                results={props.results}
                suggestionRows={props.suggestionRows}
                pendingApprovalVisible={props.pendingApprovalVisible}
            />
        </div>
    );
}
