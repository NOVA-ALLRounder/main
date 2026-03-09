import type { SuggestionRow } from "@/features/launcher/support";
import { isSuggestionProvisioning } from "@/features/launcher/components/resultsFeedParts/provisioning";

export function getSuggestionApproveLabel(
  row: SuggestionRow,
  options: {
    isApproving: boolean;
    hasApproveError: boolean;
  },
) {
  if (options.isApproving) return "Approving…";
  if (isSuggestionProvisioning(row)) return "Provisioning…";
  if (!row.rec.approval_ready && row.rec.status === "pending") return "Needs Evidence";
  if (!row.canApprove) return "Approved";
  if (row.canRetryProvision || options.hasApproveError) return "Retry";
  return "Approve";
}
