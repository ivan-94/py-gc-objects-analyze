export function formatNumber(value: number) {
  return new Intl.NumberFormat().format(value ?? 0);
}

export function formatBytes(value: number) {
  if (!value) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(unit ? 1 : 0)} ${units[unit]}`;
}

export function formatOptionalBytes(value: number | null) {
  return value === null ? "-" : formatBytes(value);
}

export function signedNumber(value: number) {
  return `${value > 0 ? "+" : ""}${formatNumber(value)}`;
}

export function signedBytes(value: number) {
  return `${value > 0 ? "+" : value < 0 ? "-" : ""}${formatBytes(Math.abs(value))}`;
}

export function formatPercent(value: number) {
  return `${(value * 100).toFixed(1)}%`;
}
