import type { Platform } from "@/types";

export const PR_CREATE_WARNING_QUERY = "create_warning";

const STORAGE_PREFIX = "mergebeacon:pr-create-warnings:v1";
const MAX_WARNING_COUNT = 20;
const MAX_WARNING_LENGTH = 2_000;

function storageKey(platform: Platform, owner: string, repo: string, number: number): string {
  return [STORAGE_PREFIX, platform, owner, repo, String(number)].map(encodeURIComponent).join(":");
}

function normalizeWarnings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((message): message is string => typeof message === "string")
    .map((message) => message.trim().slice(0, MAX_WARNING_LENGTH))
    .filter(Boolean)
    .slice(0, MAX_WARNING_COUNT);
}

export function persistPrCreateWarnings(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
  warnings: string[],
): void {
  const normalized = normalizeWarnings(warnings);
  if (normalized.length === 0) return;
  try {
    window.sessionStorage.setItem(
      storageKey(platform, owner, repo, number),
      JSON.stringify(normalized),
    );
  } catch {
    // The query marker still lets the detail page show a generic warning.
  }
}

export function readPrCreateWarnings(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): string[] {
  try {
    const raw = window.sessionStorage.getItem(storageKey(platform, owner, repo, number));
    return raw ? normalizeWarnings(JSON.parse(raw)) : [];
  } catch {
    return [];
  }
}

export function clearPrCreateWarnings(
  platform: Platform,
  owner: string,
  repo: string,
  number: number,
): void {
  try {
    window.sessionStorage.removeItem(storageKey(platform, owner, repo, number));
  } catch {
    // Session storage can be unavailable in restricted WebView contexts.
  }
}
