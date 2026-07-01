type ClassDictionary = Record<string, boolean | null | undefined>;
type ClassValue = string | number | false | null | undefined | ClassDictionary | ClassValue[];

export function cn(...inputs: ClassValue[]) {
  return inputs.flatMap(classTokens).join(" ");
}

function classTokens(value: ClassValue): string[] {
  if (!value) return [];
  if (typeof value === "string" || typeof value === "number") return [String(value)];
  if (Array.isArray(value)) return value.flatMap(classTokens);
  return Object.entries(value)
    .filter(([, enabled]) => enabled)
    .map(([name]) => name);
}
