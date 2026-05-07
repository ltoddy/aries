export type ProjectEntry = {
  id: string;
  name: string;
  path: string;
  branch: string | null;
};

export type SessionSummary = {
  id: string;
  sessionId: string;
  title: string | null;
  projectDir: string;
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
  sessionDirName: string;
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

export type ConfigProvider = "deepseek-v4" | "openai-compatible" | "azure";

export type ConfigFormData = {
  provider: ConfigProvider;
  apiKey: string;
  model: string;
  baseUrl?: string | null;
  azureEndpoint?: string | null;
  apiVersion?: string | null;
};
