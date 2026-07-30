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

export interface BundleAnalysisIdentity {
  analysis_id: string;
  generation: number;
  tool_version: string;
  scope: string;
  base_commit?: string;
  head_commit?: string;
  document_version?: number;
  file_digest?: string;
  state: string;
}

export interface BundleCoverage {
  baselineState?: string;
  movement?: string;
  readiness?: string;
  commentStatus?: string;
  contractCoverage?: string;
  guardCoverage?: string;
  manualContext?: string;
  testReachCoverage?: string;
  witnessReceiptCoverage?: string;
}

export type BundleStructuredObject = Record<string, unknown>;

export interface BundleWitnessRoute {
  kind?: string;
  reason?: string;
  command?: string;
  required?: boolean;
}

export interface BundleDiagnostic {
  cardId: string;
  code: string;
  message: string;
  path: string;
  range: BundleRange;
  evidenceSummary?: BundleStructuredObject;
  hazards?: string[];
  obligationEvidence?: BundleStructuredObject[];
  proofPath?: string;
  requiredSafetyConditions?: BundleStructuredObject[];
  severity?: number;
  source?: string;
  trustBoundary?: string;
  coverage?: BundleCoverage;
  nextAction?: string;
  missingEvidence?: string[];
  witnessRoutes?: BundleWitnessRoute[];
  verifyCommands?: string[];
  operation?: string;
  operationFamily?: string;
}

export interface BundleHover {
  analysis?: BundleAnalysisIdentity;
  cardId: string;
  path: string;
  position: BundleRangePosition;
  range?: BundleRange;
  contents: string;
  trustBoundary?: string;
}

export interface BundleCodeActionPayload {
  cardId?: string;
  actionId?: string;
  analysis?: BundleStructuredObject;
  agentPacket?: string;
  repairCandidates?: BundleStructuredObject[];
  readiness?: string;
  kind?: string;
  command?: string;
  file?: string;
  line?: number;
  name?: string;
  proofPath?: string;
  trustBoundary?: string;
}

export interface BundleCodeAction {
  actionId?: string;
  title: string;
  command?: string;
  commandArguments?: BundleStructuredObject;
  path: string;
  range?: BundleRange;
  kind?: string;
  isPreferred?: boolean;
  commandOnly?: boolean;
  disabled?: { reasonCode: string; reason: string };
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

export interface DiagnosticCapSummary {
  path: string;
  total: number;
  visible: number;
  hidden: number;
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
  const schemaVersion = readString(raw["schema_version"]);
  if (schemaVersion !== "0.1" && schemaVersion !== "0.2") {
    throw new BundleParseError(`unsupported lsp.json schema_version: ${schemaVersion ?? "missing"}`);
  }

  const trustBoundary = readString(raw["trust_boundary"]) ?? DEFAULT_TRUST_BOUNDARY;
  const status = parseStatus(raw["status"], trustBoundary);

  const diagnostics = parseDiagnostics(raw["diagnostics"], warnings);
  const hovers = parseHovers(raw["hovers"], warnings, schemaVersion, raw["analysis"], diagnostics);
  const codeActions = parseCodeActions(
    raw["code_actions"], warnings, schemaVersion, raw["analysis"], diagnostics, trustBoundary,
  );

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
    const code = readString(entry["code"]);
    if (code === undefined || code.length === 0) {
      warnings.push(`diagnostic #${i} has no canonical rule code; skipped`);
      continue;
    }
    out.push({
      cardId,
      code,
      message,
      path,
      range,
      evidenceSummary: readObject(entry["evidence_summary"]),
      hazards: readStringArray(entry["hazards"]),
      obligationEvidence: readObjectArray(entry["obligation_evidence"]),
      proofPath: readString(entry["proof_path"]),
      requiredSafetyConditions: readObjectArray(entry["required_safety_conditions"]),
      severity: readNumber(entry["severity"]),
      source: readString(entry["source"]) ?? "unsafe-review",
      trustBoundary: readString(entry["trust_boundary"]),
      coverage: parseCoverage(entry["coverage"]),
      nextAction: readString(entry["next_action"]),
      missingEvidence: readStringArray(entry["missing_evidence"]),
      witnessRoutes: parseWitnessRoutes(entry["witness_routes"]),
      verifyCommands: readStringArray(entry["verify_commands"]),
      operation: readString(entry["operation"]),
      operationFamily: readString(entry["operation_family"]),
    });
  }
  return out;
}

