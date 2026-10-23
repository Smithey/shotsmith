import en from "./messages/en.json";

const catalogs: Record<string, Record<string, string>> = { en };

export function t(key: string): string {
  const locale = document.documentElement.lang || "en";
  return catalogs[locale]?.[key] ?? catalogs.en[key] ?? key;
}
