import type { RefObject } from "react";
import type { Components } from "react-markdown";
import type { LauncherResult, SuggestionRow } from "@/features/launcher/support";

export type ResultsFeedPanelProps = {
  scrollRef: RefObject<HTMLDivElement | null>;
  results: LauncherResult[];
  suggestionRows: SuggestionRow[];
  pendingApprovalVisible: boolean;
  navigableItems: Array<{ id: string }>;
  selectedIndex: number;
  markdownComponents: Components;
  approveErrors: Record<number, string>;
  approvingIds: Set<number>;
  n8nOpenBusyKey: string | null;
  onPinResult: (content: string) => void;
  onSelectSuggestion: (recId: number, fallbackIdx: number) => void;
  onPinSuggestion: (summary: string, title?: string) => void;
  onOpenRecommendationTarget: (row: SuggestionRow) => void;
  onApprove: (recId: number) => void;
  onCopyProvisionOp: (opId: number) => void;
};

export type SuggestionRowItemProps = {
  row: SuggestionRow;
} & Pick<
  ResultsFeedPanelProps,
  | "approveErrors"
  | "approvingIds"
  | "n8nOpenBusyKey"
  | "onSelectSuggestion"
  | "onPinSuggestion"
  | "onOpenRecommendationTarget"
  | "onApprove"
  | "onCopyProvisionOp"
>;
