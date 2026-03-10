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
  const assertionsByStage = new Map<string, TaskStageAssertion[]>();
  const failedByStage = new Map<string, TaskStageAssertion[]>();
  const failedAssertions: TaskStageAssertion[] = [];
  const recoveryAssertionsBuffer: TaskStageAssertion[] = [];
  const failedArtifactKeys = new Set<string>();
  let firstFailedArtifactPath: string | null = null;

  for (const assertion of stageAssertions) {
    const stageAssertionsList = assertionsByStage.get(assertion.stage_name);
    if (stageAssertionsList) {
      stageAssertionsList.push(assertion);
    } else {
      assertionsByStage.set(assertion.stage_name, [assertion]);
    }

    if (assertion.stage_name === "recovery") {
      recoveryAssertionsBuffer.push(assertion);
    }

    if (assertion.passed) {
      continue;
    }

    failedAssertions.push(assertion);
    failedArtifactKeys.add(assertion.assertion_key);

    const failedAssertionsList = failedByStage.get(assertion.stage_name);
    if (failedAssertionsList) {
      failedAssertionsList.push(assertion);
    } else {
      failedByStage.set(assertion.stage_name, [assertion]);
    }

    if (!firstFailedArtifactPath) {
      const artifactPaths = extractArtifactPaths(
        assertion.evidence,
        `${assertion.actual} ${assertion.expected}`
      );
      firstFailedArtifactPath = artifactPaths[0] ?? null;
    }
  }

  const stageTraceItems: StageTraceItem[] = stageRuns
    .slice()
    .sort((a, b) => a.stage_order - b.stage_order)
    .map((stage) => ({
      stage,
      assertions: assertionsByStage.get(stage.stage_name) ?? [],
      failed: failedByStage.get(stage.stage_name) ?? [],
      assertionTotal:
        typeof stage.assertion_total === "number"
          ? stage.assertion_total
          : (assertionsByStage.get(stage.stage_name) ?? []).length,
      assertionFailed:
        typeof stage.assertion_failed === "number"
          ? stage.assertion_failed
          : (failedByStage.get(stage.stage_name) ?? []).length,
    }));

  const recoveryAssertions = recoveryAssertionsBuffer.slice(-10).reverse();

  const artifactTypeSet = new Set<string>();
  const artifactSearchLower = artifactSearchQuery.trim().toLowerCase();
  const artifactGroups: ArtifactGroupItem[] = (() => {
    if (taskRunArtifacts.length === 0) return [];
    const filteredArtifacts: Array<{
      item: TaskRunArtifact;
      pinned: boolean;
      failed: boolean;
      createdAtMs: number;
    }> = [];

    for (const item of taskRunArtifacts) {
      artifactTypeSet.add(item.artifact_type);
      if (artifactTypeFilter !== "all" && item.artifact_type !== artifactTypeFilter) {
        continue;
      }

      const failed = failedArtifactKeys.has(item.artifact_key);
      if (artifactFailedOnly && !failed) {
        continue;
      }

      if (artifactSearchLower) {
        const haystack = [
          item.artifact_type,
          item.artifact_key,
          item.value,
          item.metadata ?? "",
        ]
          .join("\n")
          .toLowerCase();
        if (!haystack.includes(artifactSearchLower)) {
          continue;
        }
      }

      filteredArtifacts.push({
        item,
        pinned: pinnedArtifactKeys.has(item.artifact_key),
        failed,
        createdAtMs: Date.parse(item.created_at),
      });
    }

    filteredArtifacts.sort((a, b) => {
      if (a.pinned !== b.pinned) return Number(b.pinned) - Number(a.pinned);
      if (artifactSortMode === "key") {
        return a.item.artifact_key.localeCompare(b.item.artifact_key);
      }
      if (artifactSortMode === "failed_first" && a.failed !== b.failed) {
        return Number(b.failed) - Number(a.failed);
      }
      return b.createdAtMs - a.createdAtMs;
    });

    const bucket = new Map<string, TaskRunArtifact[]>();
    for (const { item } of filteredArtifacts) {
      const group = item.artifact_type?.trim() || "unknown";
      const prev = bucket.get(group) ?? [];
      prev.push(item);
      bucket.set(group, prev);
    }

    return Array.from(bucket.entries())
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([type, items]) => ({ type, items }));
  })();

  const artifactTypeOptions = [
    "all",
    ...Array.from(artifactTypeSet).sort((a, b) => a.localeCompare(b)),
  ];

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
