const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const Module = require("node:module");

class Disposable {
  dispose() {}
}

class Uri {
  constructor(fsPath) {
    this.fsPath = fsPath;
  }

  static file(fsPath) {
    return new Uri(path.resolve(fsPath));
  }

  static parse(value) {
    return new Uri(value);
  }
}

class Position {
  constructor(line, character) {
    this.line = line;
    this.character = character;
  }
}

class Range {
  constructor(start, end) {
    this.start = start;
    this.end = end;
  }
}

class Selection extends Range {}

class Diagnostic {
  constructor(range, message, severity) {
    this.range = range;
    this.message = message;
    this.severity = severity;
  }
}

class MarkdownString {
  constructor() {
    this.value = "";
  }

  appendMarkdown(value) {
    this.value += value;
  }
}

class Hover {
  constructor(contents) {
    this.contents = contents;
  }
}

class CodeActionKind {
  constructor(value) {
    this.value = value;
  }

  append(value) {
    return new CodeActionKind(this.value.length === 0 ? value : `${this.value}.${value}`);
  }
}

CodeActionKind.Empty = new CodeActionKind("");

class CodeAction {
  constructor(title, kind) {
    this.title = title;
    this.kind = kind;
  }
}

class DiagnosticCollection extends Disposable {
  constructor() {
    super();
    this.entries = new Map();
  }

  clear() {
    this.entries.clear();
  }

  set(uri, diagnostics) {
    this.entries.set(uri.fsPath, diagnostics);
  }
}

class OutputChannel extends Disposable {
  constructor() {
    super();
    this.lines = [];
  }

  appendLine(line) {
    this.lines.push(line);
  }
}

class StatusBarItem extends Disposable {
  constructor() {
    super();
    this.text = "";
    this.tooltip = "";
  }

  show() {}
}

class FileSystemWatcher extends Disposable {
  onDidChange() {
    return new Disposable();
  }

  onDidCreate() {
    return new Disposable();
  }

  onDidDelete() {
    return new Disposable();
  }
}

class RelativePattern {
  constructor(base, pattern) {
    this.baseUri = base.uri;
    this.pattern = pattern;
  }
}

function makeVscodeHost(root, settings) {
  const state = {
    collections: [],
    hovers: [],
    codeActions: [],
    commands: new Map(),
    output: undefined,
    status: undefined,
    clipboard: "",
    messages: [],
  };
  const folder = { uri: Uri.file(root) };
  const vscode = {
    CodeAction,
    CodeActionKind,
    Diagnostic,
    DiagnosticCollection,
    DiagnosticSeverity: { Warning: 0, Information: 1, Hint: 2 },
    Hover,
    MarkdownString,
    Position,
    Range,
    RelativePattern,
    Selection,
    StatusBarAlignment: { Left: 1 },
    TextEditorRevealType: { InCenter: 1 },
    Uri,
    env: {
      clipboard: {
        writeText: async (value) => {
          state.clipboard = value;
        },
      },
    },
    commands: {
      registerCommand: (name, callback) => {
        state.commands.set(name, callback);
        return new Disposable();
      },
    },
    languages: {
      createDiagnosticCollection: () => {
        const collection = new DiagnosticCollection();
        state.collections.push(collection);
        return collection;
      },
      registerHoverProvider: (_selector, provider) => {
        state.hovers.push(provider);
        return new Disposable();
      },
      registerCodeActionsProvider: (_selector, provider) => {
        state.codeActions.push(provider);
        return new Disposable();
      },
    },
    window: {
      createOutputChannel: () => {
        state.output = new OutputChannel();
        return state.output;
      },
      createStatusBarItem: () => {
        state.status = new StatusBarItem();
        return state.status;
      },
      showInformationMessage: async (message) => {
        state.messages.push(message);
        return undefined;
      },
      showWarningMessage: async (message) => {
        state.messages.push(message);
        return undefined;
      },
      showTextDocument: async (document) => ({
        document,
        selection: undefined,
        revealRange() {},
      }),
    },
    workspace: {
      workspaceFolders: [folder],
      getConfiguration: () => ({
        get: (key) => settings[key],
      }),
      createFileSystemWatcher: () => new FileSystemWatcher(),
      onDidChangeConfiguration: () => new Disposable(),
      onDidChangeWorkspaceFolders: () => new Disposable(),
      openTextDocument: async (filePath) => ({ uri: Uri.file(filePath) }),
    },
  };
  return { state, vscode };
}

function analysis(generation) {
  return {
    analysis_id: "extension-smoke-analysis",
    generation,
    tool_version: "0.3.8",
    scope: "diff",
    state: "current",
  };
}

function packet(cardId, identity) {
  return JSON.stringify({
    schema_version: "0.1",
    mode: "bounded_repair_packet",
    card_id: cardId,
    analysis: identity,
  });
}

function diagnostic(cardId, line, readiness) {
  return {
    card_id: cardId,
    code: "guard_missing",
    message: `${cardId} explanation`,
    path: "src/lib.rs",
    range: {
      start: { line, character: 0 },
      end: { line, character: 5 },
    },
    severity: 2,
    source: "unsafe-review",
    coverage: { agent_lsp_readiness: readiness },
  };
}

function action(card, identity, packetIdentity = identity) {
  const range = card.range;
  const readiness = card.readiness;
  return {
    action_id: "agent-packet",
    title: `Review ${card.card_id}`,
    kind: readiness === "ready_for_agent"
      ? "quickfix.unsafeReview.agentPacket"
      : "source.unsafeReview.reviewContext",
    diagnostic: { card_id: card.card_id, path: card.path, range },
    payload: {
      action_id: "agent-packet",
      card_id: card.card_id,
      analysis: identity,
      agent_readiness: readiness,
      agent_packet: packet(card.card_id, packetIdentity),
    },
    command: {
      command: "unsafe-review.collectAgentPacket",
      arguments: { card_id: card.card_id, analysis: identity },
    },
    applicability: { state: "available" },
    is_preferred: false,
    command_only: true,
    trust_boundary: "Advisory unsafe contract review.",
  };
}

