import type { ComposerQuickStripSectionProps } from "@/features/launcher/components/composerParts/sectionTypes";

export function ComposerQuickStripSection({
  ui,
  quickActions,
  handlers,
}: ComposerQuickStripSectionProps) {
  if (ui.composerMode === "nl" && !ui.input.trim()) {
    return (
      <div className="launcher-quick-strip mt-2.5 rounded-xl border border-white/10 bg-[#1b1b1b]/90 p-2.5 flex flex-nowrap gap-2 overflow-x-auto">
        {quickActions.nlSuggestions.map((suggestion) => (
          <button
            key={suggestion}
            onClick={() => handlers.onSuggestionClick(suggestion)}
            disabled={ui.isExecutionLocked}
            className="text-xs px-3.5 py-1.5 rounded-full bg-white/8 text-gray-200 hover:bg-white/15 border border-white/10 transition-colors whitespace-nowrap disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {suggestion}
          </button>
        ))}
      </div>
    );
  }

  if (ui.composerMode === "chat" && !ui.input.trim()) {
    return (
      <div className="launcher-quick-strip mt-2.5 rounded-xl border border-white/10 bg-[#1b1b1b]/90 p-2.5 flex flex-nowrap gap-2 overflow-x-auto">
        {quickActions.chatSuggestions.map((suggestion) => (
          <button
            key={suggestion}
            onClick={() => handlers.onSuggestionClick(suggestion)}
            disabled={ui.isExecutionLocked}
            className="text-xs px-3.5 py-1.5 rounded-full bg-white/8 text-gray-200 hover:bg-white/15 border border-white/10 transition-colors whitespace-nowrap disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {suggestion}
          </button>
        ))}
      </div>
    );
  }

  if (ui.composerMode === "program") {
    return (
      <div className="launcher-quick-strip mt-2.5 rounded-xl border border-white/10 bg-[#1b1b1b]/90 p-2.5 flex flex-nowrap gap-2 overflow-x-auto">
        {quickActions.programActions.map((action) => (
          <button
            key={action.key}
            onClick={() => handlers.onQuickProgramAction(action)}
            disabled={ui.isExecutionLocked}
            className="text-xs px-3.5 py-1.5 rounded-full bg-white/8 text-gray-100 hover:bg-white/15 border border-white/10 disabled:opacity-50 whitespace-nowrap"
          >
            {action.label}
          </button>
        ))}
      </div>
    );
  }

  return null;
}