function parseCoverage(value: unknown): BundleCoverage | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  return {
    baselineState: readString(value["baseline_state"]),
    movement: readString(value["outcome_movement"]),
    readiness: readString(value["agent_lsp_readiness"]),
    commentStatus: readString(value["comment_plan_status"]),
    contractCoverage: readString(value["contract_coverage"]),
    guardCoverage: readString(value["guard_coverage"]),
    manualContext: readString(value["manual_context"]),
    testReachCoverage: readString(value["test_reach_coverage"]),
    witnessReceiptCoverage: readString(value["witness_receipt_coverage"]),
  };
}

function parseWitnessRoutes(value: unknown): BundleWitnessRoute[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }
  const out: BundleWitnessRoute[] = [];
  for (const entry of value) {
    if (typeof entry === "string") {
      out.push({ kind: entry });
      continue;
    }
    if (!isRecord(entry)) {
      continue;
    }
    out.push({
      kind: readString(entry["kind"]),
      reason: readString(entry["reason"]),
      command: readString(entry["command"]),
      required: typeof entry["required"] === "boolean" ? entry["required"] : undefined,
    });
  }
  return out;
}

function parseHovers(
  value: unknown,
  warnings: string[],
  schemaVersion: string,
  bundleAnalysis: unknown,
  diagnostics: BundleDiagnostic[],
): BundleHover[] {
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
    const range = parseRange(entry["range"]);
    if (position === undefined && range === undefined) {
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
    if (schemaVersion === "0.2") {
      const analysis = isAnalysisIdentity(entry["analysis"]) ? entry["analysis"] : undefined;
      const matchingDiagnostic = diagnostics.find((item) =>
        item.cardId === cardId && item.path === path && range !== undefined &&
        rangesEqual(item.range, range),
      );
      if (
        range === undefined || analysis === undefined || !isAnalysisIdentity(bundleAnalysis) ||
        JSON.stringify(analysis) !== JSON.stringify(bundleAnalysis) || matchingDiagnostic === undefined
      ) {
        warnings.push(`hover #${i} has inconsistent canonical identity; skipped`);
        continue;
      }
      out.push({
        analysis,
        cardId,
        path,
        position: position ?? range.start,
        range,
        contents,
        trustBoundary: readString(entry["trust_boundary"]),
      });
      continue;
    }
    out.push({
      cardId,
      path,
      position: position ?? range!.start,
      range,
      contents,
      trustBoundary: readString(entry["trust_boundary"]),
    });
  }
  return out;
}

function parseCodeActions(
  value: unknown,
  warnings: string[],
  schemaVersion: string,
  bundleAnalysis: unknown,
  diagnostics: BundleDiagnostic[],
  bundleTrustBoundary: string,
): BundleCodeAction[] {
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
    const actionId = readString(entry["action_id"]);
    if (schemaVersion === "0.2") {
      if (actionId === undefined) {
        warnings.push(`code_action #${i} uses legacy shape in 0.2 bundle; skipped`);
        continue;
      }
      const canonical = parseCanonicalCodeAction(
        entry, i, warnings, bundleAnalysis, diagnostics, bundleTrustBoundary,
      );
      if (canonical !== undefined) {
        out.push(canonical);
      }
      continue;
    }
    if (actionId !== undefined) {
      warnings.push(`code_action #${i} uses canonical shape in 0.1 bundle; skipped`);
      continue;
    }
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
    warnings.push(`code_action #${i} uses legacy 0.1 action shape`);
  }
  return out;
}

