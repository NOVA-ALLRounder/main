import { AppWindow, Circle, Globe, MessageCircle, Mic, Plus, Wand2 } from "lucide-react";
import type { ComposerToolbarSectionProps } from "@/features/launcher/components/composerParts/sectionTypes";

export function ComposerToolbarSection({
  ui,
  runtime,
  handlers,
}: ComposerToolbarSectionProps) {
  return (
    <div className="launcher-toolbar mt-2.5 flex items-center justify-between text-gray-300">
      <div className="flex items-center gap-1.5">
        <button
          onClick={handlers.onCycleMode}
          disabled={ui.isExecutionLocked}
          className="p-2 rounded-full hover:bg-white/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="모드 순환"
          title="모드 순환"
        >
          <Plus aria-hidden="true" className="w-4 h-4" />
        </button>
        <button
          onClick={handlers.onApplyWebSearchTemplate}
          disabled={ui.isExecutionLocked}
          className="p-2 rounded-full hover:bg-white/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="웹 검색 템플릿"
          title="웹 검색 템플릿"
        >
          <Globe aria-hidden="true" className="w-4 h-4" />
        </button>
        <button
          onClick={handlers.onApplySummaryTemplate}
          disabled={ui.isExecutionLocked}
          className="p-2 rounded-full hover:bg-white/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="요약 템플릿"
          title="요약 템플릿"
        >
          <Wand2 aria-hidden="true" className="w-4 h-4" />
        </button>
        <button
          onClick={() => handlers.onModeSelect("program")}
          disabled={ui.isExecutionLocked}
          className="p-2 rounded-full hover:bg-white/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="프로그램 버튼"
          title="프로그램 버튼"
        >
          <AppWindow aria-hidden="true" className="w-4 h-4" />
        </button>
        <button
          onClick={() => handlers.onModeSelect("chat")}
          disabled={ui.isExecutionLocked}
          className="p-2 rounded-full hover:bg-white/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="대화 모드"
          title="대화 모드"
        >
          <MessageCircle aria-hidden="true" className="w-4 h-4" />
        </button>
        <button
          onClick={() => handlers.onTelegramListenerCommand("telegram listener start")}
          disabled={ui.loading || ui.isExecutionLocked}
          className="text-[11px] px-2.5 py-1 rounded-full border border-sky-400/30 bg-sky-500/15 text-sky-200 hover:bg-sky-500/25 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="텔레그램 리스너 시작"
        >
          TG 시작
        </button>
        <button
          onClick={() => handlers.onTelegramListenerCommand("telegram listener status")}
          disabled={ui.loading || ui.isExecutionLocked}
          className="text-[11px] px-2.5 py-1 rounded-full border border-white/20 bg-white/5 text-gray-200 hover:bg-white/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="텔레그램 리스너 상태"
        >
          TG 상태
        </button>
        <span className="text-sm font-semibold tracking-wide text-gray-200 ml-1">
          <span className="text-cyan-300 font-extrabold">A</span>llv
          <span className="text-cyan-300 font-extrabold">I</span>a
        </span>
        <span className="text-xl font-semibold text-gray-300">5.2</span>
        {runtime.info && (
          <span
            className={`text-[11px] px-2 py-0.5 rounded-full border ${
              runtime.coreBinaryKind === "workspace"
                ? "border-emerald-400/35 bg-emerald-500/15 text-emerald-200"
                : runtime.coreBinaryKind === "bundle"
                  ? "border-amber-400/35 bg-amber-500/15 text-amber-100"
                  : "border-white/20 bg-white/5 text-gray-300"
            }`}
            title={`pid=${runtime.info.pid} | ${runtime.info.binary_path ?? "unknown"}`}
          >
            core {runtime.info.version} · {runtime.coreBinaryKind}
          </span>
        )}
      </div>

      <div className="flex items-center gap-1.5">
        {ui.composerMode !== "chat" && ui.hasDetailContent && (
          <button
            onClick={handlers.onToggleDetailPanel}
            className="text-xs px-2.5 py-1.5 rounded-full border border-white/15 bg-white/5 hover:bg-white/10 transition-colors"
          >
            {ui.showDetailPanel
              ? "결과 숨기기"
              : `결과 보기${ui.suggestionCount > 0 ? ` (${ui.suggestionCount})` : ""}`}
          </button>
        )}
        {ui.runScore && (
          <span
            className={`text-xs px-2.5 py-1.5 rounded-full border ${
              ui.runScore.pass
                ? "border-emerald-400/40 bg-emerald-500/15 text-emerald-200"
                : "border-rose-400/40 bg-rose-500/15 text-rose-200"
            }`}
          >
            완성도 {ui.runScore.score} · {ui.runScore.label}
          </span>
        )}
        <button
          className="p-2 rounded-full hover:bg-white/10 text-gray-300 transition-colors"
          aria-label="record"
          title="record"
        >
          <Circle aria-hidden="true" className="w-6 h-6" />
        </button>
        <button
          className="p-2 rounded-full hover:bg-white/10 text-gray-300 transition-colors"
          aria-label="voice"
          title="voice"
        >
          <Mic aria-hidden="true" className="w-6 h-6" />
        </button>
      </div>
    </div>
  );
}
