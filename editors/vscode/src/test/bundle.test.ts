import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import * as path from "node:path";
import test from "node:test";

import {
  BundleParseError,
  capDiagnosticsPerFile,
  diagnosticsByFile,
  diagnosticCapSummaries,
  parseBundle,
  positionInRange,
  rangesEqual,
  rangesIntersect,
  resolveWorkspaceFilePath,
  supportedDiagnosticSeverity,
} from "../bundle";

const MINIMAL_BUNDLE = {
  schema_version: "0.1",
  tool: "unsafe-review",
  status: {
    message: "1 unsafe-review card(s)",
    trust_boundary: "Static unsafe contract review only; not memory-safety proof.",
  },
  trust_boundary: "Static unsafe contract review only; not memory-safety proof.",
  diagnostics: [
    {
      card_id: "UR-foo",
      code: "guard_missing",
      message: "raw_pointer_read: add an alignment guard",
      path: "src/lib.rs",
      range: {
        start: { line: 7, character: 4 },
        end: { line: 7, character: 42 },
      },
      severity: 3,
      source: "unsafe-review",
      coverage: {
        baseline_state: "new",
        outcome_movement: "regressed",
        agent_lsp_readiness: "ready",
        comment_plan_status: "selected",
      },
      missing_evidence: ["alignment evidence"],
      next_action: "Add the alignment guard.",
      witness_routes: [
        {
          kind: "miri",
          reason: "run a focused witness",
          command: "cargo +nightly miri test read_header",
          required: true,
        },
      ],
      verify_commands: ["cargo +nightly miri test read_header"],
      operation: "unsafe { ptr.cast::<Header>().read() }",
      operation_family: "raw_pointer_read",
    },
  ],
  hovers: [
    {
      card_id: "UR-foo",
      path: "src/lib.rs",
      position: { line: 7, character: 4 },
      contents: "unsafe-review `guard_missing` for `raw_pointer_read`",
      trust_boundary: "Static unsafe contract review only.",
    },
  ],
  code_actions: [
    {
      title: "Copy unsafe-review packet for UR-foo",
      command: "unsafe-review.copyAgentPacket",
      path: "src/lib.rs",
      range: {
        start: { line: 7, character: 4 },
        end: { line: 7, character: 42 },
      },
      payload: {
        card_id: "UR-foo",
        kind: "unsafe-review.agent_packet",
        proof_path: "source_route_only",
        trust_boundary: "Static unsafe contract review only.",
      },
      arguments: [],
    },
  ],
};

function agentPacket(analysis: object, cardId = "UR-foo"): string {
  return JSON.stringify({
    schema_version: "0.1",
    mode: "bounded_repair_packet",
    card_id: cardId,
    analysis,
  });
}

test("parseBundle returns diagnostics, hovers, and code actions", () => {
  const result = parseBundle(JSON.stringify(MINIMAL_BUNDLE));
  assert.equal(result.diagnostics.length, 1);
  assert.equal(result.diagnostics[0].cardId, "UR-foo");
  assert.equal(result.diagnostics[0].code, "guard_missing");
  assert.equal(result.diagnostics[0].severity, 3);
  assert.deepEqual(result.diagnostics[0].range, MINIMAL_BUNDLE.diagnostics[0].range);
  assert.deepEqual(result.diagnostics[0].missingEvidence, ["alignment evidence"]);
  assert.deepEqual(result.diagnostics[0].witnessRoutes, [
    {
      kind: "miri",
      reason: "run a focused witness",
      command: "cargo +nightly miri test read_header",
      required: true,
    },
  ]);
  assert.deepEqual(result.diagnostics[0].coverage, {
    baselineState: "new",
    movement: "regressed",
    readiness: "ready",
    commentStatus: "selected",
    contractCoverage: undefined,
    guardCoverage: undefined,
    manualContext: undefined,
    testReachCoverage: undefined,
    witnessReceiptCoverage: undefined,
  });
  assert.equal(result.diagnostics[0].operation, "unsafe { ptr.cast::<Header>().read() }");
  assert.equal(result.hovers.length, 1);
  assert.equal(result.hovers[0].contents.includes("guard_missing"), true);
  assert.equal(result.codeActions.length, 1);
  assert.equal(result.codeActions[0].command, "unsafe-review.copyAgentPacket");
  assert.equal(result.codeActions[0].payload?.cardId, "UR-foo");
  assert.equal(result.codeActions[0].payload?.proofPath, "source_route_only");
  assert.deepEqual(result.warnings, ["code_action #0 uses legacy 0.1 action shape"]);
});

