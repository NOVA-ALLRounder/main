export function compactEvidence(raw?: string | null) {
  if (!raw) return "";
  return raw.length > 140 ? `${raw.slice(0, 140)}...` : raw;
}

export function compactMetadata(raw?: string | null) {
  if (!raw) return "";
  const text = raw.trim();
  if (!text) return "";
  try {
    const parsed = JSON.parse(text) as unknown;
    const normalized = JSON.stringify(parsed);
    return normalized.length > 160 ? `${normalized.slice(0, 160)}...` : normalized;
  } catch {
    return text.length > 160 ? `${text.slice(0, 160)}...` : text;
  }
}