function parseCanonicalCodeAction(
  entry: Record<string, unknown>,
  index: number,
  warnings: string[],
  bundleAnalysis: unknown,
  diagnostics: BundleDiagnostic[],
  bundleTrustBoundary: string,
): BundleCodeAction | undefined {
  const actionId = readString(entry["action_id"]);
  const title = readString(entry["title"]);
  const kind = readString(entry["kind"]);
  const diagnostic = isRecord(entry["diagnostic"]) ? entry["diagnostic"] : undefined;
  const payload = isRecord(entry["payload"]) ? entry["payload"] : undefined;
  const command = isRecord(entry["command"]) ? entry["command"] : undefined;
  const applicability = isRecord(entry["applicability"]) ? entry["applicability"] : undefined;
  const cardId = diagnostic === undefined ? undefined : readString(diagnostic["card_id"]);
  const path = diagnostic === undefined ? undefined : readString(diagnostic["path"]);
  const range = diagnostic === undefined ? undefined : parseRange(diagnostic["range"]);
  const payloadCardId = payload === undefined ? undefined : readString(payload["card_id"]);
  const payloadActionId = payload === undefined ? undefined : readString(payload["action_id"]);
  const commandOnly = entry["command_only"] === true;
  const payloadAnalysis = payload === undefined ? undefined : payload["analysis"];
  const agentPacket = payload === undefined ? undefined : readString(payload["agent_packet"]);
  const repairCandidates = payload === undefined ? undefined : readObjectArray(payload["repair_candidates"]);
  const argumentsValue = command === undefined || !isRecord(command["arguments"])
    ? undefined : command["arguments"];
  const argumentAnalysis = argumentsValue?.["analysis"];
  const argumentCardId = argumentsValue === undefined ? undefined : readString(argumentsValue["card_id"]);
  const readiness = payload === undefined ? undefined : readString(payload["agent_readiness"]);
  const commandId = command === undefined ? undefined : readString(command["command"]);
  const actionTrustBoundary = readString(entry["trust_boundary"]);
  const vocabulary = canonicalActionVocabulary(actionId, readiness);
  const matchingDiagnostic = diagnostics.find((item) =>
    item.cardId === cardId && item.path === path && range !== undefined && rangesEqual(item.range, range),
  );
  if (
    actionId === undefined || title === undefined || kind === undefined || cardId === undefined ||
    path === undefined || range === undefined || payload === undefined || command === undefined ||
    payloadCardId !== cardId || payloadActionId !== actionId || argumentCardId !== cardId || !commandOnly ||
    vocabulary === undefined || vocabulary.command !== commandId || vocabulary.kind !== kind ||
    entry["is_preferred"] !== false || matchingDiagnostic === undefined ||
    actionTrustBoundary === undefined || actionTrustBoundary !== bundleTrustBoundary ||
    !isAnalysisIdentity(bundleAnalysis) || !isAnalysisIdentity(payloadAnalysis) ||
    !isAnalysisIdentity(argumentAnalysis) ||
    JSON.stringify(payloadAnalysis) !== JSON.stringify(bundleAnalysis) ||
    JSON.stringify(argumentAnalysis) !== JSON.stringify(bundleAnalysis) ||
    (actionId === "agent-packet" && !isMatchingAgentPacket(agentPacket, cardId, bundleAnalysis))
  ) {
    warnings.push(`code_action #${index} has inconsistent canonical identity; skipped`);
    return undefined;
  }
  const state = applicability === undefined ? undefined : readString(applicability["state"]);
  if (state !== "available" && state !== "disabled") {
    warnings.push(`code_action #${index} has invalid applicability; skipped`);
    return undefined;
  }
  const reasonCode = readString(applicability?.["reason_code"]);
  const reason = readString(applicability?.["reason"]);
  if (state === "disabled" && (reasonCode === undefined || reason === undefined)) {
    warnings.push(`code_action #${index} has incomplete disabled applicability; skipped`);
    return undefined;
  }
  const expectedDisabledReason = canonicalDisabledReason(actionId);
  if (state === "disabled" && (expectedDisabledReason === undefined ||
    reasonCode !== expectedDisabledReason.reasonCode || reason !== expectedDisabledReason.reason)) {
    warnings.push(`code_action #${index} has non-canonical disabled applicability; skipped`);
    return undefined;
  }
  const disabled = state === "disabled" ? { reasonCode: reasonCode!, reason: reason! } : undefined;
  if (state === "available" && actionId === "witness-route" && !matchingDiagnostic.witnessRoutes?.length) {
    warnings.push(`code_action #${index} has no matching witness route; skipped`);
    return undefined;
  }
  if (state === "disabled" && actionId === "witness-route" && matchingDiagnostic.witnessRoutes?.length) {
    warnings.push(`code_action #${index} disables an available witness route; skipped`);
    return undefined;
  }
  const matchingWitnessCommand = matchingDiagnostic.witnessRoutes
    ?.map((route) => route.command)
    .find((candidate) => candidate !== undefined && candidate.length > 0);
  if (state === "disabled" && actionId === "witness-command" && matchingWitnessCommand !== undefined) {
    warnings.push(`code_action #${index} disables an available witness command; skipped`);
    return undefined;
  }
  if (state === "available" && actionId === "related-test") {
    const file = argumentsValue === undefined ? undefined : readString(argumentsValue["file"]);
    const name = argumentsValue === undefined ? undefined : readString(argumentsValue["name"]);
    const line = argumentsValue === undefined ? undefined : readNumber(argumentsValue["line"]);
    if (file === undefined || file.length === 0 || name === undefined || name.length === 0 ||
      line === undefined || !Number.isInteger(line) || line <= 0) {
      warnings.push(`code_action #${index} has incomplete related-test capability; skipped`);
      return undefined;
    }
  }
  const commandArguments = { ...argumentsValue };
  if (actionId === "witness-command") {
    const witnessCommand = matchingWitnessCommand;
    if (state === "available" && witnessCommand === undefined) {
      warnings.push(`code_action #${index} has no matching witness command; skipped`);
      return undefined;
    }
    if (witnessCommand !== undefined) {
      commandArguments["command"] = witnessCommand;
    }
  }
  return {
    actionId,
    title,
    kind,
    command: readString(command["command"]),
    commandArguments,
    path,
    range,
    isPreferred: false,
    commandOnly,
    disabled,
    payload: {
      actionId: payloadActionId,
      cardId: payloadCardId,
      analysis: isRecord(payload["analysis"]) ? payload["analysis"] : undefined,
      agentPacket,
      repairCandidates,
      readiness,
      trustBoundary: actionTrustBoundary,
      file: isRecord(command["arguments"]) ? readString(command["arguments"]["file"]) : undefined,
      line: isRecord(command["arguments"]) ? readNumber(command["arguments"]["line"]) : undefined,
      name: isRecord(command["arguments"]) ? readString(command["arguments"]["name"]) : undefined,
    },
  };
}

