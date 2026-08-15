export function normalizeIntegerRaw(raw: string) {
  if (raw === "-" || raw === "") {
    return raw;
  }

  const negative = raw.startsWith("-");
  const digits = (negative ? raw.slice(1) : raw)
    .replace(/\D.*$/s, "")
    .replace(/^0+(?=\d)/, "");
  return `${negative ? "-" : ""}${digits}`;
}

export function normalizeDecimalRaw(raw: string) {
  if (raw === "-" || raw === "" || raw === "." || raw === "-.") {
    return raw;
  }

  const negative = raw.startsWith("-");
  const body = (negative ? raw.slice(1) : raw).replace(/[^\d.]/g, "");
  const dotIndex = body.indexOf(".");
  const digits =
    dotIndex === -1
      ? body
      : `${body.slice(0, dotIndex)}.${body.slice(dotIndex + 1).replace(/\./g, "")}`;
  const value = digits.replace(/^0+(?=\d)/, "");
  if (value === "" || value === ".") {
    return negative ? "-" : value;
  }
  return `${negative ? "-" : ""}${value}`;
}
