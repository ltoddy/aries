export type ChatBlock = {
  type: "text" | "reasoning" | "tool-call" | "tool-result";
  content: string;
};

export type ChatMessage = {
  role: "assistant" | "user";
  content: string;
  blocks?: ChatBlock[];
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
  kind: "text" | "reasoning" | "tool-call" | "tool-result";
  delta: string;
};

export type ToolCallData = {
  name: string;
  summary: string;
  detail: string | null;
};

export type ThemeMode = "light" | "dark" | "system";