const MAX_AGENT_PACKET_CHARS = 256 * 1024;

function isMatchingAgentPacket(
  packetText: string | undefined,
  cardId: string,
  bundleAnalysis: unknown,
): boolean {
  if (packetText === undefined || packetText.length === 0 || packetText.length > MAX_AGENT_PACKET_CHARS) {
    return false;
  }
  let packet: unknown;
  try {
    packet = JSON.parse(packetText);
  } catch {
    return false;
  }
  if (!isRecord(packet) || !isAnalysisIdentity(bundleAnalysis)) {
    return false;
  }
  return readString(packet["schema_version"]) === "0.1" &&
    readString(packet["mode"]) === "bounded_repair_packet" &&
    readString(packet["card_id"]) === cardId &&
    isAnalysisIdentity(packet["analysis"]) &&
    JSON.stringify(packet["analysis"]) === JSON.stringify(bundleAnalysis);
}

function canonicalDisabledReason(
  actionId: string | undefined,
): { reasonCode: string; reason: string } | undefined {
  switch (actionId) {
    case "witness-route":
      return { reasonCode: "no_witness_route", reason: "No witness route is available for this card." };
    case "witness-command":
      return { reasonCode: "no_witness_command", reason: "No witness command is available for this card." };
    case "related-test":
      return { reasonCode: "no_related_test", reason: "No structured related test is available for this card." };
    default:
      return undefined;
  }
}

