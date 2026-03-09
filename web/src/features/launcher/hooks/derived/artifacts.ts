import type { TaskRunArtifact, TaskStageAssertion, TaskStageRun } from "@/lib/types";
import {
  extractArtifactPaths,
  type ArtifactGroupItem,
  type ArtifactSortMode,
  type StageTraceItem,
} from "@/features/launcher/support";

type DeriveArtifactStateParams = {
  stageRuns: TaskStageRun[];
  stageAssertions: TaskStageAssertion[];
  taskRunArtifacts: TaskRunArtifact[];
  artifactTypeFilter: string;
  artifactFailedOnly: boolean;
  artifactSearchQuery: string;
  artifactSortMode: ArtifactSortMode;
  pinnedArtifactKeys: Set<string>;
};

export function deriveArtifactState({
  stageRuns,
  stageAssertions,
  taskRunArtifacts,
  artifactTypeFilter,
  artifactFailedOnly,
  artifactSearchQuery,
  artifactSortMode,
  pinnedArtifactKeys,
}: DeriveArtifactStateParams) {
  const failedAssertions = stageAssertions.filter((assertion) => !assertion.passed);
  const stageTraceItems: StageTraceItem[] = stageRuns
    .slice()
    .sort((a, b) => a.stage_order - b.stage_order)
    .map((stage) => {
      const assertions = stageAssertions.filter((a) => a.stage_name === stage.stage_name);
      const failed = assertions.filter((a) => !a.passed);
      return { stage, assertions, failed };
    });
  const recoveryAssertions = stageAssertions
    .filter((assertion) => assertion.stage_name === "recovery")
    .slice(-10)
    .reverse();
  const failedArtifactKeys = new Set(
    failedAssertions.map((assertion) => assertion.assertion_key)
  );
  const artifactTypeOptions = [
    "all",
    ...Array.from(new Set(taskRunArtifacts.map((item) => item.artifact_type))).sort((a, b) =>
      a.localeCompare(b)
    ),
  ];
  const artifactSearchLower = artifactSearchQuery.trim().toLowerCase();
  const artifactGroups: ArtifactGroupItem[] = (() => {
    if (taskRunArtifacts.length === 0) return [];
    const artifacts = taskRunArtifacts
      .filter((item) => {
        if (artifactTypeFilter !== "all" && item.artifact_type !== artifactTypeFilter) {
          return false;
        }
        if (artifactFailedOnly && !failedArtifactKeys.has(item.artifact_key)) {
          return false;
        }
        if (!artifactSearchLower) return true;
        const haystack = [
          item.artifact_type,
          item.artifact_key,
          item.value,
          item.metadata ?? "",
        ]
          .join("\n")
          .toLowerCase();
        return haystack.includes(artifactSearchLower);
      })
      .sort((a, b) => {
        const aPinned = pinnedArtifactKeys.has(a.artifact_key) ? 1 : 0;
        const bPinned = pinnedArtifactKeys.has(b.artifact_key) ? 1 : 0;
        if (aPinned !== bPinned) return bPinned - aPinned;
        if (artifactSortMode === "key") {
          return a.artifact_key.localeCompare(b.artifact_key);
        }
        if (artifactSortMode === "failed_first") {
          const aFailed = failedArtifactKeys.has(a.artifact_key) ? 1 : 0;
          const bFailed = failedArtifactKeys.has(b.artifact_key) ? 1 : 0;
          if (aFailed !== bFailed) return bFailed - aFailed;
        }
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      });
    const bucket = new Map<string, TaskRunArtifact[]>();
    for (const item of artifacts) {
      const group = item.artifact_type?.trim() || "unknown";
      const prev = bucket.get(group) ?? [];
      prev.push(item);
      bucket.set(group, prev);
    }
    return Array.from(bucket.entries())
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([type, items]) => ({ type, items }));
  })();

  const firstFailedArtifactPath = (() => {
    for (const assertion of failedAssertions) {
      const artifacts = extractArtifactPaths(
        assertion.evidence,
        `${assertion.actual} ${assertion.expected}`
      );
      if (artifacts.length > 0) {
        return artifacts[0];
      }
    }
    return null;
  })();

  return {
    failedAssertions,
    stageTraceItems,
    recoveryAssertions,
    failedArtifactKeys,
    artifactTypeOptions,
    artifactGroups,
    firstFailedArtifactPath,
  };
}
