import { artifactPathLabel, extractArtifactPaths, type ArtifactSortMode } from "@/features/launcher/support";
import { compactEvidence, compactMetadata } from "@/features/launcher/components/diagnosticsParts/support";
import type { PersistedStageTraceSectionProps } from "@/features/launcher/components/diagnosticsParts/sectionTypes";

export function PersistedStageTraceSection({
  execution,
  stage,
  artifacts,
  handlers,
}: PersistedStageTraceSectionProps) {
  if (
    stage.stageRuns.length === 0 &&
    stage.stageAssertions.length === 0 &&
    stage.taskRunArtifacts.length === 0
  ) {
    return null;
  }

  return (
    <div className="px-4 py-3 border-b border-white/5 bg-[#151515]">
      <div className="text-[11px] uppercase tracking-wider text-sky-300 font-semibold mb-2">
        Persisted Stage Trace
      </div>
      {stage.artifactActionMessage && (
        <div className="text-[11px] rounded border border-white/15 bg-white/5 px-2 py-1.5 text-gray-200 mb-2">
          {stage.artifactActionMessage}
        </div>
      )}
      {stage.recoveryAssertions.length > 0 && (
        <div className="mb-2 rounded border border-amber-400/25 bg-amber-500/10 px-2 py-2">
          <div className="text-[11px] uppercase tracking-wider text-amber-200 font-semibold mb-1">
            Recovery Timeline
          </div>
          <div className="space-y-1">
            {stage.recoveryAssertions.map((item) => (
              <div
                key={`recovery-${item.id}`}
                className={`text-[11px] rounded px-2 py-1 border ${
                  item.passed
                    ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
                    : "border-rose-400/30 bg-rose-500/10 text-rose-200"
                }`}
              >
                <div className="font-medium">
                  {item.passed ? "✅" : "❌"} {item.assertion_key}
                </div>
                <div className="opacity-80">
                  expected={item.expected} actual={item.actual}
                </div>
                {item.evidence && <div className="opacity-75 line-clamp-1">{item.evidence}</div>}
                <div className="opacity-60">{new Date(item.created_at).toLocaleTimeString()}</div>
              </div>
            ))}
          </div>
        </div>
      )}
      {(execution.runPhase === "failed" || execution.lastStatus === "manual_required") &&
        execution.firstFailedArtifactPath && (
          <div className="mb-2">
            <button
              onClick={handlers.onGuidedRecovery}
              disabled={execution.loading || execution.artifactOpenBusy === execution.firstFailedArtifactPath}
              className="text-[11px] px-2.5 py-1.5 rounded border border-sky-400/40 bg-sky-500/15 text-sky-100 hover:bg-sky-500/25 disabled:opacity-50"
              title={execution.firstFailedArtifactPath}
            >
              {execution.lastStatus === "manual_required"
                ? "실패 증거 열고 Resume"
                : `실패 증거 열기 (${artifactPathLabel(execution.firstFailedArtifactPath)})`}
            </button>
          </div>
        )}
      {stage.stageTraceItems.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2 mb-2">
          {stage.stageTraceItems.map((row) => (
            <div
              key={`stage-${row.stage.id}`}
              className={`text-xs rounded border px-2 py-1.5 ${
                row.stage.status === "completed"
                  ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
                  : row.stage.status === "running"
                    ? "border-sky-400/30 bg-sky-500/10 text-sky-200"
                    : row.stage.status === "retrying"
                      ? "border-amber-400/30 bg-amber-500/10 text-amber-200"
                      : row.stage.status === "blocked"
                        ? "border-amber-500/40 bg-amber-600/10 text-amber-100"
                        : "border-rose-400/30 bg-rose-500/10 text-rose-200"
              }`}
            >
              <div className="font-semibold">
                {row.stage.status === "completed"
                  ? "✅"
                  : row.stage.status === "running"
                    ? "⏳"
                    : row.stage.status === "retrying"
                      ? "🔁"
                      : row.stage.status === "blocked"
                        ? "⛔"
                        : "❌"}{" "}
                {row.stage.stage_order}. {row.stage.stage_name}
              </div>
              <div className="opacity-80 mt-0.5">
                {row.stage.status} · assertions {row.assertionTotal} · failed {row.assertionFailed}
              </div>
              {(row.stage.retry_count ?? 0) > 0 && (
                <div className="opacity-80 mt-0.5">
                  retry {row.stage.retry_count}
                  {typeof row.stage.max_retries === "number" && row.stage.max_retries > 0
                    ? `/${row.stage.max_retries}`
                    : ""}
                  {row.stage.next_retry_at
                    ? ` · next=${new Date(row.stage.next_retry_at).toLocaleTimeString()}`
                    : ""}
                </div>
              )}
              {row.stage.details && <div className="opacity-70 mt-0.5 line-clamp-2">{row.stage.details}</div>}
              {row.failed.length > 0 && (
                <div className="mt-1.5 space-y-1">
                  {row.failed.slice(0, 2).map((a) => {
                    const artifactPaths = extractArtifactPaths(
                      a.evidence,
                      row.stage.details,
                      `${a.actual} ${a.expected}`,
                    );
                    return (
                      <div key={`stage-${row.stage.id}-assert-${a.id}`} className="opacity-90">
                        <div>
                          • {a.assertion_key}: expected={a.expected} actual={a.actual}
                          {a.evidence ? ` | evidence=${compactEvidence(a.evidence)}` : ""}
                        </div>
                        {artifactPaths.length > 0 && (
                          <div className="mt-1 flex flex-wrap gap-1">
                            {artifactPaths.map((artifactPath) => (
                              <button
                                key={`open-${a.id}-${artifactPath}`}
                                onClick={() => handlers.onOpenArtifactPath(artifactPath)}
                                disabled={execution.artifactOpenBusy === artifactPath}
                                className="text-[10px] px-2 py-0.5 rounded border border-sky-400/40 bg-sky-500/15 text-sky-100 hover:bg-sky-500/25 disabled:opacity-50"
                                title={artifactPath}
                              >
                                {execution.artifactOpenBusy === artifactPath
                                  ? "열기..."
                                  : `열기 ${artifactPathLabel(artifactPath)}`}
                              </button>
                            ))}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      {stage.taskRunArtifacts.length > 0 && (
        <div className="mb-2 rounded border border-indigo-400/20 bg-indigo-500/10 px-2 py-2">
          <div className="flex items-center justify-between gap-2 mb-1">
            <div className="text-[11px] uppercase tracking-wider text-indigo-200 font-semibold">
              Run Artifacts ({stage.taskRunArtifacts.length})
            </div>
            <div className="flex items-center gap-1.5">
              <select
                value={artifacts.artifactTypeFilter}
                onChange={(e) => handlers.onArtifactTypeFilterChange(e.target.value)}
                className="text-[10px] rounded border border-white/20 bg-black/30 px-2 py-0.5 text-gray-200"
                title="아티팩트 타입 필터"
              >
                {artifacts.artifactTypeOptions.map((option) => (
                  <option key={`artifact-filter-${option}`} value={option}>
                    {option === "all" ? "all types" : option}
                  </option>
                ))}
              </select>
              <button
                onClick={handlers.onArtifactFailedOnlyToggle}
                className={`text-[10px] px-2 py-0.5 rounded border ${
                  artifacts.artifactFailedOnly
                    ? "border-rose-400/50 bg-rose-500/20 text-rose-100"
                    : "border-white/20 bg-black/30 text-gray-300"
                }`}
                title="실패한 assertion 키와 연결된 artifact만 보기"
              >
                failed only {artifacts.artifactFailedOnly ? "ON" : "OFF"}
              </button>
              <select
                value={artifacts.artifactSortMode}
                onChange={(e) => handlers.onArtifactSortModeChange(e.target.value as ArtifactSortMode)}
                className="text-[10px] rounded border border-white/20 bg-black/30 px-2 py-0.5 text-gray-200"
                title="아티팩트 정렬"
              >
                <option value="failed_first">failed first</option>
                <option value="newest">newest</option>
                <option value="key">key</option>
              </select>
              <input
                value={artifacts.artifactSearchQuery}
                onChange={(e) => handlers.onArtifactSearchQueryChange(e.target.value)}
                placeholder="search key/value"
                className="text-[10px] rounded border border-white/20 bg-black/30 px-2 py-0.5 text-gray-200 w-28"
              />
            </div>
          </div>
          <div className="space-y-2">
            {artifacts.artifactGroups.length === 0 && (
              <div className="text-[11px] text-gray-400 rounded border border-white/10 bg-black/20 px-2 py-1.5">
                필터 조건에 맞는 artifact가 없습니다.
              </div>
            )}
            {artifacts.artifactGroups.map((group) => (
              <div
                key={`artifact-group-${group.type}`}
                className="rounded border border-white/10 bg-black/20 px-2 py-1.5"
              >
                <div className="text-[11px] font-semibold text-indigo-100 mb-1">
                  {group.type} ({group.items.length})
                </div>
                <div className="space-y-1">
                  {group.items.slice(0, 6).map((artifact) => (
                    <div key={`artifact-${artifact.id}`} className="text-[11px] text-gray-200">
                      <div className="flex items-center justify-between gap-2">
                        <div className="font-mono text-[10px] text-indigo-100">
                          {artifact.artifact_key}
                          {artifacts.failedArtifactKeys.has(artifact.artifact_key) && (
                            <span className="ml-1 text-rose-300">• failed</span>
                          )}
                          {artifacts.pinnedArtifactKeys.has(artifact.artifact_key) && (
                            <span className="ml-1 text-amber-300">• pinned</span>
                          )}
                        </div>
                        <div className="flex items-center gap-1">
                          <button
                            onClick={() => handlers.onTogglePinArtifactKey(artifact.artifact_key)}
                            className={`text-[10px] px-2 py-0.5 rounded border ${
                              artifacts.pinnedArtifactKeys.has(artifact.artifact_key)
                                ? "border-amber-300/45 bg-amber-400/20 text-amber-100"
                                : "border-white/20 bg-black/20 text-gray-300"
                            }`}
                          >
                            pin
                          </button>
                          <button
                            onClick={() => handlers.onCopyArtifactPayload(artifact)}
                            className="text-[10px] px-2 py-0.5 rounded border border-indigo-300/35 bg-indigo-400/20 text-indigo-100 hover:bg-indigo-400/30"
                          >
                            copy
                          </button>
                        </div>
                      </div>
                      <div>{compactEvidence(artifact.value)}</div>
                      {artifact.metadata && (
                        <div className="text-gray-400">metadata={compactMetadata(artifact.metadata)}</div>
                      )}
                    </div>
                  ))}
                  {group.items.length > 6 && (
                    <div className="text-[10px] text-gray-400">... {group.items.length - 6} more</div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
      {stage.failedAssertions.length > 0 && (
        <div className="text-xs rounded border border-rose-500/30 bg-rose-500/10 text-rose-200 px-2 py-1.5">
          <div className="font-semibold mb-1">Failed Assertions ({stage.failedAssertions.length})</div>
          <div className="space-y-1">
            {stage.failedAssertions.slice(0, 4).map((a) => (
              <div key={`assert-${a.id}`} className="opacity-90">
                {a.stage_name}.{a.assertion_key}: expected={a.expected} actual={a.actual}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
