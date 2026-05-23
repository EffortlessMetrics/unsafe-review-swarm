import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  BundleCodeAction,
  BundleDiagnostic,
  BundleHover,
  BundleParseError,
  ParsedBundle,
  capDiagnosticsPerFile,
  diagnosticsByFile,
  parseBundle,
  resolveWorkspaceFilePath,
} from "./bundle";

const EXTENSION_ID = "unsafe-review";
const DEFAULT_BUNDLE_PATH = "target/unsafe-review/lsp.json";
const PR_SUMMARY_PATH = "target/unsafe-review/pr-summary.md";
const WITNESS_PLAN_PATH = "target/unsafe-review/witness-plan.md";
const DEFAULT_MAX_DIAGNOSTICS_PER_FILE = 200;

const TRUST_BOUNDARY_FOOTER =
  "*Advisory unsafe contract review. Not memory-safety proof, " +
  "not UB-free status, not Miri-clean status, and not site-execution proof.*";

interface AdapterState {
  bundle: ParsedBundle | undefined;
  bundleRoot: string | undefined;
  diagnosticsCollection: vscode.DiagnosticCollection;
  hoversByPath: Map<string, BundleHover[]>;
  codeActionsByPath: Map<string, BundleCodeAction[]>;
  statusBar: vscode.StatusBarItem;
  output: vscode.OutputChannel;
}

let adapter: AdapterState | undefined;
let bundleWatcher: vscode.FileSystemWatcher | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("unsafe-review");
  const diagnosticsCollection = vscode.languages.createDiagnosticCollection(EXTENSION_ID);
  const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  statusBar.text = "unsafe-review: idle";
  statusBar.tooltip = "Click to refresh the unsafe-review bundle";
  statusBar.command = `${EXTENSION_ID}.refreshBundle`;

  adapter = {
    bundle: undefined,
    bundleRoot: undefined,
    diagnosticsCollection,
    hoversByPath: new Map(),
    codeActionsByPath: new Map(),
    statusBar,
    output,
  };

  context.subscriptions.push(
    diagnosticsCollection,
    statusBar,
    output,
    vscode.languages.registerHoverProvider(
      [{ language: "rust" }, { pattern: "**/*.rs" }],
      new BundleHoverProvider(),
    ),
    vscode.languages.registerCodeActionsProvider(
      [{ language: "rust" }, { pattern: "**/*.rs" }],
      new BundleCodeActionProvider(),
      { providedCodeActionKinds: [vscode.CodeActionKind.Empty] },
    ),
    vscode.commands.registerCommand(`${EXTENSION_ID}.refreshBundle`, refreshBundle),
    vscode.commands.registerCommand(`${EXTENSION_ID}.openPrSummary`, openPrSummary),
    vscode.commands.registerCommand(`${EXTENSION_ID}.openWitnessPlan`, openWitnessPlan),
    vscode.commands.registerCommand(`${EXTENSION_ID}.openRelatedTest`, openRelatedTest),
    vscode.commands.registerCommand(`${EXTENSION_ID}.copyAgentPacket`, copyAgentPacket),
    vscode.commands.registerCommand(`${EXTENSION_ID}.copyWitnessCommand`, copyWitnessCommand),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("unsafeReview")) {
        rewireBundleWatcher();
        void refreshBundle();
      }
    }),
    vscode.workspace.onDidChangeWorkspaceFolders(() => {
      rewireBundleWatcher();
      void refreshBundle();
    }),
  );

  rewireBundleWatcher();
  void refreshBundle();
  statusBar.show();
}

export function deactivate(): void {
  if (bundleWatcher !== undefined) {
    bundleWatcher.dispose();
    bundleWatcher = undefined;
  }
  if (adapter !== undefined) {
    adapter.diagnosticsCollection.dispose();
    adapter.statusBar.dispose();
    adapter.output.dispose();
    adapter = undefined;
  }
}

function rewireBundleWatcher(): void {
  if (bundleWatcher !== undefined) {
    bundleWatcher.dispose();
    bundleWatcher = undefined;
  }
  const folder = primaryWorkspaceFolder();
  if (folder === undefined) {
    return;
  }
  const settings = readSettings();
  const pattern = new vscode.RelativePattern(folder, settings.bundlePath);
  const watcher = vscode.workspace.createFileSystemWatcher(pattern);
  watcher.onDidChange(() => {
    if (settings.autoRefreshOnSave) {
      void refreshBundle();
    }
  });
  watcher.onDidCreate(() => void refreshBundle());
  watcher.onDidDelete(() => void clearBundle());
  bundleWatcher = watcher;
}

interface AdapterSettings {
  bundlePath: string;
  autoRefreshOnSave: boolean;
  maxDiagnosticsPerFile: number;
}

