import type { DiagnosticsOverviewSectionProps } from "@/features/launcher/components/diagnosticsParts/sectionTypes";

export function DiagnosticsOverviewSection({
  telemetry,
  history,
  execution,
  handlers,
}: DiagnosticsOverviewSectionProps) {
  return (
    <div className="px-4 py-3 border-b border-white/5 bg-[#151515]">
      <div className="mb-2 rounded border border-white/10 bg-white/5 px-2 py-2">
        <div className="text-[11px] uppercase tracking-wider text-cyan-200 font-semibold mb-1">
          Singleton Lock Telemetry
        </div>
        {telemetry.lockMetrics ? (
          <div className="grid grid-cols-2 md:grid-cols-5 gap-1.5 text-[11px]">
            <div className="rounded border border-emerald-400/25 bg-emerald-500/10 px-2 py-1 text-emerald-200">
              acquired={telemetry.lockMetrics.acquired}
            </div>
            <div className="rounded border border-sky-400/25 bg-sky-500/10 px-2 py-1 text-sky-200">
              bypassed={telemetry.lockMetrics.bypassed}
            </div>
            <div className="rounded border border-rose-400/25 bg-rose-500/10 px-2 py-1 text-rose-200">
              blocked={telemetry.lockMetrics.blocked}
            </div>
            <div className="rounded border border-amber-400/25 bg-amber-500/10 px-2 py-1 text-amber-100">
              stale_recovered={telemetry.lockMetrics.stale_recovered}
            </div>
            <div className="rounded border border-fuchsia-400/25 bg-fuchsia-500/10 px-2 py-1 text-fuchsia-200">
              rejected={telemetry.lockMetrics.rejected}
            </div>
          </div>
        ) : (
          <div className="text-xs text-gray-400">
            {telemetry.lockMetricsError ?? "아직 lock telemetry를 불러오지 않았습니다."}
          </div>
        )}
      </div>
      <div className="mb-2 rounded border border-white/10 bg-white/5 px-2 py-2">
        <div className="text-[11px] uppercase tracking-wider text-cyan-200 font-semibold mb-1">
          Runtime Core Info
        </div>
        {telemetry.runtimeInfo ? (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-1.5 text-[11px]">
            <div className="rounded border border-white/15 bg-black/20 px-2 py-1 text-gray-200">
              service={telemetry.runtimeInfo.service} · version={telemetry.runtimeInfo.version} ·
              profile={telemetry.runtimeInfo.profile}
            </div>
            <div className="rounded border border-white/15 bg-black/20 px-2 py-1 text-gray-200">
              pid={telemetry.runtimeInfo.pid} · port={telemetry.runtimeInfo.api_port} · no_key=
              {telemetry.runtimeInfo.allow_no_key ? "1" : "0"}
            </div>
            <div className="rounded border border-white/15 bg-black/20 px-2 py-1 text-gray-300 md:col-span-2 break-all">
              binary={telemetry.runtimeInfo.binary_path ?? "unknown"}
            </div>
            <div className="rounded border border-white/15 bg-black/20 px-2 py-1 text-gray-400 md:col-span-2 break-all">
              cwd={telemetry.runtimeInfo.current_dir ?? "unknown"} · started_at=
              {telemetry.runtimeInfo.started_at}
            </div>
          </div>
        ) : (
          <div className="text-xs text-gray-400">
            {telemetry.runtimeInfoError ?? "아직 runtime info를 불러오지 않았습니다."}
          </div>
        )}
      </div>
      <div className="text-[11px] uppercase tracking-wider text-cyan-300 font-semibold mb-2">
        최근 DoD 히스토리
      </div>
      {history.dodHistory.length === 0 ? (
        <div className="text-xs text-gray-400">
          {history.dodHistoryLoading ? "히스토리를 불러오는 중..." : "기록된 실행 히스토리가 없습니다."}
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          {history.dodHistory.map((item) => {
            const assertionsPass = item.assertionTotal === 0 || item.assertionFailed === 0;
            const stagePass = item.plannerComplete && item.executionComplete && item.businessComplete;
            const pass = stagePass && assertionsPass;
            return (
              <div
                key={item.runId}
                className={`text-xs rounded border px-2 py-1.5 ${
                  pass
                    ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
                    : "border-rose-400/30 bg-rose-500/10 text-rose-200"
                }`}
              >
                <div className="font-semibold">
                  {pass ? "✅" : "❌"} {item.runId}
                </div>
                <div className="opacity-80 mt-0.5">
                  status={item.status} · assertions={item.assertionTotal - item.assertionFailed}/
                  {item.assertionTotal}
                </div>
                <div className="opacity-70 mt-0.5">
                  planner={item.plannerComplete ? "1" : "0"} · execution=
                  {item.executionComplete ? "1" : "0"} · business=
                  {item.businessComplete ? "1" : "0"}
                </div>
              </div>
            );
          })}
        </div>
      )}
      <div className="mt-2 rounded border border-white/10 bg-white/5 px-2 py-2">
        <div className="text-[11px] uppercase tracking-wider text-rose-200 font-semibold mb-1">
          반복 실패 Top 3
        </div>
        {history.dodFailureTop.length === 0 ? (
          <div className="text-xs text-gray-400">최근 반복 실패 항목이 없습니다.</div>
        ) : (
          <div className="space-y-1">
            {history.dodFailureTop.map((item, idx) => {
              const action = handlers.onRecoverFailureKey(item.key);
              return (
                <div
                  key={`top-fail-${item.key}`}
                  className="text-xs rounded border border-rose-400/25 bg-rose-500/10 px-2 py-1 text-rose-100"
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="font-semibold">
                      {idx + 1}. {item.key} ({item.count})
                    </div>
                    {action && (
                      <button
                        onClick={() => handlers.onRunRecoveryAction(action)}
                        disabled={execution.loading || !!execution.recoveryActionBusyKey}
                        className="text-[10px] px-2 py-0.5 rounded-full border border-rose-300/40 bg-rose-400/20 hover:bg-rose-400/30 disabled:opacity-50"
                      >
                        복구 실행
                      </button>
                    )}
                  </div>
                  <div className="opacity-80 mt-0.5">actual={item.sampleActual || "n/a"}</div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