function canonicalActionVocabulary(
  actionId: string | undefined,
  readiness: string | undefined,
): { command: string; kind: string } | undefined {
  if (!["ready_for_agent", "requires_human_review", "requires_witness_receipt", "unsupported"].includes(readiness ?? "")) {
    return undefined;
  }
  switch (actionId) {
    case "agent-packet":
      return {
        command: "unsafe-review.collectAgentPacket",
        kind: readiness === "ready_for_agent"
          ? "quickfix.unsafeReview.agentPacket"
          : "source.unsafeReview.reviewContext",
      };
    case "witness-route":
      return { command: "unsafe-review.explainWitnessRoute", kind: "source.unsafeReview.witnessRoute" };
    case "witness-command":
      return { command: "unsafe-review.collectWitnessCommand", kind: "source.unsafeReview.witnessCommand" };
    case "related-test":
      return { command: "unsafe-review.openRelatedTest", kind: "source.unsafeReview.relatedTest" };
    default:
      return undefined;
  }
}

export function rangesEqual(left: BundleRange, right: BundleRange): boolean {
  return left.start.line === right.start.line && left.start.character === right.start.character &&
    left.end.line === right.end.line && left.end.character === right.end.character;
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
    proofPath: readString(value["proof_path"]),
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

function readObject(value: unknown): BundleStructuredObject | undefined {
  return isRecord(value) ? value : undefined;
}

function readObjectArray(value: unknown): BundleStructuredObject[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }
  return value.filter(isRecord);
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

function isAnalysisIdentity(value: unknown): value is BundleAnalysisIdentity {
  if (!isRecord(value)) {
    return false;
  }
  const record = value as Record<string, unknown>;
  const generation = record["generation"];
  const documentVersion = record["document_version"];
  const state = readString(record["state"]);
  return ["analysis_id", "tool_version", "scope"].every((field) => {
    const fieldValue = readString(record[field]);
    return fieldValue !== undefined && fieldValue.trim().length > 0;
  }) && typeof generation === "number" && Number.isSafeInteger(generation) && generation >= 0 &&
    state !== undefined && ["current", "refreshing", "stale", "partial", "capped", "failed"].includes(state) &&
    ["base_commit", "head_commit", "file_digest"].every((field) =>
      record[field] === undefined || typeof record[field] === "string"
    ) && (documentVersion === undefined ||
      (typeof documentVersion === "number" && Number.isSafeInteger(documentVersion) && documentVersion >= 0));
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

export function diagnosticCapSummaries(
  diagnostics: BundleDiagnostic[],
  max: number,
): DiagnosticCapSummary[] {
  if (max <= 0) {
    return [];
  }
  const totals = new Map<string, number>();
  for (const diagnostic of diagnostics) {
    totals.set(diagnostic.path, (totals.get(diagnostic.path) ?? 0) + 1);
  }
  return [...totals].map(([path, total]) => ({
    path,
    total,
    visible: Math.min(total, max),
    hidden: Math.max(total - max, 0),
  }));
}

export function positionInRange(position: BundleRangePosition, range: BundleRange): boolean {
  return comparePositions(range.start, position) <= 0 && comparePositions(position, range.end) <= 0;
}

export function rangesIntersect(left: BundleRange, right: BundleRange): boolean {
  return comparePositions(left.start, right.end) <= 0 && comparePositions(right.start, left.end) <= 0;
}

function comparePositions(left: BundleRangePosition, right: BundleRangePosition): number {
  if (left.line !== right.line) {
    return left.line - right.line;
  }
  return left.character - right.character;
}

export function supportedDiagnosticSeverity(value: number | undefined): number | undefined {
  return value === 2 || value === 3 || value === 4 ? value : undefined;
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
