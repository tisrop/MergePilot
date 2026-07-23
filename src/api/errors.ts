import type { CommandErrorCode, CommandErrorPayload } from "@/types";

const COMMAND_ERROR_CODES = new Set<CommandErrorCode>([
  "validation",
  "authentication",
  "permission_denied",
  "not_found",
  "conflict",
  "rate_limited",
  "network",
  "timeout",
  "invalid_response",
  "storage",
  "unsupported",
  "ai",
  "platform",
  "unknown",
]);

function containsControlCharacters(value: string): boolean {
  return Array.from(value).some((character) => {
    const code = character.charCodeAt(0);
    return code <= 31 || code === 127;
  });
}

export class ApiError extends Error {
  readonly code: CommandErrorCode;
  readonly retryable: boolean;
  readonly requestId?: string;
  readonly httpStatus?: number;

  constructor(payload: CommandErrorPayload) {
    super(payload.message);
    this.name = "ApiError";
    this.code = payload.code;
    this.retryable = payload.retryable;
    this.requestId = payload.request_id;
    this.httpStatus = payload.http_status;
  }

  override toString(): string {
    return this.message;
  }
}

function isCommandErrorPayload(value: unknown): value is CommandErrorPayload {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<CommandErrorPayload>;
  return (
    typeof candidate.code === "string" &&
    COMMAND_ERROR_CODES.has(candidate.code as CommandErrorCode) &&
    typeof candidate.message === "string" &&
    candidate.message.trim().length > 0 &&
    typeof candidate.retryable === "boolean" &&
    (candidate.request_id === undefined ||
      (typeof candidate.request_id === "string" &&
        candidate.request_id.length > 0 &&
        candidate.request_id.length <= 128 &&
        !containsControlCharacters(candidate.request_id))) &&
    (candidate.http_status === undefined ||
      (Number.isInteger(candidate.http_status) &&
        candidate.http_status >= 100 &&
        candidate.http_status <= 599))
  );
}

function parsedCommandError(value: string): CommandErrorPayload | null {
  try {
    const parsed: unknown = JSON.parse(value);
    return isCommandErrorPayload(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function normalizeApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  if (isCommandErrorPayload(error)) return new ApiError(error);
  if (typeof error === "string") {
    const parsed = parsedCommandError(error);
    if (parsed) return new ApiError(parsed);
    return new ApiError({ code: "unknown", message: error.trim() || "操作失败", retryable: false });
  }
  if (error instanceof Error) {
    return new ApiError({
      code: "unknown",
      message: error.message.trim() || "操作失败",
      retryable: false,
    });
  }
  return new ApiError({ code: "unknown", message: "操作失败", retryable: false });
}

export function commandErrorCode(error: unknown): CommandErrorCode | null {
  if (error instanceof ApiError) return error.code;
  if (typeof error !== "object" || error === null || !("code" in error)) return null;
  const code = (error as { code?: unknown }).code;
  return typeof code === "string" && COMMAND_ERROR_CODES.has(code as CommandErrorCode)
    ? (code as CommandErrorCode)
    : null;
}
