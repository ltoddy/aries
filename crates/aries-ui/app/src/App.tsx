import { FormEvent, KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, Bot, FileText, Monitor, Moon, Send, Sun, User } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { WelcomePage } from "@/components/WelcomePage";

import type { ChatMessage, ChatResponse, ChatStreamPayload, SessionBootstrap, ThemeMode } from "./types";
import { CHAT_STREAM_EVENT, THEME_STORAGE_KEY } from "./constants";
import { appendStreamBlock, findLastAssistantIndex, getPreferredTheme, resolveTheme } from "./utils";
import { renderMessage } from "./components/MessageBlocks";

function App() {
  const [theme, setTheme] = useState<ThemeMode>(() => getPreferredTheme());
  const [projectPath, setProjectPath] = useState<string | null>(null);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", resolveTheme(theme) === "dark");
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const listener = () => {
      if (theme === "system") {
        document.documentElement.classList.toggle("dark", media.matches);
      }
    };
    media.addEventListener("change", listener);
    return () => media.removeEventListener("change", listener);
  }, [theme]);

  if (!projectPath) {
    return <WelcomePage onSelect={setProjectPath} />;
  }

  return <ChatView theme={theme} setTheme={setTheme} projectPath={projectPath} onBack={() => setProjectPath(null)} />;
}

