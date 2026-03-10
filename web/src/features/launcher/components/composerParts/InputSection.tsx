import { Activity, ArrowUp } from "lucide-react";
import type { ComposerInputSectionProps } from "@/features/launcher/components/composerParts/sectionTypes";

export function ComposerInputRow({
  refs,
  ui,
  handlers,
}: ComposerInputSectionProps) {
  const { inputRef } = refs;

  return (
    <>
      <div className="launcher-input-wrap relative flex-1">
        <input
          ref={inputRef}
          type="text"
          className="launcher-input w-full h-11 sm:h-12 md:h-[50px] bg-white/[0.03] border border-white/15 rounded-xl px-3.5 sm:px-4 text-[16px] sm:text-[18px] md:text-[20px] text-white/95 placeholder-gray-500 outline-none focus:border-white/30 transition-colors"
          placeholder={
            ui.composerMode === "program"
              ? "버튼 또는 명령으로 실행"
              : ui.composerMode === "chat"
                ? "간단히 대화해보세요"
                : "무엇이든 부탁하세요"
          }
          value={ui.input}
          onChange={(e) => handlers.onInputChange(e.target.value)}
          onCompositionStart={handlers.onCompositionStart}
          onCompositionEnd={handlers.onCompositionEnd}
          onBlur={handlers.onInputBlur}
          onKeyDown={handlers.onInputKeyDown}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="none"
          spellCheck={false}
          autoFocus
        />
      </div>

      <button
        onClick={handlers.onSend}
        disabled={!ui.input.trim() || ui.isExecutionLocked || ui.loading}
        aria-label={ui.loading ? "Sending prompt" : "Send prompt"}
        className="launcher-send w-11 h-11 sm:w-12 sm:h-12 rounded-full bg-white/18 hover:bg-white/30 disabled:opacity-40 text-white flex items-center justify-center transition-colors"
      >
        {ui.loading ? (
          <Activity aria-hidden="true" className="w-5 h-5 animate-spin" />
        ) : (
          <ArrowUp aria-hidden="true" className="w-5 h-5" />
        )}
      </button>
      {ui.pendingDispatch && (
        <button
          onClick={handlers.onCancelPendingDispatch}
          className="h-12 px-3 rounded-xl border border-rose-400/35 bg-rose-500/15 text-rose-100 hover:bg-rose-500/25 transition-colors text-xs"
        >
          취소 ({ui.safeCountdownSeconds})
        </button>
      )}
    </>
  );
}
