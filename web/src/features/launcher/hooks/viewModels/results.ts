import type { PanelBuilderArgs } from "@/features/launcher/hooks/viewModels/shared";
import type { ResultsFeedViewModel } from "@/features/launcher/hooks/viewModels/types";

export function buildResultsFeedPanelProps({
    runtime,
    actions,
}: PanelBuilderArgs): ResultsFeedViewModel {
    return {
        scrollRef: runtime.scrollRef,
        results: runtime.results,
        suggestionRows: runtime.suggestionRows,
        pendingApprovalVisible: !!runtime.pendingApproval,
        navigableItems: runtime.navigableItems,
        selectedIndex: runtime.selectedIndex,
        approveErrors: actions.approveErrors,
        approvingIds: actions.approvingIds,
        n8nOpenBusyKey: actions.n8nOpenBusyKey,
        onPinResult: actions.handlePin,
        onSelectSuggestion: (recId: number, fallbackIdx: number) => {
            const navIndex = runtime.navigableItems.findIndex((x) => x.id === `rec-${recId}`);
            runtime.setSelectedIndex(navIndex >= 0 ? navIndex : fallbackIdx);
        },
        onPinSuggestion: actions.handlePin,
        onOpenRecommendationTarget: (row: (typeof runtime.suggestionRows)[number]) => {
            const recN8nTarget = row.n8nTarget;
            if (!recN8nTarget) return;
            void actions.openRecommendationTarget({
                rec: row.rec,
                n8nTarget: recN8nTarget,
            });
        },
        onApprove: actions.handleApprove,
        onCopyProvisionOp: (opId: number) =>
            void actions.copyTextValue(String(opId), "provision_op_id"),
    };
}