test("parseBundle preserves canonical 0.2 action semantics", () => {
  const analysis = { analysis_id: "analysis-1", generation: 1, tool_version: "0.3.8", scope: "diff", state: "current" };
  const canonical = {
    ...MINIMAL_BUNDLE,
    schema_version: "0.2",
    analysis,
    hovers: [{
      ...MINIMAL_BUNDLE.hovers[0],
      analysis,
      range: MINIMAL_BUNDLE.diagnostics[0].range,
    }],
    code_actions: [{
      action_id: "agent-packet",
      title: "Copy bounded unsafe-review agent packet",
      kind: "quickfix.unsafeReview.agentPacket",
      diagnostic: { card_id: "UR-foo", path: "src/lib.rs", range: MINIMAL_BUNDLE.diagnostics[0].range },
      payload: {
        action_id: "agent-packet",
        card_id: "UR-foo",
        analysis,
        agent_readiness: "ready_for_agent",
        agent_packet: agentPacket(analysis),
      },
      command: { command: "unsafe-review.collectAgentPacket", arguments: { card_id: "UR-foo", analysis } },
      applicability: { state: "available" },
      is_preferred: false,
      command_only: true,
      trust_boundary: MINIMAL_BUNDLE.trust_boundary,
    }],
  };
  const result = parseBundle(JSON.stringify(canonical));
  assert.equal(result.warnings.length, 0);
  assert.equal(result.codeActions[0].actionId, "agent-packet");
  assert.equal(result.codeActions[0].kind, "quickfix.unsafeReview.agentPacket");
  assert.equal(result.codeActions[0].command, "unsafe-review.collectAgentPacket");
  assert.equal(result.codeActions[0].payload?.readiness, "ready_for_agent");
  assert.equal(JSON.parse(result.codeActions[0].payload?.agentPacket ?? "{}").card_id, "UR-foo");
  assert.deepEqual(result.codeActions[0].commandArguments?.["analysis"], analysis);
  assert.deepEqual(result.hovers[0].analysis, analysis);
  assert.deepEqual(result.hovers[0].range, MINIMAL_BUNDLE.diagnostics[0].range);
});

test("canonical agent packets reject stale card or analysis identity", () => {
  const analysis = { analysis_id: "analysis-1", generation: 1, tool_version: "0.3.8", scope: "diff", state: "current" };
  const action = {
    action_id: "agent-packet",
    title: "Copy bounded unsafe-review agent packet",
    kind: "quickfix.unsafeReview.agentPacket",
    diagnostic: { card_id: "UR-foo", path: "src/lib.rs", range: MINIMAL_BUNDLE.diagnostics[0].range },
    payload: {
      action_id: "agent-packet",
      card_id: "UR-foo",
      analysis,
      agent_readiness: "ready_for_agent",
      agent_packet: agentPacket(analysis),
    },
    command: { command: "unsafe-review.collectAgentPacket", arguments: { card_id: "UR-foo", analysis } },
    applicability: { state: "available" },
    is_preferred: false,
    command_only: true,
    trust_boundary: MINIMAL_BUNDLE.trust_boundary,
  };
  const staleCard = parseBundle(JSON.stringify({
    ...MINIMAL_BUNDLE,
    schema_version: "0.2",
    analysis,
    code_actions: [{ ...action, payload: { ...action.payload, agent_packet: agentPacket(analysis, "UR-other") } }],
  }));
  assert.equal(staleCard.codeActions.length, 0);

  const staleAnalysis = { ...analysis, generation: 2 };
  const staleAnalysisResult = parseBundle(JSON.stringify({
    ...MINIMAL_BUNDLE,
    schema_version: "0.2",
    analysis,
    code_actions: [{ ...action, payload: { ...action.payload, agent_packet: agentPacket(staleAnalysis) } }],
  }));
  assert.equal(staleAnalysisResult.codeActions.length, 0);
});

