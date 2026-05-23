import { strict as assert } from "node:assert";
import * as path from "node:path";
import test from "node:test";

import {
  BundleParseError,
  capDiagnosticsPerFile,
  diagnosticsByFile,
  parseBundle,
  resolveWorkspaceFilePath,
} from "../bundle";

const MINIMAL_BUNDLE = {
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
      next_action: "Add the alignment guard.",
      witness_routes: ["miri"],
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
        trust_boundary: "Static unsafe contract review only.",
      },
      arguments: [],
    },
  ],
};

test("parseBundle returns diagnostics, hovers, and code actions", () => {
  const result = parseBundle(JSON.stringify(MINIMAL_BUNDLE));
  assert.equal(result.diagnostics.length, 1);
  assert.equal(result.diagnostics[0].cardId, "UR-foo");
  assert.equal(result.diagnostics[0].operation, "unsafe { ptr.cast::<Header>().read() }");
  assert.equal(result.hovers.length, 1);
  assert.equal(result.hovers[0].contents.includes("guard_missing"), true);
  assert.equal(result.codeActions.length, 1);
  assert.equal(result.codeActions[0].command, "unsafe-review.copyAgentPacket");
  assert.equal(result.codeActions[0].payload?.cardId, "UR-foo");
  assert.equal(result.warnings.length, 0);
});

test("parseBundle rejects non-JSON", () => {
  assert.throws(() => parseBundle("not json"), BundleParseError);
});

test("parseBundle rejects non-object root", () => {
  assert.throws(() => parseBundle("[]"), BundleParseError);
});

test("parseBundle uses default trust boundary when missing", () => {
  const result = parseBundle(JSON.stringify({ tool: "unsafe-review" }));
  assert.ok(result.trustBoundary.length > 0);
  assert.ok(result.trustBoundary.toLowerCase().includes("not a proof"));
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

test("parseBundle skips diagnostics missing card_id, path, or message", () => {
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
    ],
  };
  const result = parseBundle(JSON.stringify(broken));
  assert.equal(result.diagnostics.length, 1);
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
          path: "src/other.rs",
        },
      ],
    }),
  );
  const grouped = diagnosticsByFile(result.diagnostics);
  assert.equal(grouped.size, 2);
  assert.equal(grouped.get("src/lib.rs")?.length, 1);
  assert.equal(grouped.get("src/other.rs")?.length, 1);
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
