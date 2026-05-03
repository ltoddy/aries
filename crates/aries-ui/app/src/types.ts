export type ProjectEntry = {
  name: string;
  path: string;
  branch: string | null;
};

export type ChatBlock = {
  type: "text" | "reasoning" | "tool-call" | "tool-result";
  content: string;
};

export type ChatMessage = {
  role: "assistant" | "user";
  content: string;
  blocks?: ChatBlock[];
  usage?: TokenUsage;
};

export type SessionBootstrap = {
  appName: string;
  provider: string;
  model: string;
  sessionId: string;
  messages: ChatMessage[];
};

export type ChatResponse = {
  sessionId: string;
  message: ChatMessage;
};

export type ChatStreamPayload = {
  seq: number;
  sessionId: string;
  kind: "text" | "reasoning" | "tool-call" | "tool-result" | "usage";
  delta: string;
};

export type TokenUsage = {
  total_tokens: number;
  input_tokens: number;
  output_tokens: number;
  cached_input_tokens: number;
  elapsed_ms: number;
};

export type ToolCallData = {
  name: string;
  summary: string;
  detail: string | null;
};

export type ThemeMode = "light" | "dark" | "system";
