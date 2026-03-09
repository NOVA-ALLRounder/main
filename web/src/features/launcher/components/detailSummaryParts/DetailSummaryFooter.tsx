import type { DetailSummaryFooterProps } from "@/features/launcher/components/detailSummaryParts/sectionTypes";

export function DetailSummaryFooter({
  dodHistoryLoading,
  showDiagnostics,
  onLoadDodHistory,
  onToggleDiagnostics,
}: DetailSummaryFooterProps) {
  return (
    <div className="mt-2 flex items-center justify-end gap-2">
      <button
        onClick={onLoadDodHistory}
        disabled={dodHistoryLoading}
        className="text-[11px] px-2.5 py-1 rounded-full border border-sky-400/35 bg-sky-500/15 text-sky-100 hover:bg-sky-500/25 disabled:opacity-50"
      >
        {dodHistoryLoading ? "히스토리 새로고침..." : "히스토리 새로고침"}
      </button>
      <button
        onClick={onToggleDiagnostics}
        className="text-[11px] px-2.5 py-1 rounded-full border border-white/15 bg-white/5 hover:bg-white/10"
      >
        {showDiagnostics ? "진단 접기" : "진단 펼치기"}
      </button>
    </div>
  );
}
