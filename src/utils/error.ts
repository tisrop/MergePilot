export function getErrorMessage(error: unknown, fallback: string): string {
  if (error === null || error === undefined) {
    return fallback;
  }

  const message = String(error);
  return message || fallback;
}