function readSettings(): AdapterSettings {
  const config = vscode.workspace.getConfiguration("unsafeReview");
  const bundlePath = (config.get<string>("bundlePath") ?? DEFAULT_BUNDLE_PATH).trim();
  const autoRefreshOnSave = config.get<boolean>("autoRefreshOnSave") ?? false;
  const maxDiagnosticsPerFile =
    config.get<number>("maxDiagnosticsPerFile") ?? DEFAULT_MAX_DIAGNOSTICS_PER_FILE;
  return {
    bundlePath: bundlePath.length === 0 ? DEFAULT_BUNDLE_PATH : bundlePath,
    autoRefreshOnSave,
    maxDiagnosticsPerFile,
  };
}

function primaryWorkspaceFolder(): vscode.WorkspaceFolder | undefined {
  const folders = vscode.workspace.workspaceFolders;
  if (folders === undefined || folders.length === 0) {
    return undefined;
  }
  return folders[0];
}

async function refreshBundle(): Promise<void> {
  if (adapter === undefined) {
    return;
  }
  const folder = primaryWorkspaceFolder();
  if (folder === undefined) {
    clearBundleState();
    setStatus("no workspace open", undefined);
    return;
  }
  const settings = readSettings();
  const absolutePath = path.join(folder.uri.fsPath, settings.bundlePath);

  let text: string;
  try {
    text = await fs.promises.readFile(absolutePath, { encoding: "utf8" });
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code === "ENOENT") {
      setStatus(
        `bundle not found at ${settings.bundlePath}; run \`unsafe-review first-pr\``,
        undefined,
      );
      clearBundleState();
      return;
    }
    adapter.output.appendLine(`refresh failed: ${(err as Error).message}`);
    clearBundleState();
    setStatus("bundle read failed (see Output)", undefined);
    return;
  }

  let parsed: ParsedBundle;
  try {
    parsed = parseBundle(text);
  } catch (err) {
    if (err instanceof BundleParseError) {
      adapter.output.appendLine(`parse failed: ${err.message}`);
      clearBundleState();
      setStatus("bundle parse failed (see Output)", undefined);
      return;
    }
    throw err;
  }
  for (const warning of parsed.warnings) {
    adapter.output.appendLine(`bundle warning: ${warning}`);
  }

  adapter.bundle = parsed;
  adapter.bundleRoot = folder.uri.fsPath;
  applyDiagnostics(parsed, settings, folder);
  applyHovers(parsed, folder);
  applyCodeActions(parsed, folder);

  setStatus(parsed.status.message, parsed.status.trustBoundary);
}

function applyDiagnostics(
  bundle: ParsedBundle,
  settings: AdapterSettings,
  folder: vscode.WorkspaceFolder,
): void {
  if (adapter === undefined) {
    return;
  }
  adapter.diagnosticsCollection.clear();

  const capped = capDiagnosticsPerFile(bundle.diagnostics, settings.maxDiagnosticsPerFile);
  const grouped = diagnosticsByFile(capped);
  for (const [relativePath, list] of grouped) {
    const absolute = resolveWorkspaceFile(folder, relativePath);
    if (absolute === undefined) {
      adapter.output.appendLine(`bundle warning: diagnostic path escapes workspace: ${relativePath}`);
      continue;
    }
    const uri = vscode.Uri.file(absolute);
    const diags = list.map((entry) => toVscodeDiagnostic(entry));
    adapter.diagnosticsCollection.set(uri, diags);
  }
}

function applyHovers(bundle: ParsedBundle, folder: vscode.WorkspaceFolder): void {
  if (adapter === undefined) {
    return;
  }
  const grouped = new Map<string, BundleHover[]>();
  for (const hover of bundle.hovers) {
    const absolute = resolveWorkspaceFile(folder, hover.path);
    if (absolute === undefined) {
      adapter.output.appendLine(`bundle warning: hover path escapes workspace: ${hover.path}`);
      continue;
    }
    const list = grouped.get(absolute);
    if (list === undefined) {
      grouped.set(absolute, [hover]);
    } else {
      list.push(hover);
    }
  }
  adapter.hoversByPath = grouped;
}

function applyCodeActions(bundle: ParsedBundle, folder: vscode.WorkspaceFolder): void {
  if (adapter === undefined) {
    return;
  }
  const grouped = new Map<string, BundleCodeAction[]>();
  for (const action of bundle.codeActions) {
    const absolute = resolveWorkspaceFile(folder, action.path);
    if (absolute === undefined) {
      adapter.output.appendLine(`bundle warning: code action path escapes workspace: ${action.path}`);
      continue;
    }
    const list = grouped.get(absolute);
    if (list === undefined) {
      grouped.set(absolute, [action]);
    } else {
      list.push(action);
    }
  }
  adapter.codeActionsByPath = grouped;
}

