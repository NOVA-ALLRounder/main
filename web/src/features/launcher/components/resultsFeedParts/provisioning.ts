import type { SuggestionRow } from "@/features/launcher/support";

export function isSuggestionProvisioning(row: SuggestionRow) {
  return (
    row.uiProvision?.phase === "provisioning" ||
    row.rec.workflow_id?.startsWith("provisioning:") ||
    row.uiProvision?.detail === "requested" ||
    row.uiProvision?.detail === "created"
  );
}