test("canonical hovers reject cross-card or cross-analysis identity drift", () => {
  const analysis = { analysis_id: "analysis-1", generation: 1, tool_version: "0.3.8", scope: "diff", state: "current" };
  const hover = {
    ...MINIMAL_BUNDLE.hovers[0],
    analysis,
    range: MINIMAL_BUNDLE.diagnostics[0].range,
  };
  const wrongCard = parseBundle(JSON.stringify({
    ...MINIMAL_BUNDLE,
    schema_version: "0.2",
    analysis,
    hovers: [{ ...hover, card_id: "UR-other" }],
  }));
  assert.equal(wrongCard.hovers.length, 0);
  assert.match(wrongCard.warnings[0], /inconsistent canonical identity/);

  const wrongAnalysis = parseBundle(JSON.stringify({
    ...MINIMAL_BUNDLE,
    schema_version: "0.2",
    analysis,
    hovers: [{ ...hover, analysis: { ...analysis, generation: 2 } }],
  }));
  assert.equal(wrongAnalysis.hovers.length, 0);
  assert.match(wrongAnalysis.warnings[0], /inconsistent canonical identity/);
});

test("parseBundle preserves the committed canonical saved diagnostic fields", async () => {
  const fixturePath = path.resolve(
    __dirname,
    "../../../../fixtures/raw_pointer_alignment/expected.lsp.json",
  );
  const result = parseBundle(await readFile(fixturePath, "utf8"));
  const diagnostic = result.diagnostics[0];
  assert.ok(diagnostic);
  assert.equal(diagnostic.code, "guard_missing");
  assert.equal(diagnostic.severity, 2);
  assert.equal(diagnostic.coverage?.baselineState, "new");
  assert.equal(diagnostic.coverage?.movement, "regressed");
  assert.equal(diagnostic.coverage?.readiness, "ready");
  assert.equal(diagnostic.coverage?.contractCoverage, "present");
  assert.equal(diagnostic.coverage?.guardCoverage, "missing");
  assert.equal(diagnostic.coverage?.manualContext, "absent");
  assert.equal(diagnostic.coverage?.testReachCoverage, "missing");
  assert.equal(diagnostic.coverage?.witnessReceiptCoverage, "missing");
  assert.equal(diagnostic.missingEvidence?.length, 2);
  assert.equal(diagnostic.hazards?.length, 4);
  assert.equal(diagnostic.obligationEvidence?.length, 5);
  assert.equal(diagnostic.proofPath, "source_route_only");
  assert.equal(diagnostic.requiredSafetyConditions?.length, 5);
  assert.equal(diagnostic.range.start.line, 7);
  assert.equal(diagnostic.range.start.character, 4);
  assert.equal(diagnostic.range.end.character, 42);
  assert.ok(diagnostic.cardId.startsWith("UR-raw-pointer-alignment-fixture"));
  const witnessAction = result.codeActions.find((action) => action.actionId === "witness-command");
  assert.ok(witnessAction);
  assert.equal(
    witnessAction.commandArguments?.["command"],
    diagnostic.witnessRoutes?.find((route) => route.command !== undefined)?.command,
  );
});

test("canonical available actions require matching capabilities and trust boundary", async () => {
  const fixturePath = path.resolve(
    __dirname,
    "../../../../fixtures/raw_pointer_alignment/expected.lsp.json",
  );
  const original = JSON.parse(await readFile(fixturePath, "utf8"));

  const noRoute = structuredClone(original);
  noRoute.diagnostics[0].witness_routes = [];
  const noRouteResult = parseBundle(JSON.stringify(noRoute));
  assert.equal(noRouteResult.codeActions.some((action) => action.actionId === "witness-route"), false);

  const incompleteTest = structuredClone(original);
  const related = incompleteTest.code_actions.find((action: { action_id: string }) => action.action_id === "related-test");
  delete related.command.arguments.file;
  const incompleteTestResult = parseBundle(JSON.stringify(incompleteTest));
  assert.equal(incompleteTestResult.codeActions.some((action) => action.actionId === "related-test"), false);

  const missingBoundary = structuredClone(original);
  delete missingBoundary.code_actions[0].trust_boundary;
  const missingBoundaryResult = parseBundle(JSON.stringify(missingBoundary));
  assert.equal(missingBoundaryResult.codeActions.some((action) => action.actionId === "agent-packet"), false);
});

