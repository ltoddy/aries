import type { ChatBlock, ChatMessage, ChatStreamPayload, ThemeMode, ToolCallData } from "./types";
import { THEME_STORAGE_KEY } from "./constants";

export function getPreferredTheme(): ThemeMode {
  const stored = localStorage.getItem(THEME_STORAGE_KEY);
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }
  return "system";
}

export function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode;
}

export function findLastAssistantIndex(messages: ChatMessage[]): number | null {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === "assistant") {
      return i;
    }
  }
  return null;
}

export function appendStreamBlock(message: ChatMessage, payload: ChatStreamPayload): ChatMessage {
  // Handle usage event separately - store on message, don't add as block
  if (payload.kind === "usage") {
    try {
      const usage = JSON.parse(payload.delta);
      return { ...message, usage };
    } catch {
      return message;
    }
  }

  const prevBlocks = message.blocks ?? [];
  const lastBlock = prevBlocks[prevBlocks.length - 1];
  let blocks: ChatBlock[];
  let textDelta = "";

  if (lastBlock && lastBlock.type === payload.kind) {
    // 就地更新最后一个 block，只替换最后一个元素（避免 slice 整个数组）
    const updatedBlock = { type: lastBlock.type, content: lastBlock.content + payload.delta };
    blocks = prevBlocks.slice();
    blocks[blocks.length - 1] = updatedBlock;
    if (payload.kind === "text") textDelta = payload.delta;
  } else {
    blocks = [...prevBlocks, { type: payload.kind, content: payload.delta }];
    if (payload.kind === "text") textDelta = payload.delta;
  }

  // 增量更新 textContent，无需每次遍历所有 blocks
  const prevTextContent = message.content ?? "";
  const textContent = prevTextContent + textDelta;

  return { ...message, content: textContent, blocks };
}

export function blockLabel(type: ChatBlock["type"]) {
  switch (type) {
    case "reasoning":
      return "Reasoning";
    case "tool-call":
      return "Tool Call";
    case "tool-result":
      return "Tool Result";
    default:
      return "Response";
  }
}

export function formatToolResult(toolName: string, content: string): string {
  const trimmed = content.trim();
  if (!trimmed) return "No output";

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return trimmed;
  }

  switch (toolName) {
    case "read_file": {
      // ReadFileOutput: { content: string }
      if (parsed && typeof parsed === "object" && typeof (parsed as { content?: unknown }).content === "string") {
        return (parsed as { content: string }).content;
      }
      return trimmed;
    }
    case "write_file": {
      const obj = parsed as Record<string, unknown>;
      return obj.success ? "File written successfully" : "Failed to write file";
    }
    case "shell_command": {
      const obj = parsed as Record<string, unknown>;
      let out = "";
      if (obj.stdout && typeof obj.stdout === "string") out += obj.stdout;
      if (obj.stderr && typeof obj.stderr === "string") {
        if (out) out += "\n";
        out += obj.stderr;
      }
      return out || "No output";
    }
    case "glob": {
      const obj = parsed as Record<string, unknown>;
      const files = Array.isArray(obj.files) ? obj.files : [];
      return files.join("\n") || "No matches";
    }
    case "grep": {
      const obj = parsed as Record<string, unknown>;
      const matches = Array.isArray(obj.matches) ? obj.matches : [];
      return matches.join("\n") || "No matches";
    }
    case "ls": {
      const obj = parsed as Record<string, unknown>;
      const entries = Array.isArray(obj.entries) ? obj.entries : [];
      return entries.join("\n") || "Empty directory";
    }
    case "edit":
    case "multiedit":
    case "apply_patch": {
      const obj = parsed as Record<string, unknown>;
      return str(obj.message) || (obj.success ? "Success" : "Failed");
    }
    case "task": {
      const obj = parsed as Record<string, unknown>;
      return str(obj.result);
    }
    case "question": {
      const obj = parsed as Record<string, unknown>;
      const answers = Array.isArray(obj.answers) ? obj.answers : [];
      return answers.join("\n");
    }
    case "web_search": {
      const obj = parsed as Record<string, unknown>;
      return str(obj.results);
    }
    case "web_fetch": {
      const obj = parsed as Record<string, unknown>;
      return str(obj.content);
    }
    case "code_search": {
      const obj = parsed as Record<string, unknown>;
      return str(obj.results);
    }
    case "skill": {
      const obj = parsed as Record<string, unknown>;
      const meta = obj.metadata as Record<string, unknown> | undefined;
      let out = "";
      if (meta) out += `${meta.name} at ${meta.dir}\n`;
      out += str(obj.output);
      return out;
    }
    default:
      return typeof parsed === "string" ? parsed : JSON.stringify(parsed, null, 2);
  }
}