function clearBundle(): Promise<void> {
  if (adapter === undefined) {
    return Promise.resolve();
  }
  clearBundleState();
  setStatus("bundle removed; run `unsafe-review first-pr` to refresh", undefined);
  return Promise.resolve();
}

function clearBundleState(): void {
  if (adapter === undefined) {
    return;
  }
  adapter.diagnosticsCollection.clear();
  adapter.bundle = undefined;
  adapter.hoversByPath = new Map();
  adapter.codeActionsByPath = new Map();
}

function toVscodeDiagnostic(entry: BundleDiagnostic): vscode.Diagnostic {
  const range = new vscode.Range(
    new vscode.Position(entry.range.start.line, entry.range.start.character),
    new vscode.Position(entry.range.end.line, entry.range.end.character),
  );
  const severity = severityFromBundle(entry.severity);
  const diagnostic = new vscode.Diagnostic(range, entry.message, severity);
  diagnostic.source = entry.source ?? "unsafe-review";
  diagnostic.code = {
    value: entry.cardId,
    target: vscode.Uri.parse("https://crates.io/crates/unsafe-review"),
  };
  return diagnostic;
}

function severityFromBundle(value: number | undefined): vscode.DiagnosticSeverity {
  switch (value) {
    case 1:
      return vscode.DiagnosticSeverity.Error;
    case 2:
      return vscode.DiagnosticSeverity.Warning;
    case 4:
      return vscode.DiagnosticSeverity.Hint;
    case 3:
    default:
      return vscode.DiagnosticSeverity.Information;
  }
}

function setStatus(message: string, boundary: string | undefined): void {
  if (adapter === undefined) {
    return;
  }
  adapter.statusBar.text = `unsafe-review: ${message}`;
  adapter.statusBar.tooltip = boundary ?? "unsafe-review (advisory)";
}

class BundleHoverProvider implements vscode.HoverProvider {
  provideHover(
    document: vscode.TextDocument,
    position: vscode.Position,
  ): vscode.ProviderResult<vscode.Hover> {
    if (adapter === undefined) {
      return undefined;
    }
    const candidates = adapter.hoversByPath.get(document.uri.fsPath);
    if (candidates === undefined || candidates.length === 0) {
      return undefined;
    }
    let chosen: BundleHover | undefined;
    let chosenDelta = Number.MAX_SAFE_INTEGER;
    for (const hover of candidates) {
      const delta = Math.abs(hover.position.line - position.line);
      if (delta < chosenDelta) {
        chosen = hover;
        chosenDelta = delta;
      }
    }
    if (chosen === undefined || chosenDelta > 3) {
      return undefined;
    }
    const md = new vscode.MarkdownString();
    md.isTrusted = false;
    md.supportHtml = false;
    md.appendMarkdown(withTrustBoundaryFooter(chosen.contents, chosen.trustBoundary));
    return new vscode.Hover(md);
  }
}

class BundleCodeActionProvider implements vscode.CodeActionProvider {
  provideCodeActions(
    document: vscode.TextDocument,
    range: vscode.Range | vscode.Selection,
  ): vscode.ProviderResult<vscode.CodeAction[]> {
    if (adapter === undefined) {
      return undefined;
    }
    const candidates = adapter.codeActionsByPath.get(document.uri.fsPath);
    if (candidates === undefined || candidates.length === 0) {
      return undefined;
    }
    const actions: vscode.CodeAction[] = [];
    for (const candidate of candidates) {
      if (candidate.range !== undefined) {
        const cardLine = candidate.range.start.line;
        if (Math.abs(cardLine - range.start.line) > 5) {
          continue;
        }
      }
      const action = new vscode.CodeAction(
        decorateCodeActionTitle(candidate),
        vscode.CodeActionKind.Empty,
      );
      action.command = {
        title: candidate.title,
        command: extensionCommandFor(candidate.command),
        arguments: [candidate.payload ?? {}],
      };
      actions.push(action);
    }
    return actions;
  }
}

function decorateCodeActionTitle(action: BundleCodeAction): string {
  if (/\((copy|open)\)$/.test(action.title)) {
    return action.title;
  }
  if (action.command.includes("copy")) {
    return `${action.title} (copy)`;
  }
  if (action.command.includes("open") || action.command.includes("Open")) {
    return `${action.title} (open)`;
  }
  return action.title;
}

function extensionCommandFor(bundleCommand: string): string {
  switch (bundleCommand) {
    case "unsafe-review.copyAgentPacket":
      return `${EXTENSION_ID}.copyAgentPacket`;
    case "unsafe-review.copyWitnessCommand":
      return `${EXTENSION_ID}.copyWitnessCommand`;
    case "unsafe-review.openRelatedTest":
      return `${EXTENSION_ID}.openRelatedTest`;
    case "unsafe-review.openPrSummary":
      return `${EXTENSION_ID}.openPrSummary`;
    case "unsafe-review.openWitnessPlan":
      return `${EXTENSION_ID}.openWitnessPlan`;
    default:
      return `${EXTENSION_ID}.refreshBundle`;
  }
}

