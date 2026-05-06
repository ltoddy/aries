import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, GitBranch, Search, Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { ConfigFormData, ConfigProvider, ProjectEntry } from "../types";

const COLORS = [
  "bg-blue-500",
  "bg-green-500",
  "bg-amber-500",
  "bg-purple-500",
  "bg-rose-500",
  "bg-cyan-500",
  "bg-indigo-500",
  "bg-orange-500",
];

const DEFAULT_CONFIG: ConfigFormData = {
  provider: "deepseek-v4",
  apiKey: "",
  model: "deepseek-chat",
  baseUrl: "",
  azureEndpoint: "",
  apiVersion: "",
};

function getInitials(name: string): string {
  const parts = name.split(/[-_\s]+/);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
  return name.slice(0, 2).toUpperCase();
}

function getColor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return COLORS[Math.abs(hash) % COLORS.length];
}

function providerLabel(provider: ConfigProvider): string {
  switch (provider) {
    case "deepseek-v4":
      return "DeepSeek V4";
    case "openai-compatible":
      return "OpenAI Compatible";
    case "azure":
      return "Azure";
  }
}

export function WelcomePage({ onSelect }: { onSelect: (project: ProjectEntry) => void }) {
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [configOpen, setConfigOpen] = useState(false);
  const [config, setConfig] = useState<ConfigFormData>(DEFAULT_CONFIG);
  const [configLoading, setConfigLoading] = useState(true);
  const [configSaving, setConfigSaving] = useState(false);
  const [configError, setConfigError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ProjectEntry[]>("list_projects").then(setProjects).catch(() => {});
  }, []);

  useEffect(() => {
    invoke<ConfigFormData | null>("get_config")
      .then((saved) => {
        if (saved) {
          setConfig({
            provider: saved.provider,
            apiKey: saved.apiKey,
            model: saved.model,
            baseUrl: saved.baseUrl ?? "",
            azureEndpoint: saved.azureEndpoint ?? "",
            apiVersion: saved.apiVersion ?? "",
          });
        }
      })
      .catch(() => {})
      .finally(() => setConfigLoading(false));
  }, []);

  const filtered = useMemo(
    () =>
      projects.filter(
        (p) =>
          p.name.toLowerCase().includes(filter.toLowerCase()) ||
          p.path.toLowerCase().includes(filter.toLowerCase())
      ),
    [projects, filter]
  );

  async function handleOpen() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      const project = await invoke<ProjectEntry>("activate_project", { path: selected });
      onSelect(project);
    }
  }

  async function handleSelectProject(path: string) {
    const project = await invoke<ProjectEntry>("activate_project", { path });
    onSelect(project);
  }

  async function handleSaveConfig() {
    setConfigError(null);

    if (!config.apiKey.trim()) {
      setConfigError("API key is required.");
      return;
    }
    if (!config.model.trim()) {
      setConfigError("Model is required.");
      return;
    }
    if (config.provider === "openai-compatible" && !config.baseUrl?.trim()) {
      setConfigError("Base URL is required for OpenAI Compatible.");
      return;
    }
    if (config.provider === "azure") {
      if (!config.azureEndpoint?.trim()) {
        setConfigError("Azure endpoint is required.");
        return;
      }
      if (!config.apiVersion?.trim()) {
        setConfigError("API version is required.");
        return;
      }
    }

    setConfigSaving(true);
    try {
      await invoke("save_config", { config });
      setConfigOpen(false);
    } catch (err) {
      setConfigError(String(err));
    } finally {
      setConfigSaving(false);
    }
  }

  return (
    <div className="flex h-screen flex-col bg-background">
      <div className="flex items-center justify-between border-b px-4 py-3">
        <div>
          <div className="text-sm font-semibold">Aries</div>
          <div className="text-xs text-muted-foreground">
            {configLoading ? "Loading model config..." : `Current provider: ${providerLabel(config.provider)}`}
          </div>
        </div>
        <Button variant="outline" onClick={() => setConfigOpen(true)}>
          <Settings className="mr-1.5 h-4 w-4" />
          Model Config
        </Button>
      </div>

      <div className="flex-1 overflow-hidden px-4 py-4">
        <div className="mb-4 flex items-center gap-2">
          <div className="relative flex-1">
            <Search className="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Search projects"
              className="pl-8"
            />
          </div>
          <Button variant="outline" onClick={handleOpen}>
            <FolderOpen className="mr-1.5 h-4 w-4" />
            Open
          </Button>
        </div>

        <div className="space-y-1 overflow-y-auto" style={{ maxHeight: "calc(100vh - 170px)" }}>
          {filtered.length === 0 && (
            <div className="py-12 text-center text-sm text-muted-foreground">
              {projects.length === 0
                ? "No recent projects. Click \"Open\" to select a directory."
                : "No projects match your search."}
            </div>
          )}
          {filtered.map((project) => (
            <button
              key={project.id}
              onClick={() => handleSelectProject(project.path)}
              className="flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition-colors hover:bg-accent"
            >
              <div
                className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-xs font-bold text-white ${getColor(project.name)}`}
              >
                {getInitials(project.name)}
              </div>
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium">{project.name}</div>
                <div className="truncate text-xs text-muted-foreground">
                  {project.path.replace(/^\/Users\/[^/]+/, "~")}
                </div>
                {project.branch && (
                  <div className="flex items-center gap-1 text-xs text-muted-foreground">
                    <GitBranch className="h-3 w-3" />
                    <span>{project.branch}</span>
                  </div>
                )}
              </div>
            </button>
          ))}
        </div>
      </div>

      <Dialog open={configOpen} onOpenChange={setConfigOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Model Configuration</DialogTitle>
            <DialogDescription>
              Configure the provider used by Aries. DeepSeek V4 requires API key and model.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Provider</label>
              <select
                value={config.provider}
                onChange={(e) => {
                  const provider = e.target.value as ConfigProvider;
                  setConfig((prev) => ({
                    ...prev,
                    provider,
                    model:
                      provider === "deepseek-v4"
                        ? prev.model || "deepseek-chat"
                        : prev.model,
                  }));
                }}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
              >
                <option value="deepseek-v4">DeepSeek V4</option>
                <option value="openai-compatible">OpenAI Compatible</option>
                <option value="azure">Azure</option>
              </select>
            </div>

            {config.provider === "openai-compatible" && (
              <div className="space-y-2">
                <label className="text-sm font-medium">Base URL</label>
                <Input
                  value={config.baseUrl ?? ""}
                  onChange={(e) => setConfig((prev) => ({ ...prev, baseUrl: e.target.value }))}
                  placeholder="https://api.openai.com/v1"
                />
              </div>
            )}

            {config.provider === "azure" && (
              <>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Azure Endpoint</label>
                  <Input
                    value={config.azureEndpoint ?? ""}
                    onChange={(e) => setConfig((prev) => ({ ...prev, azureEndpoint: e.target.value }))}
                    placeholder="https://your-resource.openai.azure.com"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">API Version</label>
                  <Input
                    value={config.apiVersion ?? ""}
                    onChange={(e) => setConfig((prev) => ({ ...prev, apiVersion: e.target.value }))}
                    placeholder="2024-02-01"
                  />
                </div>
              </>
            )}

            <div className="space-y-2">
              <label className="text-sm font-medium">API Key</label>
              <Input
                type="password"
                value={config.apiKey}
                onChange={(e) => setConfig((prev) => ({ ...prev, apiKey: e.target.value }))}
                placeholder="sk-..."
              />
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">Model</label>
              <Input
                value={config.model}
                onChange={(e) => setConfig((prev) => ({ ...prev, model: e.target.value }))}
                placeholder={config.provider === "deepseek-v4" ? "deepseek-chat" : "Enter model name"}
              />
            </div>

            {config.provider === "deepseek-v4" && (
              <div className="rounded-md border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
                DeepSeek V4 uses the built-in base URL: https://api.deepseek.com
              </div>
            )}

            {configError && <div className="text-sm text-destructive">{configError}</div>}
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setConfigOpen(false)} disabled={configSaving}>
              Cancel
            </Button>
            <Button onClick={handleSaveConfig} disabled={configSaving}>
              {configSaving ? "Saving..." : "Save"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
