import type { Subscription } from "./types";
export function subscriptionUsage(s: Subscription, lang: string): string {
  const ru = lang === "ru",
    u = s.usage;
  const bytes = (n: number) => {
    const i = Math.min(
      4,
      Math.floor(Math.log(Math.max(1, n)) / Math.log(1024)),
    );
    return (
      (n / 1024 ** i).toLocaleString(lang, { maximumFractionDigits: 1 }) +
      " " +
      ["B", "KiB", "MiB", "GiB", "TiB"][i]
    );
  };
  let traffic = ru
    ? "Трафик: провайдер не передал данные"
    : "Traffic: not provided";
  if (u?.total === 0) traffic = ru ? "Трафик без лимита" : "Unlimited traffic";
  else if (u?.total != null) {
    traffic =
      u.upload != null && u.download != null
        ? (ru ? "Осталось " : "Remaining ") +
          bytes(Math.max(0, u.total - u.upload - u.download)) +
          " / " +
          bytes(u.total)
        : (ru ? "Лимит " : "Limit ") +
          bytes(u.total) +
          (ru ? " · остаток неизвестен" : " · remaining unknown");
  }
  const expires =
    u?.expire && Number.isFinite(new Date(u.expire * 1000).getTime())
      ? (ru ? "До " : "Until ") +
        new Date(u.expire * 1000).toLocaleDateString(lang)
      : u?.expire === 0
        ? ru
          ? "Без срока"
          : "No expiry"
        : ru
          ? "Срок: не указан"
          : "Expiry: not provided";
  return traffic + " · " + expires;
}
