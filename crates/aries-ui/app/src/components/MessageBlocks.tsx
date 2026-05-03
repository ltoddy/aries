import { useState } from "react";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import { ChevronRight, Wrench } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Separator } from "@/components/ui/separator";
import type { ChatMessage } from "../types";
import { blockLabel, formatToolResult, parseToolCall } from "../utils";

function Markdown({ content }: { content: string }) {
  return (
    <ReactMarkdown
      rehypePlugins={[rehypeHighlight]}
      components={{
        pre({ children }) {
          return <pre className="overflow-x-auto rounded-md bg-muted p-2 text-[13px] leading-relaxed">{children}</pre>;
        },
        code({ className, children, ...props }) {
          const isInline = !className;
          if (isInline) {
            return <code className="rounded bg-muted px-1 py-0.5 text-[13px]" {...props}>{children}</code>;
          }
          return <code className={className} {...props}>{children}</code>;
        },
        p({ children }) {
          return <p className="my-1">{children}</p>;
        },
        ul({ children }) {
          return <ul className="my-1 list-disc pl-5">{children}</ul>;
        },
        ol({ children }) {
          return <ol className="my-1 list-decimal pl-5">{children}</ol>;
        },
        li({ children }) {
          return <li className="my-0.5">{children}</li>;
        },
        h1({ children }) {
          return <h1 className="my-2 text-lg font-bold">{children}</h1>;
        },
        h2({ children }) {
          return <h2 className="my-2 text-base font-bold">{children}</h2>;
        },
        h3({ children }) {
          return <h3 className="my-1.5 text-sm font-bold">{children}</h3>;
        },
        blockquote({ children }) {
          return <blockquote className="my-1 border-l-2 border-muted-foreground/30 pl-3 text-muted-foreground">{children}</blockquote>;
        },
        a({ href, children }) {
          return <a href={href} className="text-primary underline" target="_blank" rel="noopener noreferrer">{children}</a>;
        },
        hr() {
          return <hr className="my-2 border-border" />;
        },
        table({ children }) {
          return <div className="my-1 overflow-x-auto"><table className="w-full text-sm">{children}</table></div>;
        },
        th({ children }) {
          return <th className="border px-2 py-1 text-left font-medium">{children}</th>;
        },
        td({ children }) {
          return <td className="border px-2 py-1">{children}</td>;
        },
      }}
    >
      {content}
    </ReactMarkdown>
  );
}

export function ReasoningBlock({ content, isStreaming }: { content: string; isStreaming: boolean }) {
  const [userToggled, setUserToggled] = useState<boolean | null>(null);
  const open = userToggled ?? isStreaming;
  return (
    <Collapsible open={open} onOpenChange={(value) => setUserToggled(value)} className="overflow-hidden rounded-md border bg-muted/50">
      <CollapsibleTrigger className="flex h-7 w-full items-center justify-between px-2 text-left text-xs text-muted-foreground hover:bg-accent">
        <div className="flex items-center gap-1.5">
          <Badge variant="secondary" className="text-[10px] font-medium uppercase">
            {blockLabel("reasoning")}
          </Badge>
          <span>Chain-of-thought</span>
        </div>
        <ChevronRight className={`h-3.5 w-3.5 transition-transform ${open ? "rotate-90" : "rotate-0"}`} />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <Separator />
        <div className="whitespace-pre-wrap px-2 py-1.5 font-mono text-[11px] leading-relaxed text-muted-foreground">{content}</div>
      </CollapsibleContent>
    </Collapsible>
  );
}

