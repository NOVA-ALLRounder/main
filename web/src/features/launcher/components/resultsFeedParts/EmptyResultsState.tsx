import { Terminal } from "lucide-react";
import type { EmptyResultsStateProps } from "@/features/launcher/components/resultsFeedParts/sectionTypes";

export function EmptyResultsState({
  results,
  suggestionRows,
  pendingApprovalVisible,
}: EmptyResultsStateProps) {
  if (results.length > 0 || suggestionRows.length > 0 || pendingApprovalVisible) return null;

  return (
    <div className="p-6 text-center text-gray-500">
      <Terminal className="w-10 h-10 mx-auto mb-2 opacity-20" />
      <p className="text-sm">결과가 준비되면 여기에 표시됩니다.</p>
    </div>
  );
}
