import { FormEvent, KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Bot, Monitor, Moon, Send, Sun, User } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";

import type { ChatMessage, ChatResponse, ChatStreamPayload, SessionBootstrap, ThemeMode } from "./types";
import { CHAT_STREAM_EVENT, THEME_STORAGE_KEY } from "./constants";
import { appendStreamBlock, findLastAssistantIndex, getPreferredTheme, resolveTheme } from "./utils";
import { renderMessage } from "./components/MessageBlocks";

function App() {
  const [theme, setTheme] = useState<ThemeMode>(() => getPreferredTheme());
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const isSubmittingRef = useRef(false);
  const lastProcessedSeqRef = useRef(0);
  const messagesEndRef = useRef<HTMLDivElement | null>(null);

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
    invoke<SessionBootstrap>("bootstrap_chat")
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
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      const content = prompt.trim();
      if (!content || sending) return;
      submitPrompt(content);
    }
  }

  return (
    <div className="flex h-screen flex-col bg-background">
      {/* Header */}
      <header className="flex h-9 shrink-0 items-center justify-between border-b px-3">
        <div className="flex items-center gap-1.5">
          <Bot className="h-4 w-4" />
          <h1 className="text-sm font-semibold">Aries</h1>
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setTheme((c) => c === "system" ? "light" : c === "light" ? "dark" : "system")}
          title={theme === "system" ? "System" : theme === "dark" ? "Dark" : "Light"}
        >
          {theme === "system" ? <Monitor className="h-4 w-4" /> : theme === "dark" ? <Moon className="h-4 w-4" /> : <Sun className="h-4 w-4" />}
        </Button>
      </header>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto">
        <div className="px-3 py-3">
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

          <div className="flex flex-col gap-2">
            {messages.map((message, index) => {
              const isStreamingMessage = sending && message.role === "assistant" && index === lastAssistantIndex;
              const isUser = message.role === "user";
              return (
                <div key={`${message.role}-${index}`} className={`flex items-start gap-2 ${isUser ? "flex-row-reverse" : ""}`}>
                  <div className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full ${isUser ? "bg-primary text-primary-foreground" : "bg-muted"}`}>
                    {isUser ? <User className="h-3 w-3" /> : <Bot className="h-3 w-3" />}
                  </div>
                  <Card className={`max-w-[85%] shadow-sm ${isUser ? "bg-primary text-primary-foreground" : ""}`}>
                    <CardContent className="px-2.5 py-1.5">{renderMessage(message, isStreamingMessage)}</CardContent>
                  </Card>
                </div>
              );
            })}
            <div ref={messagesEndRef} />
          </div>
        </div>
      </div>

      {/* Input */}
      <div className="shrink-0 border-t bg-background px-3 py-2">
        <form className="flex flex-col gap-1.5" onSubmit={handleSubmit}>
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
            <span>{sending ? "Streaming..." : "Enter to send, Shift+Enter for newline"}</span>
            {sessionId && <span className="font-mono text-[10px]">{sessionId}</span>}
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
        </form>
      </div>
    </div>
  );
}

export default App;