test("parseBundle rejects non-JSON", () => {
  assert.throws(() => parseBundle("not json"), BundleParseError);
});

test("parseBundle rejects non-object root", () => {
  assert.throws(() => parseBundle("[]"), BundleParseError);
});

test("parseBundle uses default trust boundary when missing", () => {
  const result = parseBundle(JSON.stringify({ schema_version: "0.1", tool: "unsafe-review" }));
  assert.ok(result.trustBoundary.length > 0);
  assert.ok(result.trustBoundary.toLowerCase().includes("not a proof"));
});

test("canonical actions reject cross-card command arguments", () => {
  const analysis = { analysis_id: "analysis-1", generation: 1, tool_version: "0.3.8", scope: "diff", state: "current" };
  const action = {
    action_id: "agent-packet", title: "Copy bounded unsafe-review agent packet",
    kind: "quickfix.unsafeReview.agentPacket",
    diagnostic: { card_id: "UR-foo", path: "src/lib.rs", range: MINIMAL_BUNDLE.diagnostics[0].range },
    payload: { action_id: "agent-packet", card_id: "UR-foo", analysis, agent_readiness: "ready_for_agent" },
    command: { command: "unsafe-review.collectAgentPacket", arguments: { card_id: "UR-other", analysis } },
    applicability: { state: "available" }, is_preferred: false, command_only: true,
    trust_boundary: MINIMAL_BUNDLE.trust_boundary,
  };
  const result = parseBundle(JSON.stringify({ ...MINIMAL_BUNDLE, schema_version: "0.2", analysis, code_actions: [action] }));
  assert.equal(result.codeActions.length, 0);
  assert.match(result.warnings[0], /inconsistent canonical identity/);
});

test("canonical actions require complete analysis identities", () => {
  const action = {
    action_id: "agent-packet", title: "Copy bounded unsafe-review agent packet",
    kind: "quickfix.unsafeReview.agentPacket",
    diagnostic: { card_id: "UR-foo", path: "src/lib.rs", range: MINIMAL_BUNDLE.diagnostics[0].range },
    payload: { action_id: "agent-packet", card_id: "UR-foo", agent_readiness: "ready_for_agent" },
    command: { command: "unsafe-review.collectAgentPacket", arguments: { card_id: "UR-foo" } },
    applicability: { state: "available" }, is_preferred: false, command_only: true,
    trust_boundary: MINIMAL_BUNDLE.trust_boundary,
  };
  const result = parseBundle(JSON.stringify({ ...MINIMAL_BUNDLE, schema_version: "0.2", code_actions: [action] }));
  assert.equal(result.codeActions.length, 0);
  assert.match(result.warnings[0], /inconsistent canonical identity/);

  const emptyIdentity = {
    ...action,
    payload: { ...action.payload, analysis: {} },
    command: { ...action.command, arguments: { ...action.command.arguments, analysis: {} } },
  };
  const emptyResult = parseBundle(JSON.stringify({
    ...MINIMAL_BUNDLE,
    schema_version: "0.2",
    analysis: {},
    code_actions: [emptyIdentity],
  }));
  assert.equal(emptyResult.codeActions.length, 0);
  assert.match(emptyResult.warnings[0], /inconsistent canonical identity/);

  const malformedAnalysis = {
    analysis_id: "analysis-1", generation: 1, tool_version: "0.3.8",
    scope: "diff", state: "current", base_commit: 7, document_version: "oops",
  };
  const malformedIdentity = {
    ...action,
    payload: { ...action.payload, analysis: malformedAnalysis },
    command: {
      ...action.command,
      arguments: { ...action.command.arguments, analysis: malformedAnalysis },
    },
  };
  const malformedResult = parseBundle(JSON.stringify({
    ...MINIMAL_BUNDLE,
    schema_version: "0.2",
    analysis: malformedAnalysis,
    code_actions: [malformedIdentity],
  }));
  assert.equal(malformedResult.codeActions.length, 0);
  assert.match(malformedResult.warnings[0], /inconsistent canonical identity/);
});