async function openPrSummary(): Promise<void> {
  await openBundleSibling(PR_SUMMARY_PATH);
}

async function openWitnessPlan(): Promise<void> {
  await openBundleSibling(WITNESS_PLAN_PATH);
}

async function openBundleSibling(defaultRelative: string): Promise<void> {
  const folder = primaryWorkspaceFolder();
  if (folder === undefined) {
    void vscode.window.showInformationMessage("unsafe-review: no workspace open");
    return;
  }
  const settings = readSettings();
  const relative = path.join(path.dirname(settings.bundlePath), path.basename(defaultRelative));
  await openWorkspaceFile(folder, relative);
}

async function openRelatedTest(payload: unknown): Promise<void> {
  const folder = primaryWorkspaceFolder();
  if (folder === undefined) {
    void vscode.window.showInformationMessage("unsafe-review: no workspace open");
    return;
  }
  if (!isRecord(payload) || typeof payload["file"] !== "string" || payload["file"].length === 0) {
    void vscode.window.showInformationMessage("unsafe-review: no related test file in action");
    return;
  }
  await openWorkspaceFile(folder, payload["file"], readPositiveInteger(payload["line"]));
}

async function openWorkspaceFile(
  folder: vscode.WorkspaceFolder,
  relative: string,
  oneBasedLine?: number,
): Promise<void> {
  const target = resolveWorkspaceFile(folder, relative);
  if (target === undefined) {
    void vscode.window.showInformationMessage(
      `unsafe-review: refused to open path outside workspace: ${relative}`,
    );
    return;
  }
  if (!fs.existsSync(target)) {
    void vscode.window.showInformationMessage(
      `unsafe-review: ${relative} not found. Run \`unsafe-review first-pr\` first.`,
    );
    return;
  }
  const doc = await vscode.workspace.openTextDocument(target);
  const editor = await vscode.window.showTextDocument(doc, { preview: false });
  if (oneBasedLine !== undefined) {
    const line = Math.max(oneBasedLine - 1, 0);
    const position = new vscode.Position(line, 0);
    editor.selection = new vscode.Selection(position, position);
    editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
  }
}

async function copyAgentPacket(payload: unknown): Promise<void> {
  const cardId = pickCardId(payload);
  if (cardId === undefined) {
    void vscode.window.showInformationMessage(
      "unsafe-review: no card id in selection; right-click an unsafe-review diagnostic and select Copy Agent Packet.",
    );
    return;
  }
  const command = `unsafe-review context ${shellQuote(cardId)} --json`;
  await vscode.env.clipboard.writeText(command);
  void vscode.window.showInformationMessage(
    `unsafe-review: copied \`${command}\`. Run it in your terminal to print the bounded agent packet.`,
  );
}

async function copyWitnessCommand(payload: unknown): Promise<void> {
  let command: string | undefined;
  if (isRecord(payload)) {
    const raw = payload["command"];
    if (typeof raw === "string" && raw.length > 0) {
      command = raw;
    }
  }
  if (command === undefined) {
    const cardId = pickCardId(payload);
    if (cardId !== undefined) {
      command = `unsafe-review explain ${shellQuote(cardId)}`;
    }
  }
  if (command === undefined) {
    void vscode.window.showInformationMessage(
      "unsafe-review: no witness command available for this card",
    );
    return;
  }
  await vscode.env.clipboard.writeText(command);
  void vscode.window.showInformationMessage(`unsafe-review: copied \`${command}\``);
}

function pickCardId(payload: unknown): string | undefined {
  if (isRecord(payload)) {
    const cardId = payload["card_id"] ?? payload["cardId"];
    if (typeof cardId === "string" && cardId.length > 0) {
      return cardId;
    }
  }
  return undefined;
}

function readPositiveInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : undefined;
}

function resolveWorkspaceFile(
  folder: vscode.WorkspaceFolder,
  workspaceRelativePath: string,
): string | undefined {
  return resolveWorkspaceFilePath(folder.uri.fsPath, workspaceRelativePath);
}

function withTrustBoundaryFooter(contents: string, boundary: string | undefined): string {
  const footer = boundary ?? TRUST_BOUNDARY_FOOTER;
  const lower = contents.toLowerCase();
  if (
    lower.includes("trust boundary") ||
    lower.includes("not memory-safety proof") ||
    lower.includes("not a proof of memory safety")
  ) {
    return contents;
  }
  return `${contents}\n\n---\n\n${footer}`;
}

function shellQuote(value: string): string {
  if (/^[A-Za-z0-9_\-./@:+=]+$/.test(value)) {
    return value;
  }
  return `'${value.replace(/'/g, "'\\''")}'`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
