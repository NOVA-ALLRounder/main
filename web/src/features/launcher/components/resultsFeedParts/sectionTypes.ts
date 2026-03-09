import type {
  ResultsFeedPanelProps,
  SuggestionRowItemProps,
} from "@/features/launcher/components/resultsFeedParts/panelTypes";

export type ResultMessagesSectionProps = Pick<
  ResultsFeedPanelProps,
  | "results"
  | "navigableItems"
  | "selectedIndex"
  | "markdownComponents"
  | "onPinResult"
>;

export type SuggestionRowsSectionProps = Pick<
  ResultsFeedPanelProps,
  | "suggestionRows"
  | "approveErrors"
  | "approvingIds"
  | "n8nOpenBusyKey"
  | "onSelectSuggestion"
  | "onPinSuggestion"
  | "onOpenRecommendationTarget"
  | "onApprove"
  | "onCopyProvisionOp"
>;

export type EmptyResultsStateProps = Pick<
  ResultsFeedPanelProps,
  "results" | "suggestionRows" | "pendingApprovalVisible"
>;

export type SuggestionRowMetaProps = Pick<
  SuggestionRowItemProps,
  "row" | "approveErrors" | "onCopyProvisionOp"
>;

export type SuggestionRowActionsProps = Pick<
  SuggestionRowItemProps,
  | "row"
  | "approvingIds"
  | "n8nOpenBusyKey"
  | "onPinSuggestion"
  | "onOpenRecommendationTarget"
  | "onApprove"
  | "approveErrors"
>;
