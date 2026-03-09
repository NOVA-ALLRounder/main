export function getModeButtonClass(selected: boolean, disabled: boolean) {
  return `px-3 py-1.5 text-xs rounded-lg font-medium transition-colors ${
    selected ? "bg-white/15 text-white" : "text-gray-400 hover:text-gray-200"
  } ${disabled ? "opacity-50 cursor-not-allowed" : ""}`;
}

export function getAdvancedToggleClass(enabled: boolean, disabled: boolean) {
  return `px-2.5 py-1.5 text-[11px] rounded-xl border transition-colors shrink-0 ${
    enabled
      ? "border-cyan-400/35 bg-cyan-500/15 text-cyan-200"
      : "border-white/20 bg-white/5 text-gray-300 hover:bg-white/10"
  } ${disabled ? "opacity-50 cursor-not-allowed" : ""}`;
}

export function getProfileOptionClass(active: boolean, disabled: boolean) {
  return `px-2.5 py-1.5 text-xs rounded-lg font-medium transition-colors ${
    active ? "bg-white/15 text-white" : "text-gray-400 hover:text-gray-200"
  } ${disabled ? "opacity-50 cursor-not-allowed" : ""}`;
}

export function getPillButtonClass(
  enabled: boolean,
  disabled: boolean,
  enabledClass: string,
) {
  return `px-2 py-1 rounded-full border text-[11px] transition-colors ${
    enabled ? enabledClass : "border-white/20 bg-white/5 text-gray-300"
  } ${disabled ? "opacity-50 cursor-not-allowed" : ""}`;
}