test("canonical actions reject diagnostic and applicability drift", () => {
  const analysis = { analysis_id: "analysis-1", generation: 1, tool_version: "0.3.8", scope: "diff", state: "current" };
  const action = {
    action_id: "agent-packet", title: "Copy bounded unsafe-review agent packet",
    kind: "quickfix.unsafeReview.agentPacket",
    diagnostic: { card_id: "UR-foo", path: "src/other.rs", range: MINIMAL_BUNDLE.diagnostics[0].range },
    payload: { action_id: "agent-packet", card_id: "UR-foo", analysis, agent_readiness: "ready_for_agent" },
    command: { command: "unsafe-review.collectAgentPacket", arguments: { card_id: "UR-foo", analysis } },
    applicability: { state: "mystery" }, is_preferred: false, command_only: true,
    trust_boundary: MINIMAL_BUNDLE.trust_boundary,
  };
  const result = parseBundle(JSON.stringify({ ...MINIMAL_BUNDLE, schema_version: "0.2", analysis, code_actions: [action] }));
  assert.equal(result.codeActions.length, 0);
});

test("parseBundle skips diagnostics that lack a renderable range", () => {
  const broken = {
    ...MINIMAL_BUNDLE,
    diagnostics: [
      ...MINIMAL_BUNDLE.diagnostics,
      {
        card_id: "UR-no-range",
        code: "guard_missing",
        message: "missing range",
        path: "src/lib.rs",
      },
    ],
  };
  const result = parseBundle(JSON.stringify(broken));
  assert.equal(result.diagnostics.length, 1);
  assert.equal(result.warnings.some((w) => w.includes("range")), true);
});

test("parseBundle accepts zero-based LSP ranges", () => {
  const zeroBased = {
    ...MINIMAL_BUNDLE,
    diagnostics: [
      {
        ...MINIMAL_BUNDLE.diagnostics[0],
        range: { start: { line: 0, character: 0 }, end: { line: 0, character: 8 } },
      },
    ],
  };
  const result = parseBundle(JSON.stringify(zeroBased));
  assert.equal(result.diagnostics.length, 1);
  assert.equal(result.diagnostics[0].range.start.line, 0);
});

test("parseBundle skips diagnostics missing card_id, path, message, or code", () => {
  const broken = {
    ...MINIMAL_BUNDLE,
    diagnostics: [
      ...MINIMAL_BUNDLE.diagnostics,
      {
        code: "guard_missing",
        message: "no card id",
        path: "src/lib.rs",
        range: { start: { line: 1, character: 0 }, end: { line: 1, character: 5 } },
      },
      {
        card_id: "UR-no-message",
        code: "guard_missing",
        path: "src/lib.rs",
        range: { start: { line: 1, character: 0 }, end: { line: 1, character: 5 } },
      },
      {
        card_id: "UR-no-code",
        message: "no rule code",
        path: "src/lib.rs",
        range: { start: { line: 1, character: 0 }, end: { line: 1, character: 5 } },
      },
    ],
  };
  const result = parseBundle(JSON.stringify(broken));
  assert.equal(result.diagnostics.length, 1);
  assert.equal(result.warnings.some((warning) => warning.includes("rule code")), true);
});

test("diagnosticsByFile groups by path", () => {
  const result = parseBundle(
    JSON.stringify({
      ...MINIMAL_BUNDLE,
      diagnostics: [
        ...MINIMAL_BUNDLE.diagnostics,
        {
          ...MINIMAL_BUNDLE.diagnostics[0],
          card_id: "UR-other",
          path: "src/lib.rs",
          range: { start: { line: 8, character: 20 }, end: { line: 8, character: 30 } },
        },
      ],
    }),
  );
  const grouped = diagnosticsByFile(result.diagnostics);
  assert.equal(grouped.size, 1);
  assert.deepEqual(
    grouped.get("src/lib.rs")?.map((diagnostic) => diagnostic.cardId),
    ["UR-foo", "UR-other"],
  );
});