function ToolCallPairBlock({ callContent, resultContent }: { callContent: string; resultContent: string | null }) {
  const parsed = parseToolCall(callContent);
  const [open, setOpen] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
  const hasResult = resultContent !== null;
  const formattedResult = hasResult ? formatToolResult(parsed.name, resultContent!) : "";
  return (
    <div className="overflow-hidden rounded-md border">
      <div className="flex items-center gap-1.5 px-2 py-1 text-xs">
        <Wrench className="h-3 w-3 text-muted-foreground" />
        <span className="font-medium">{parsed.name}</span>
        {parsed.summary && <span className="truncate text-muted-foreground">{parsed.summary}</span>}
      </div>
      {parsed.detail && (
        <Collapsible open={detailOpen} onOpenChange={setDetailOpen} className="border-t">
          <CollapsibleTrigger className="flex h-6 w-full items-center justify-between px-2 text-left text-[11px] text-muted-foreground hover:bg-accent">
            <span>Detail</span>
            <ChevronRight className={`h-3 w-3 transition-transform ${detailOpen ? "rotate-90" : "rotate-0"}`} />
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="border-t px-2 py-1.5">
              <pre className="overflow-x-auto whitespace-pre-wrap rounded bg-muted px-2 py-1 font-mono text-[11px] leading-relaxed">
                {parsed.detail}
              </pre>
            </div>
          </CollapsibleContent>
        </Collapsible>
      )}
      {hasResult && (
        <Collapsible open={open} onOpenChange={setOpen} className="border-t">
          <CollapsibleTrigger className="flex h-6 w-full items-center justify-between px-2 text-left text-[11px] text-muted-foreground hover:bg-accent">
            <span>Result</span>
            <ChevronRight className={`h-3 w-3 transition-transform ${open ? "rotate-90" : "rotate-0"}`} />
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="border-t px-2 py-1.5">
              <pre className="overflow-x-auto whitespace-pre-wrap rounded bg-muted px-2 py-1 font-mono text-[11px] leading-relaxed">
                {formattedResult}
              </pre>
            </div>
          </CollapsibleContent>
        </Collapsible>
      )}
    </div>
  );
}

export function renderMessage(message: ChatMessage, isStreaming: boolean) {
  if (message.role === "user") {
    return (
      <div className="whitespace-pre-wrap text-sm leading-relaxed">
        {message.content}
      </div>
    );
  }
  const blocks = message.blocks?.length ? message.blocks : [{ type: "text" as const, content: message.content }];

  const elements: React.ReactNode[] = [];
  let i = 0;
  while (i < blocks.length) {
    const block = blocks[i];
    if (block.type === "tool-call") {
      const nextBlock = blocks[i + 1];
      const resultContent = nextBlock && nextBlock.type === "tool-result" ? nextBlock.content : null;
      elements.push(<ToolCallPairBlock key={i} callContent={block.content} resultContent={resultContent} />);
      if (resultContent !== null) i += 2; else i += 1;
    } else if (block.type === "text") {
      elements.push(
        <div key={i} className="text-sm leading-relaxed">
          <Markdown content={block.content} />
          {isStreaming && i === blocks.length - 1 && <span className="ml-1 inline-block h-4 w-1.5 animate-pulse rounded-sm bg-foreground/50 align-middle" />}
        </div>
      );
      i += 1;
    } else if (block.type === "reasoning") {
      elements.push(<ReasoningBlock key={i} content={block.content} isStreaming={isStreaming} />);
      i += 1;
    } else {
      elements.push(<ToolCallPairBlock key={i} callContent={`[Tool] unknown\n{}`} resultContent={block.content} />);
      i += 1;
    }
  }

  const usage = message.usage;

  return (
    <div className="space-y-2">
      {elements}
      {usage && (
        <div className="flex items-center gap-2 pt-3 text-[11px] text-muted-foreground/70">
          <div className="h-px flex-1 bg-border" />
          <span className="shrink-0">
            tokens: {usage.total_tokens.toLocaleString()} (in {usage.input_tokens.toLocaleString()}{usage.cached_input_tokens > 0 ? `, cached ${usage.cached_input_tokens.toLocaleString()}` : ""}, out {usage.output_tokens.toLocaleString()})
            {" · "}
            {usage.elapsed_ms >= 1000
              ? `${(usage.elapsed_ms / 1000).toFixed(2)}s`
              : `${usage.elapsed_ms}ms`}
          </span>
          <div className="h-px flex-1 bg-border" />
        </div>
      )}
    </div>
  );
}
