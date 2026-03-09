import type { StageDodSectionProps } from "@/features/launcher/components/diagnosticsParts/sectionTypes";

export function StageDodSection({ history }: StageDodSectionProps) {
  if (history.dodItems.length === 0) return null;

  return (
    <div className="px-4 py-3 border-b border-white/5 bg-[#181818]">
      <div className="text-[11px] uppercase tracking-wider text-emerald-300 font-semibold mb-2">
        Stage DoD
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        {history.dodItems.map((item) => (
          <div
            key={item.key}
            className={`text-xs rounded border px-2 py-1.5 ${
              item.done
                ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
                : "border-rose-400/30 bg-rose-500/10 text-rose-200"
            }`}
          >
            <div className="font-semibold">
              {item.done ? "✅" : "❌"} {item.label}
            </div>
            <div className="opacity-80 mt-0.5">{item.detail}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