function makeBundle(identity, staleOrMissing = false) {
  const human = diagnostic("card-human", 0, "needs_human");
  const ready = diagnostic("card-ready", 1, "ready");
  const hidden = diagnostic("card-hidden", 2, "ready");
  const cards = [human, ready, hidden].map((card) => ({
    ...card,
    readiness: card.card_id === "card-human" ? "requires_human_review" : "ready_for_agent",
  }));
  const hovers = cards.slice(0, 2).map((card) => ({
    card_id: card.card_id,
    path: card.path,
    position: card.range.start,
    range: card.range,
    contents: `${card.card_id} exact hover`,
    analysis: identity,
  }));
  const codeActions = [action(cards[0], identity), action(cards[1], identity)];
  if (staleOrMissing) {
    codeActions.push(action(cards[0], identity, analysis(2)));
    codeActions.push({ ...action(cards[1], identity), payload: { ...action(cards[1], identity).payload, agent_packet: undefined } });
  }
  return {
    schema_version: "0.2",
    tool: "unsafe-review",
    analysis: identity,
    status: { message: "3 unsafe-review card(s)", trust_boundary: "Advisory unsafe contract review." },
    trust_boundary: "Advisory unsafe contract review.",
    diagnostics: cards,
    hovers,
    code_actions: codeActions,
  };
}

async function waitFor(predicate) {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  assert.fail("extension host smoke timed out waiting for bundle refresh");
}

(async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "unsafe-review-vscode-host-"));
  try {
  fs.mkdirSync(path.join(root, "src"), { recursive: true });
  fs.writeFileSync(path.join(root, "src", "lib.rs"), "fn smoke() {}\n");
  fs.mkdirSync(path.join(root, "target", "unsafe-review"), { recursive: true });
  const identity = analysis(1);
  const bundlePath = path.join(root, "target", "unsafe-review", "lsp.json");
  fs.writeFileSync(bundlePath, JSON.stringify(makeBundle(identity, true)));

  const host = makeVscodeHost(root, {
    bundlePath: "target/unsafe-review/lsp.json",
    autoRefreshOnSave: false,
    maxDiagnosticsPerFile: 2,
  });
  const originalLoad = Module._load;
  Module._load = function (request, parent, isMain) {
    if (request === "vscode") return host.vscode;
    return originalLoad.call(this, request, parent, isMain);
  };
  const extension = require(path.resolve(__dirname, "../out/extension.js"));
  Module._load = originalLoad;

  const context = { subscriptions: [] };
  extension.activate(context);
  await waitFor(() => host.state.status?.text.includes("diagnostic(s) hidden"));

  const collection = host.state.collections[0];
  assert.equal(collection.entries.get(path.join(root, "src", "lib.rs")).length, 2);
  assert.match(host.state.status.text, /1 diagnostic\(s\) hidden/);
  assert.ok(host.state.output.lines.some((line) => line.includes("hidden for src/lib.rs")));
  assert.ok(host.state.output.lines.some((line) => line.includes("inconsistent canonical identity")));

  const document = { uri: Uri.file(path.join(root, "src", "lib.rs")) };
  const humanHover = host.state.hovers[0].provideHover(document, new Position(0, 1));
  const readyHover = host.state.hovers[0].provideHover(document, new Position(1, 1));
  assert.match(humanHover.contents.value, /card-human exact hover/);
  assert.match(readyHover.contents.value, /card-ready exact hover/);
  assert.notEqual(humanHover.contents.value, readyHover.contents.value);

  const humanActions = host.state.codeActions[0].provideCodeActions(
    document,
    new Range(new Position(0, 0), new Position(0, 5)),
  );
  const readyActions = host.state.codeActions[0].provideCodeActions(
    document,
    new Range(new Position(1, 0), new Position(1, 5)),
  );
  assert.equal(humanActions.length, 1);
  assert.equal(readyActions.length, 1);
  assert.match(humanActions[0].title, /card-human/);
  assert.match(readyActions[0].title, /card-ready/);
  assert.equal(humanActions[0].kind.value, "source.unsafeReview.reviewContext");
  assert.equal(readyActions[0].kind.value, "quickfix.unsafeReview.agentPacket");

  await host.state.commands.get("unsafe-review.copyAgentPacket")(
    readyActions[0].command.arguments[0],
  );
  assert.equal(JSON.parse(host.state.clipboard).card_id, "card-ready");
  assert.equal(JSON.parse(host.state.clipboard).analysis.generation, 1);

  const staleBundle = makeBundle(analysis(2));
  staleBundle.code_actions = [action({
    ...staleBundle.diagnostics[0],
    readiness: "ready_for_agent",
  }, analysis(2), analysis(1))];
  fs.writeFileSync(bundlePath, JSON.stringify(staleBundle));
  await host.state.commands.get("unsafe-review.refreshBundle")();
  await waitFor(() => host.state.status?.text.includes("3 unsafe-review card(s)"));
  const staleActions = host.state.codeActions[0].provideCodeActions(
    document,
    new Range(new Position(0, 0), new Position(0, 5)),
  );
  assert.equal((staleActions ?? []).length, 0);

  extension.deactivate();
  console.log("extension-host-smoke: ok (activation, adjacent cards, human-only readiness, stale/missing packets, cap visibility)");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