export function parseToolCall(content: string): ToolCallData {
  const lines = content.split("\n");
  const firstLine = lines[0] ?? "";
  const name = firstLine.replace(/^\[Tool\]\s*/, "").trim() || "Unknown Tool";
  const argumentsBlock = lines.slice(1).join("\n").trim();

  if (!argumentsBlock) {
    return { name, summary: "", detail: null };
  }

  let args: Record<string, unknown>;
  try {
    args = JSON.parse(argumentsBlock);
  } catch {
    return { name, summary: argumentsBlock, detail: null };
  }

  return formatToolCallArgs(name, args);
}

function formatToolCallArgs(name: string, args: Record<string, unknown>): ToolCallData {
  switch (name) {
    case "read_file": {
      let s = str(args.file_path);
      if (args.offset) s += `, offset = ${args.offset}`;
      return { name, summary: s, detail: null };
    }
    case "write_file":
      return { name, summary: str(args.file_path), detail: str(args.content) || null };
    case "shell_command":
      return { name, summary: str(args.command), detail: null };
    case "glob": {
      let s = str(args.pattern);
      if (args.base_dir) s += `, base_dir = ${args.base_dir}`;
      return { name, summary: s, detail: null };
    }
    case "grep": {
      let s = str(args.pattern);
      if (args.include) s += `, include = ${args.include}`;
      return { name, summary: s, detail: null };
    }
    case "ls":
      return { name, summary: str(args.path ?? "."), detail: null };
    case "edit": {
      let s = str(args.file_path);
      if (args.replace_all) s += ", replace_all = true";
      const diff = diffLines(str(args.old_string), str(args.new_string));
      return { name, summary: s, detail: diff || null };
    }
    case "multiedit": {
      const edits = Array.isArray(args.edits) ? args.edits : [];
      let detail = "";
      for (const edit of edits) {
        const d = diffLines(str((edit as Record<string, unknown>).old_string), str((edit as Record<string, unknown>).new_string));
        if (d) detail += (detail ? "\n" : "") + d;
      }
      return { name, summary: str(args.file_path), detail: detail || null };
    }
    case "apply_patch": {
      const patch = str(args.patch);
      const file = patch.split("\n").find(l => l.startsWith("+++ b/") || l.startsWith("--- a/"))?.replace(/^(\+\+\+ b\/|--- a\/)/, "") ?? "?";
      return { name, summary: file, detail: patch || null };
    }
    case "batch": {
      const calls = Array.isArray(args.calls) ? args.calls : [];
      const summaries = calls.map((c: unknown) => {
        const call = c as Record<string, unknown>;
        const toolArgs = call.parameters;
        const argsStr = toolArgs ? JSON.stringify(toolArgs) : "";
        return `${call.tool}(${argsStr})`;
      });
      return { name, summary: `${calls.length} tool calls`, detail: summaries.join("\n") || null };
    }
    case "task": {
      let s = str(args.description);
      if (args.subagent_type) s += `, subagent_type = ${args.subagent_type}`;
      return { name, summary: s, detail: str(args.prompt) || null };
    }
    case "web_search":
      return { name, summary: str(args.query), detail: null };
    case "web_fetch":
      return { name, summary: str(args.url), detail: null };
    case "code_search": {
      let s = str(args.query);
      if (args.tokens_num) s += `, token = ${args.tokens_num}`;
      return { name, summary: s, detail: null };
    }
    case "question":
      return { name, summary: str(args.question), detail: null };
    case "skill":
      return { name, summary: str(args.name), detail: null };
    case "lsp": {
      let s = str(args.operation);
      if (args.file_path) s += ` ${args.file_path}`;
      if (args.line) s += `:${args.line}`;
      if (args.character) s += `:${args.character}`;
      if (args.query) s += ` query = ${args.query}`;
      return { name, summary: s, detail: null };
    }
    default:
      return { name, summary: "", detail: JSON.stringify(args, null, 2) };
  }
}

function str(v: unknown): string {
  if (typeof v === "string") return v;
  if (v == null) return "";
  return String(v);
}

function diffLines(oldStr: string, newStr: string): string {
  const old = oldStr.split("\n").map(l => `- ${l}`);
  const neu = newStr.split("\n").map(l => `+ ${l}`);
  return [...old, ...neu].join("\n");
}
