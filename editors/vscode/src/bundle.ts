/**
 * Saved-lsp.json bundle parser.
 *
 * The extension consumes target/unsafe-review/lsp.json directly. It never
 * invokes a subprocess, never starts a live LSP server, and never invents
 * analyzer truth beyond what is in this file.
 */

import * as path from "node:path";

export interface BundleStatus {
  message: string;
  trustBoundary: string;
  state?: string;
  cards?: number;
  openActionableGaps?: number;
  highPriorityCards?: number;
}

export interface BundleRangePosition {
  line: number;
  character: number;
}

export interface BundleRange {
  start: BundleRangePosition;
  end: BundleRangePosition;
}

export interface BundleDiagnostic {
  cardId: string;
  code: string;
  message: string;
  path: string;
  range: BundleRange;
  severity?: number;
  source?: string;
  trustBoundary?: string;
  nextAction?: string;
  missingEvidence?: string[];
  witnessRoutes?: string[];
  verifyCommands?: string[];
  operation?: string;
  operationFamily?: string;
}

export interface BundleHover {
  cardId: string;
  path: string;
  position: BundleRangePosition;
  contents: string;
  trustBoundary?: string;
}

export interface BundleCodeActionPayload {
  cardId?: string;
  kind?: string;
  command?: string;
  file?: string;
  line?: number;
  name?: string;
  trustBoundary?: string;
}

export interface BundleCodeAction {
  title: string;
  command: string;
  path: string;
  range?: BundleRange;
  payload?: BundleCodeActionPayload;
}

export interface ParsedBundle {
  status: BundleStatus;
  diagnostics: BundleDiagnostic[];
  hovers: BundleHover[];
  codeActions: BundleCodeAction[];
  trustBoundary: string;
  warnings: string[];
}

const DEFAULT_TRUST_BOUNDARY =
  "Static unsafe contract review only; this is not a proof of memory safety, " +
  "not UB-free status, and not a Miri result unless a witness receipt is attached.";

export class BundleParseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BundleParseError";
  }
}

export function parseBundle(text: string): ParsedBundle {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (err) {
    throw new BundleParseError(`lsp.json is not valid JSON: ${(err as Error).message}`);
  }
  if (!isRecord(raw)) {
    throw new BundleParseError("lsp.json must be a JSON object");
  }

  const warnings: string[] = [];

  const trustBoundary = readString(raw["trust_boundary"]) ?? DEFAULT_TRUST_BOUNDARY;
  const status = parseStatus(raw["status"], trustBoundary);

  const diagnostics = parseDiagnostics(raw["diagnostics"], warnings);
  const hovers = parseHovers(raw["hovers"], warnings);
  const codeActions = parseCodeActions(raw["code_actions"], warnings);

  return {
    status,
    diagnostics,
    hovers,
    codeActions,
    trustBoundary,
    warnings,
  };
}

function parseStatus(value: unknown, fallbackTrustBoundary: string): BundleStatus {
  if (!isRecord(value)) {
    return {
      message: "unsafe-review: status missing",
      trustBoundary: fallbackTrustBoundary,
    };
  }
  return {
    message: readString(value["message"]) ?? "unsafe-review: no status message",
    trustBoundary: readString(value["trust_boundary"]) ?? fallbackTrustBoundary,
    state: readString(value["state"]),
    cards: readNumber(value["cards"]),
    openActionableGaps: readNumber(value["open_actionable_gaps"]),
    highPriorityCards: readNumber(value["high_priority_cards"]),
  };
}

function parseDiagnostics(value: unknown, warnings: string[]): BundleDiagnostic[] {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    warnings.push("lsp.json `diagnostics` is not an array; ignored");
    return [];
  }
  const out: BundleDiagnostic[] = [];
  for (let i = 0; i < value.length; i++) {
    const entry = value[i];
    if (!isRecord(entry)) {
      warnings.push(`diagnostic #${i} is not an object; skipped`);
      continue;
    }
    const range = parseRange(entry["range"]);
    if (range === undefined) {
      warnings.push(`diagnostic #${i} has no renderable range; skipped`);
      continue;
    }
    const path = readString(entry["path"]);
    if (path === undefined || path.length === 0) {
      warnings.push(`diagnostic #${i} has no path; skipped`);
      continue;
    }
    const cardId = readString(entry["card_id"]);
    if (cardId === undefined || cardId.length === 0) {
      warnings.push(`diagnostic #${i} has no card_id; skipped`);
      continue;
    }
    const message = readString(entry["message"]);
    if (message === undefined || message.length === 0) {
      warnings.push(`diagnostic #${i} has no message; skipped`);
      continue;
    }
    out.push({
      cardId,
      code: readString(entry["code"]) ?? "",
      message,
      path,
      range,
      severity: readNumber(entry["severity"]),
      source: readString(entry["source"]) ?? "unsafe-review",
      trustBoundary: readString(entry["trust_boundary"]),
      nextAction: readString(entry["next_action"]),
      missingEvidence: readStringArray(entry["missing_evidence"]),
      witnessRoutes: readStringArray(entry["witness_routes"]),
      verifyCommands: readStringArray(entry["verify_commands"]),
      operation: readString(entry["operation"]),
      operationFamily: readString(entry["operation_family"]),
    });
  }
  return out;
}