function ChatView({
  theme,
  setTheme,
  projectPath,
  onBack,
}: {
  theme: ThemeMode;
  setTheme: (t: ThemeMode | ((prev: ThemeMode) => ThemeMode)) => void;
  projectPath: string;
  onBack: () => void;
}) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [systemPrompt, setSystemPrompt] = useState<string | null>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const isSubmittingRef = useRef(false);
  const lastProcessedSeqRef = useRef(0);
  const messagesEndRef = useRef<HTMLDivElement | null>(null);

  async function fetchSystemPrompt() {
    try {
      const sp = await invoke<string>("get_system_prompt");
      setSystemPrompt(sp);
    } catch {}
  }

  useEffect(() => {
    activeSessionIdRef.current = sessionId;
  }, [sessionId]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    (async () => {
      const fn = await listen<ChatStreamPayload>(CHAT_STREAM_EVENT, (event) => {
        if (!active) return;
        const payload = event.payload;
        if (!payload || payload.sessionId !== activeSessionIdRef.current) return;
        if (payload.seq <= lastProcessedSeqRef.current) return;
        lastProcessedSeqRef.current = payload.seq;
        setMessages((current) => {
          const targetIndex = findLastAssistantIndex(current);
          if (targetIndex === null) return current;
          const next = [...current];
          next[targetIndex] = appendStreamBlock(next[targetIndex], payload);
          return next;
        });
      });
      if (active) { unlisten = fn; } else { fn(); }
    })();
    return () => { active = false; if (unlisten) unlisten(); };
  }, []);

  useEffect(() => {
    invoke("resize_window_for_chat").catch(() => {});
    invoke<SessionBootstrap>("bootstrap_chat", { projectPath })
      .then((data) => { setMessages(data.messages); setSessionId(data.sessionId); })
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: sending ? "auto" : "smooth", block: "end" });
  }, [messages, sending]);

  const canSend = useMemo(() => prompt.trim().length > 0 && !sending, [prompt, sending]);
  const lastAssistantIndex = useMemo(() => findLastAssistantIndex(messages), [messages]);

  async function submitPrompt(content: string) {
    if (isSubmittingRef.current) return;
    isSubmittingRef.current = true;
    lastProcessedSeqRef.current = 0;
    const nextUserMessage: ChatMessage = { role: "user", content };
    const placeholder: ChatMessage = { role: "assistant", content: "", blocks: [] };
    setMessages((current) => {
      const last = current[current.length - 1];
      if (last && last.role === "assistant" && last.content === "" && (last.blocks?.length ?? 0) === 0) {
        return current;
      }
      return [...current, nextUserMessage, placeholder];
    });
    setPrompt(""); setSending(true); setError(null);
    try {
      const response = await invoke<ChatResponse>("send_chat_message", { request: { prompt: content } });
      setMessages((current) => {
        const targetIndex = findLastAssistantIndex(current);
        if (targetIndex !== null && current[targetIndex]) {
          const existing = current[targetIndex];
          const existingBlocks = existing.blocks ?? [];
          const next = [...current];
          if (existingBlocks.length === 0) {
            next[targetIndex] = { ...existing, ...response.message, blocks: [{ type: "text", content: response.message.content }] };
          } else {
            next[targetIndex] = { ...existing, role: response.message.role };
          }
          return next;
        }
        return current;
      });
    } catch (err) {
      const message = String(err);
      setError(message);
      setMessages((current) => {
        const targetIndex = findLastAssistantIndex(current);
        if (targetIndex !== null && current[targetIndex]) {
          const next = [...current];
          next[targetIndex] = { role: "assistant", content: `Failed to send message: ${message}`, blocks: [{ type: "tool-result", content: `Failed to send message: ${message}` }] };
          return next;
        }
        return current;
      });
    } finally {
      isSubmittingRef.current = false; setSending(false);
    }
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const content = prompt.trim();
    if (!content || sending) return;
    submitPrompt(content);
  }

  function handlePromptKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && event.metaKey) {
      // Cmd+Enter inserts newline
      event.preventDefault();
      setPrompt((p) => p + "\n");
      return;
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      const content = prompt.trim();
      if (!content || sending) return;
      submitPrompt(content);
    }
  }

  return (
    <div className="flex h-screen bg-background">
      {/* Left sidebar */}
      <aside className="flex w-12 shrink-0 flex-col items-center justify-between border-r py-2">
        <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onBack} title="Back to projects">
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex flex-col items-center gap-1">
          <Dialog>
            <DialogTrigger asChild>
              <Button variant="ghost" size="icon" className="h-8 w-8" onClick={fetchSystemPrompt} title="System Prompt">
                <FileText className="h-4 w-4" />
              </Button>
            </DialogTrigger>
            <DialogContent className="max-h-[80vh] max-w-2xl overflow-hidden">
              <DialogHeader>
                <DialogTitle>System Prompt</DialogTitle>
              </DialogHeader>
              <div className="overflow-y-auto max-h-[65vh]">
                <pre className="whitespace-pre-wrap text-xs leading-relaxed">{systemPrompt ?? "Loading..."}</pre>
              </div>
            </DialogContent>
          </Dialog>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={() => setTheme((c) => c === "system" ? "light" : c === "light" ? "dark" : "system")}
            title={theme === "system" ? "System" : theme === "dark" ? "Dark" : "Light"}
          >
            {theme === "system" ? <Monitor className="h-4 w-4" /> : theme === "dark" ? <Moon className="h-4 w-4" /> : <Sun className="h-4 w-4" />}
          </Button>
        </div>
      </aside>

      {/* Main content */}
      <div className="flex flex-1 flex-col">
        {/* Messages */}
        <div className="flex-1 overflow-y-auto">
          <div className="mx-auto max-w-3xl px-6 py-3">
            {!loading && messages.length === 0 && (
              <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 text-center">
                <div className="flex h-12 w-12 items-center justify-center rounded-full bg-muted">
                  <Bot className="h-6 w-6 text-muted-foreground" />
                </div>
                <div>
                  <h2 className="text-lg font-semibold">Start a conversation</h2>
                  <p className="mt-1 text-sm text-muted-foreground">Ask about the current project, inspect files, run tools, or iterate on code.</p>
                </div>
              </div>
            )}

            <div className="flex flex-col gap-3">
              {messages.map((message, index) => {
                const isStreamingMessage = sending && message.role === "assistant" && index === lastAssistantIndex;
                const isUser = message.role === "user";
                if (isUser) {
                  return (
                    <div key={`${message.role}-${index}`} className="flex items-start gap-2 flex-row-reverse">
                      <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
                        <User className="h-3 w-3" />
                      </div>
                      <Card className="max-w-[85%] shadow-sm bg-primary text-primary-foreground">
                        <CardContent className="px-2.5 py-1.5">{renderMessage(message, false)}</CardContent>
                      </Card>
                    </div>
                  );
                }
                return (
                  <div key={`${message.role}-${index}`} className="w-full">
                    {renderMessage(message, isStreamingMessage)}
                  </div>
                );
              })}
              <div ref={messagesEndRef} />
            </div>
          </div>
        </div>

        {/* Input */}
        <div className="shrink-0 border-t bg-background py-2">
          <form className="mx-auto flex max-w-3xl flex-col gap-1.5 px-6" onSubmit={handleSubmit}>
            <div className="relative">
              <Textarea
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                onKeyDown={handlePromptKeyDown}
                placeholder="Ask Aries about the current project..."
                rows={3}
                className="resize-none pr-12"
              />
              <Button
                type="submit"
                size="icon"
                disabled={!canSend}
                className="absolute bottom-2 right-2 h-8 w-8"
              >
                <Send className="h-4 w-4" />
              </Button>
            </div>
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>{sending ? "Streaming..." : "Enter to send, ⌘+Enter for newline"}</span>
              {sessionId && <span className="font-mono text-[10px]">{sessionId}</span>}
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
          </form>
        </div>
      </div>
    </div>
  );
}

export default App;
