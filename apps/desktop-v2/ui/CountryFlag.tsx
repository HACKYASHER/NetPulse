export function countryCode(name: string) {
  const emoji = name.match(/[\u{1F1E6}-\u{1F1FF}]{2}/u)?.[0];
  if (emoji)
    return [...emoji]
      .map((c) => String.fromCharCode(c.codePointAt(0)! - 0x1f1e6 + 65))
      .join("");
  const iso = name.match(
    /^\s*(FI|DE|RU|US|NL|FR|GB|SE|PL|TR|JP|SG|KZ|EE|LV|LT)\b/i,
  )?.[1];
  if (iso) return iso.toUpperCase();
  for (const [pattern, code] of [
    [/finland|финлянд/i, "FI"],
    [/germany|германи/i, "DE"],
    [/netherlands|нидерланд/i, "NL"],
    [/france|франци/i, "FR"],
    [/sweden|швеци/i, "SE"],
    [/russia|росси/i, "RU"],
    [/united states|сша/i, "US"],
  ] as const) {
    if (pattern.test(name)) return code;
  }
  return null;
}
export function serverName(name: string) {
  return name
    .replace(/[\u{1F1E6}-\u{1F1FF}]{2}\s*/u, "")
    .replace(/^\s*(FI|DE|RU|US|NL|FR|GB|SE|PL|TR|JP|SG|KZ|EE|LV|LT)\s+/i, "");
}
export function CountryFlag({ name }: { name: string }) {
  const code = countryCode(name);
  if (!code)
    return (
      <span className="country-unknown" aria-label="Country unknown">
        🌐
      </span>
    );
  let art;
  if (code === "FI")
    art = (
      <>
        <rect width="30" height="20" fill="#fff" />
        <path d="M0 10H30M10 0V20" stroke="#003580" strokeWidth="5" />
      </>
    );
  else if (["DE", "RU", "NL", "FR", "EE", "LT"].includes(code)) {
    const colors: Record<string, string[]> = {
      DE: ["#151515", "#DD0000", "#FFCE00"],
      RU: ["#fff", "#2050C0", "#D52B1E"],
      NL: ["#AE1C28", "#fff", "#21468B"],
      FR: ["#002395", "#fff", "#ED2939"],
      EE: ["#4891D9", "#111", "#fff"],
      LT: ["#FDB913", "#006A44", "#C1272D"],
    };
    art = colors[code].map((fill, i) => (
      <rect
        key={i}
        x={code === "FR" ? i * 10 : 0}
        y={code === "FR" ? 0 : (i * 20) / 3}
        width={code === "FR" ? 10 : 30}
        height={code === "FR" ? 20 : 20 / 3}
        fill={fill}
      />
    ));
  } else if (code === "SE")
    art = (
      <>
        <rect width="30" height="20" fill="#006AA7" />
        <path d="M0 10H30M10 0V20" stroke="#FECC00" strokeWidth="4" />
      </>
    );
  else if (code === "PL")
    art = (
      <>
        <rect width="30" height="10" fill="#fff" />
        <rect y="10" width="30" height="10" fill="#DC143C" />
      </>
    );
  else if (code === "JP")
    art = (
      <>
        <rect width="30" height="20" fill="#fff" />
        <circle cx="15" cy="10" r="6" fill="#BC002D" />
      </>
    );
  else
    return (
      <span className="country-code" title={code}>
        {code}
      </span>
    );
  return (
    <svg
      className="country-flag"
      width="27"
      height="18"
      viewBox="0 0 30 20"
      role="img"
      aria-label={code}
    >
      <title>{code}</title>
      {art}
    </svg>
  );
}
