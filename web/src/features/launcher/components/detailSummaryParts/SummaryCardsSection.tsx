import type { SummaryCardsSectionProps } from "@/features/launcher/components/detailSummaryParts/sectionTypes";

export function SummaryCardsSection({
  runPhase,
  runStatus,
  runScore,
  nextActionHint,
}: SummaryCardsSectionProps) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
      <div className="text-xs rounded border border-white/15 bg-white/5 px-2 py-1.5 text-gray-200">
        <div className="font-semibold">최종 상태</div>
        <div className="mt-0.5">
          {runPhase} {runStatus ? `(${runStatus})` : ""}
        </div>
      </div>
      <div className="text-xs rounded border border-white/15 bg-white/5 px-2 py-1.5 text-gray-200">
        <div className="font-semibold">완성도</div>
        <div className="mt-0.5">{runScore ? `${runScore.score} · ${runScore.label}` : "n/a"}</div>
      </div>
      <div className="text-xs rounded border border-white/15 bg-white/5 px-2 py-1.5 text-gray-200">
        <div className="font-semibold">다음 액션</div>
        <div className="mt-0.5 line-clamp-2">{nextActionHint}</div>
      </div>
    </div>
  );
}