test("saved UTF-16 positions are preserved", () => {
  const result = parseBundle(
    JSON.stringify({
      ...MINIMAL_BUNDLE,
      diagnostics: [
        {
          ...MINIMAL_BUNDLE.diagnostics[0],
          range: { start: { line: 0, character: 2 }, end: { line: 0, character: 5 } },
        },
      ],
    }),
  );
  assert.equal(
    result.diagnostics[0].range.end.character - result.diagnostics[0].range.start.character,
    3,
  );
});

test("supported diagnostic severity rejects forbidden and unknown values", () => {
  assert.equal(supportedDiagnosticSeverity(2), 2);
  assert.equal(supportedDiagnosticSeverity(3), 3);
  assert.equal(supportedDiagnosticSeverity(4), 4);
  assert.equal(supportedDiagnosticSeverity(1), undefined);
  assert.equal(supportedDiagnosticSeverity(undefined), undefined);
});

test("capDiagnosticsPerFile caps per file and preserves order", () => {
  const diagnostics = [
    { ...sampleDiagnostic("a"), path: "src/lib.rs" },
    { ...sampleDiagnostic("b"), path: "src/lib.rs" },
    { ...sampleDiagnostic("c"), path: "src/lib.rs" },
    { ...sampleDiagnostic("d"), path: "src/lib.rs" },
    { ...sampleDiagnostic("e"), path: "src/other.rs" },
  ];
  const capped = capDiagnosticsPerFile(diagnostics, 2);
  assert.equal(capped.length, 3);
  assert.deepEqual(
    capped.map((d) => d.cardId),
    ["a", "b", "e"],
  );
});

test("capDiagnosticsPerFile returns input when cap is non-positive", () => {
  const diagnostics = [sampleDiagnostic("x")];
  assert.equal(capDiagnosticsPerFile(diagnostics, 0).length, 1);
});

test("diagnosticCapSummaries reports hidden diagnostics by file", () => {
  const diagnostics = [
    { ...sampleDiagnostic("a"), path: "src/lib.rs" },
    { ...sampleDiagnostic("b"), path: "src/lib.rs" },
    { ...sampleDiagnostic("c"), path: "src/lib.rs" },
    { ...sampleDiagnostic("d"), path: "src/other.rs" },
  ];
  assert.deepEqual(diagnosticCapSummaries(diagnostics, 2), [
    { path: "src/lib.rs", total: 3, visible: 2, hidden: 1 },
    { path: "src/other.rs", total: 1, visible: 1, hidden: 0 },
  ]);
  assert.deepEqual(diagnosticCapSummaries(diagnostics, 0), []);
});

test("range binding uses containment and intersection instead of proximity", () => {
  const range = { start: { line: 7, character: 4 }, end: { line: 7, character: 42 } };
  assert.equal(positionInRange({ line: 7, character: 4 }, range), true);
  assert.equal(positionInRange({ line: 7, character: 42 }, range), true);
  assert.equal(positionInRange({ line: 7, character: 3 }, range), false);
  assert.equal(positionInRange({ line: 8, character: 4 }, range), false);
  assert.equal(rangesIntersect(range, { start: { line: 7, character: 20 }, end: { line: 7, character: 21 } }), true);
  assert.equal(rangesIntersect(range, { start: { line: 8, character: 0 }, end: { line: 8, character: 1 } }), false);
  assert.equal(rangesEqual(range, { ...range, start: { ...range.start } }), true);
});

test("resolveWorkspaceFilePath keeps paths inside workspace", () => {
  const root = path.resolve("workspace-root");
  assert.equal(
    resolveWorkspaceFilePath(root, path.join("src", "lib.rs")),
    path.join(root, "src", "lib.rs"),
  );
  assert.equal(resolveWorkspaceFilePath(root, path.join("..", "secret.rs")), undefined);
});

function sampleDiagnostic(cardId: string) {
  return {
    cardId,
    code: "guard_missing",
    message: "msg",
    path: "src/lib.rs",
    range: {
      start: { line: 1, character: 0 },
      end: { line: 1, character: 5 },
    },
    source: "unsafe-review",
  };
}
