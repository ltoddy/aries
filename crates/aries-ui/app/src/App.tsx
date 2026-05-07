import { FormEvent, KeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { EmojiPicker } from "frimousse";
import { ArrowLeft, Bot, FileText, MessageSquare, Monitor, Moon, PanelLeftClose, PanelLeftOpen, Plus, Send, Smile, Square, Sun, Trash2, User } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { WelcomePage } from "@/components/WelcomePage";

import type { ChatMessage, ChatResponse, ChatStreamPayload, ProjectEntry, SessionBootstrap, SessionSummary, ThemeMode } from "./types";
import { CHAT_STREAM_EVENT, THEME_STORAGE_KEY } from "./constants";
import { appendStreamBlock, findLastAssistantIndex, getPreferredTheme, resolveTheme } from "./utils";
import { Markdown, renderMessage } from "./components/MessageBlocks";

function App() {
  const [theme, setTheme] = useState<ThemeMode>(() => getPreferredTheme());
  const [project, setProject] = useState<ProjectEntry | null>(null);

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

  if (!project) {
    return <WelcomePage onSelect={setProject} />;
  }

  return <ChatView theme={theme} setTheme={setTheme} project={project} onBack={() => setProject(null)} />;
}

function ChatView({
  theme,
  setTheme,
  project,
  onBack,
}: {
  theme: ThemeMode;
  setTheme: (t: ThemeMode | ((prev: ThemeMode) => ThemeMode)) => void;
  project: ProjectEntry;
  onBack: () => void;
}) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [sessionDirName, setSessionDirName] = useState<string | null>(null);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [activeRowId, setActiveRowId] = useState<string | null>(null);
  const [systemPrompt, setSystemPrompt] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"chat" | "system-prompt">("chat");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const emojiPickerRef = useRef<HTMLDivElement | null>(null);
  const emojiButtonRef = useRef<HTMLButtonElement | null>(null);
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

  function openSystemPromptTab() {
    fetchSystemPrompt();
    setActiveTab("system-prompt");
  }

  async function handleClearHistory() {
    try {
      await invoke("clear_history");
      setMessages([]);
    } catch {}
  }

  async function refreshSessions() {
    try {
      const list = await invoke<SessionSummary[]>("list_sessions");
      setSessions(list);
    } catch {}
  }

  async function loadSession(sid?: string, rowId?: string | null) {
    setLoading(true);
    setError(null);
    lastProcessedSeqRef.current = 0;
    try {
      const data = await invoke<SessionBootstrap>("bootstrap_chat", { sessionId: sid ?? null });
      setMessages(data.messages);
      setSessionId(data.sessionId);
      setSessionDirName(data.sessionDirName);
      setActiveRowId(rowId ?? null);
      await refreshSessions();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleSelectSession(summary: SessionSummary) {
    if (summary.id === activeRowId) return;
    await loadSession(summary.sessionId, summary.id);
  }

  async function handleNewSession() {
    await loadSession(undefined, null);
  }

  useEffect(() => {
    activeSessionIdRef.current = sessionId;
  }, [sessionId]);

  const pendingPayloadsRef = useRef<ChatStreamPayload[]>([]);
  const rafIdRef = useRef<number | null>(null);

  const flushPayloads = useCallback(() => {
    rafIdRef.current = null;
    const payloads = pendingPayloadsRef.current;
    if (payloads.length === 0) return;
    pendingPayloadsRef.current = [];
    setMessages((current) => {
      const targetIndex = findLastAssistantIndex(current);
      if (targetIndex === null) return current;
      let updated = current[targetIndex];
      for (const payload of payloads) {
        updated = appendStreamBlock(updated, payload);
      }
      const next = [...current];
      next[targetIndex] = updated;
      return next;
    });
  }, []);

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
        pendingPayloadsRef.current.push(payload);
        if (rafIdRef.current === null) {
          rafIdRef.current = requestAnimationFrame(flushPayloads);
        }
      });
      if (active) { unlisten = fn; } else { fn(); }
    })();
    return () => {
      active = false;
      if (unlisten) unlisten();
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
    };
  }, [flushPayloads]);

  useEffect(() => {
    invoke("resize_window_for_chat").catch(() => {});
    refreshSessions();
    setLoading(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: sending ? "auto" : "smooth", block: "end" });
  }, [messages, sending]);

  useEffect(() => {
    if (!showEmojiPicker) return;
    function handleClick(e: MouseEvent) {
      if (
        emojiPickerRef.current && !emojiPickerRef.current.contains(e.target as Node) &&
        emojiButtonRef.current && !emojiButtonRef.current.contains(e.target as Node)
      ) {
        setShowEmojiPicker(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [showEmojiPicker]);

  const canSend = useMemo(() => prompt.trim().length > 0 && !sending && !!sessionId, [prompt, sending, sessionId]);
  const lastAssistantIndex = useMemo(() => findLastAssistantIndex(messages), [messages]);

  async function submitPrompt(content: string) {
    if (isSubmittingRef.current) return;
    if (!sessionId) return;
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

  function handlePromptKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      const content = prompt.trim();
      if (!content || sending) return;
      submitPrompt(content);
    }
  }

  return (
    <div className="flex h-screen bg-background">
      {/* Icon sidebar */}
      <aside className="flex w-12 shrink-0 flex-col items-center justify-between border-r py-2">
        <div className="flex flex-col items-center gap-1">
          <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onBack} title="Back to projects">
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => setSidebarOpen((o) => !o)} title="Toggle sidebar">
            {sidebarOpen ? <PanelLeftClose className="h-4 w-4" /> : <PanelLeftOpen className="h-4 w-4" />}
          </Button>
        </div>
        <div className="flex flex-col items-center gap-1">
          <Button
            variant={activeTab === "chat" ? "secondary" : "ghost"}
            size="icon"
            className="h-8 w-8"
            onClick={() => setActiveTab("chat")}
            title="Chat"
          >
            <MessageSquare className="h-4 w-4" />
          </Button>
          <Button
            variant={activeTab === "system-prompt" ? "secondary" : "ghost"}
            size="icon"
            className="h-8 w-8"
            onClick={openSystemPromptTab}
            title="System Prompt"
          >
            <FileText className="h-4 w-4" />
          </Button>
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

      {/* Left navigation panel */}
      {sidebarOpen && (
        <aside className="flex w-56 shrink-0 flex-col border-r bg-muted/30">
          <div className="flex items-center justify-between px-3 py-2">
            <div className="text-xs font-medium text-muted-foreground uppercase tracking-wide">Sessions</div>
            <Button variant="ghost" size="icon" className="h-6 w-6" onClick={handleNewSession} title="New session">
              <Plus className="h-3.5 w-3.5" />
            </Button>
          </div>
          <div className="px-3 py-1">
            <div className="rounded-md bg-muted/50 px-2.5 py-2 text-xs">
              <div className="font-medium truncate" title={project.path}>{project.name}</div>
              <div className="mt-0.5 truncate text-[11px] text-muted-foreground" title={project.path}>
                {project.path.replace(/^\/Users\/[^/]+/, "~")}
              </div>
            </div>
          </div>
          <div className="flex-1 overflow-y-auto px-2 py-1">
            {sessions.length === 0 && (
              <div className="px-2 py-4 text-center text-xs text-muted-foreground">No sessions yet.</div>
            )}
            {sessions.map((s) => {
              const isActive = s.id === activeRowId;
              const label = s.title?.trim() ? s.title : s.sessionId;
              return (
                <button
                  key={s.id}
                  onClick={() => handleSelectSession(s)}
                  className={`mb-0.5 flex w-full flex-col items-start rounded-md px-2 py-1.5 text-left text-xs transition-colors ${
                    isActive ? "bg-accent text-accent-foreground" : "hover:bg-accent/60"
                  }`}
                >
                  <span className="truncate w-full font-medium" title={label}>{label}</span>
                  <span className="truncate w-full font-mono text-[10px] text-muted-foreground" title={s.sessionId}>
                    {s.sessionId}
                  </span>
                </button>
              );
            })}
          </div>
          {sessionDirName && (
            <div className="border-t px-3 py-1.5 font-mono text-[10px] text-muted-foreground truncate" title={sessionDirName}>
              {sessionDirName}
            </div>
          )}
        </aside>
      )}

      {/* Main content */}
      {activeTab === "chat" ? (
        <div className="flex flex-1 flex-col">
          {/* Messages */}
          <div className="flex-1 overflow-y-auto">
            <div className="mx-auto max-w-3xl px-6 py-3">
              {!loading && !sessionId && (
                <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 text-center">
                  <div className="flex h-12 w-12 items-center justify-center rounded-full bg-muted">
                    <Bot className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <div>
                    <h2 className="text-lg font-semibold">No session selected</h2>
                    <p className="mt-1 text-sm text-muted-foreground">Pick a session from the sidebar, or click + to start a new one.</p>
                  </div>
                </div>
              )}
              {!loading && sessionId && messages.length === 0 && (
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
          <div className="shrink-0 border-t bg-background px-6 py-2">
            <form className="mx-auto max-w-3xl relative" onSubmit={handleSubmit}>
              {showEmojiPicker && (
                <div ref={emojiPickerRef} className="absolute bottom-12 right-0 z-50">
                  <EmojiPicker.Root
                    className="isolate flex h-[368px] w-[352px] flex-col rounded-lg border bg-background shadow-lg"
                    onEmojiSelect={({ emoji }) => {
                      setPrompt((p) => p + emoji);
                      setShowEmojiPicker(false);
                    }}
                  >
                    <EmojiPicker.Search className="mx-2 mt-2 appearance-none rounded-md border bg-muted px-2.5 py-2 text-sm outline-none" />
                    <EmojiPicker.Viewport className="relative flex-1 outline-hidden">
                      <EmojiPicker.Loading className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
                        Loading…
                      </EmojiPicker.Loading>
                      <EmojiPicker.Empty className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
                        No emoji found.
                      </EmojiPicker.Empty>
                      <EmojiPicker.List
                        className="select-none pb-1.5"
                        components={{
                          CategoryHeader: ({ category, ...props }) => (
                            <div className="bg-background px-3 pt-3 pb-1.5 text-xs font-medium text-muted-foreground" {...props}>
                              {category.label}
                            </div>
                          ),
                          Row: ({ children, ...props }) => (
                            <div className="scroll-my-1.5 px-1.5" {...props}>
                              {children}
                            </div>
                          ),
                          Emoji: ({ emoji, ...props }) => (
                            <button className="flex size-8 items-center justify-center rounded-md text-lg data-[active]:bg-muted" {...props}>
                              {emoji.emoji}
                            </button>
                          ),
                        }}
                      />
                    </EmojiPicker.Viewport>
                  </EmojiPicker.Root>
                </div>
              )}
              <div className="flex items-center rounded-md border bg-background px-3">
                <input
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  onKeyDown={handlePromptKeyDown}
                  className="flex-1 h-10 bg-transparent text-sm outline-none"
                />
                <div className="flex items-center gap-0.5">
                  <Button type="button" variant="ghost" size="icon" className="h-7 w-7" onClick={handleClearHistory} title="Clear History">
                    <Trash2 className="h-4 w-4" />
                  </Button>
                  <Button ref={emojiButtonRef} type="button" variant="ghost" size="icon" className="h-7 w-7" onClick={() => setShowEmojiPicker((v) => !v)} title="Emoji">
                    <Smile className="h-4 w-4" />
                  </Button>
                  <Button type="submit" variant="ghost" size="icon" disabled={!canSend} className="h-7 w-7">
                    {sending ? <Square className="h-3.5 w-3.5 fill-current" /> : <Send className="h-4 w-4" />}
                  </Button>
                </div>
              </div>
            </form>
            {error && <p className="mx-auto max-w-3xl mt-1 text-sm text-destructive">{error}</p>}
          </div>
        </div>
      ) : (
        <div className="flex flex-1 flex-col overflow-y-auto">
          <div className="mx-auto w-full max-w-3xl px-6 py-4">
            <h1 className="mb-4 text-lg font-semibold">System Prompt</h1>
            {systemPrompt ? (
              <div className="prose prose-sm dark:prose-invert max-w-none text-sm leading-relaxed">
                <Markdown content={systemPrompt} />
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">Loading...</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
