import {
    type ResultsFeedPanelProps,
    EmptyResultsState,
    ResultMessagesSection,
    SuggestionRowsSection,
} from "@/features/launcher/components/ResultsFeedSections";

export type { ResultsFeedPanelProps } from "@/features/launcher/components/ResultsFeedSections";

export function ResultsFeedPanel({
    scrollRef,
    results,
    navigableItems,
    selectedIndex,
    markdownComponents,
    onPinResult,
    suggestionRows,
    approveErrors,
    approvingIds,
    n8nOpenBusyKey,
    onSelectSuggestion,
    onPinSuggestion,
    onOpenRecommendationTarget,
    onApprove,
    onCopyProvisionOp,
    pendingApprovalVisible,
}: ResultsFeedPanelProps) {
    return (
        <div ref={scrollRef} className="launcher-feed max-h-[240px] overflow-y-auto p-3 space-y-2">
            <ResultMessagesSection
                results={results}
                navigableItems={navigableItems}
                selectedIndex={selectedIndex}
                markdownComponents={markdownComponents}
                onPinResult={onPinResult}
            />
            <SuggestionRowsSection
                suggestionRows={suggestionRows}
                approveErrors={approveErrors}
                approvingIds={approvingIds}
                n8nOpenBusyKey={n8nOpenBusyKey}
                onSelectSuggestion={onSelectSuggestion}
                onPinSuggestion={onPinSuggestion}
                onOpenRecommendationTarget={onOpenRecommendationTarget}
                onApprove={onApprove}
                onCopyProvisionOp={onCopyProvisionOp}
            />
            <EmptyResultsState
                results={results}
                suggestionRows={suggestionRows}
                pendingApprovalVisible={pendingApprovalVisible}
            />
        </div>
    );
}