function parseHovers(value: unknown, warnings: string[]): BundleHover[] {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    warnings.push("lsp.json `hovers` is not an array; ignored");
    return [];
  }
  const out: BundleHover[] = [];
  for (let i = 0; i < value.length; i++) {
    const entry = value[i];
    if (!isRecord(entry)) {
      warnings.push(`hover #${i} is not an object; skipped`);
      continue;
    }
    const position = parsePosition(entry["position"]);
    if (position === undefined) {
      warnings.push(`hover #${i} has no position; skipped`);
      continue;
    }
    const path = readString(entry["path"]);
    const cardId = readString(entry["card_id"]);
    const contents = readString(entry["contents"]);
    if (path === undefined || cardId === undefined || contents === undefined) {
      warnings.push(`hover #${i} is missing path/card_id/contents; skipped`);
      continue;
    }
    out.push({
      cardId,
      path,
      position,
      contents,
      trustBoundary: readString(entry["trust_boundary"]),
    });
  }
  return out;
}

function parseCodeActions(value: unknown, warnings: string[]): BundleCodeAction[] {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    warnings.push("lsp.json `code_actions` is not an array; ignored");
    return [];
  }
  const out: BundleCodeAction[] = [];
  for (let i = 0; i < value.length; i++) {
    const entry = value[i];
    if (!isRecord(entry)) {
      warnings.push(`code_action #${i} is not an object; skipped`);
      continue;
    }
    const title = readString(entry["title"]);
    const command = readString(entry["command"]);
    const path = readString(entry["path"]);
    if (title === undefined || command === undefined || path === undefined) {
      warnings.push(`code_action #${i} is missing title/command/path; skipped`);
      continue;
    }
    out.push({
      title,
      command,
      path,
      range: parseRange(entry["range"]),
      payload: parseCodeActionPayload(entry["payload"]),
    });
  }
  return out;
}

function parseCodeActionPayload(value: unknown): BundleCodeActionPayload | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  return {
    cardId: readString(value["card_id"]),
    kind: readString(value["kind"]),
    command: readString(value["command"]),
    file: readString(value["file"]),
    line: readNumber(value["line"]),
    name: readString(value["name"]),
    trustBoundary: readString(value["trust_boundary"]),
  };
}

function parseRange(value: unknown): BundleRange | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  const start = parsePosition(value["start"]);
  const end = parsePosition(value["end"]);
  if (start === undefined || end === undefined) {
    return undefined;
  }
  // The saved projection uses LSP-style zero-based positions; line 0 is valid.
  // Reject only missing, non-finite, or negative positions.
  return { start, end };
}

function parsePosition(value: unknown): BundleRangePosition | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  const line = readNumber(value["line"]);
  const character = readNumber(value["character"]);
  if (line === undefined || character === undefined) {
    return undefined;
  }
  if (!Number.isFinite(line) || !Number.isFinite(character)) {
    return undefined;
  }
  if (line < 0 || character < 0) {
    return undefined;
  }
  return { line, character };
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function readNumber(value: unknown): number | undefined {
  return typeof value === "number" ? value : undefined;
}

function readStringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }
  const out: string[] = [];
  for (const entry of value) {
    if (typeof entry === "string") {
      out.push(entry);
    }
  }
  return out;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function diagnosticsByFile(
  diagnostics: BundleDiagnostic[],
): Map<string, BundleDiagnostic[]> {
  const map = new Map<string, BundleDiagnostic[]>();
  for (const diag of diagnostics) {
    const list = map.get(diag.path);
    if (list === undefined) {
      map.set(diag.path, [diag]);
    } else {
      list.push(diag);
    }
  }
  return map;
}

export function capDiagnosticsPerFile(
  diagnostics: BundleDiagnostic[],
  max: number,
): BundleDiagnostic[] {
  if (max <= 0 || diagnostics.length <= max) {
    return diagnostics;
  }
  const counts = new Map<string, number>();
  const out: BundleDiagnostic[] = [];
  for (const diag of diagnostics) {
    const current = counts.get(diag.path) ?? 0;
    if (current >= max) {
      continue;
    }
    counts.set(diag.path, current + 1);
    out.push(diag);
  }
  return out;
}

export function resolveWorkspaceFilePath(
  workspaceRoot: string,
  workspaceRelativePath: string,
): string | undefined {
  const root = path.resolve(workspaceRoot);
  const target = path.resolve(root, workspaceRelativePath);
  const rootWithSeparator = root.endsWith(path.sep) ? root : `${root}${path.sep}`;
  if (target !== root && !target.startsWith(rootWithSeparator)) {
    return undefined;
  }
  return target;
}
