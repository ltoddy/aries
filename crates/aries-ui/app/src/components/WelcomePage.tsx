import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, GitBranch, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { ProjectEntry } from "../types";

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

export function WelcomePage({ onSelect }: { onSelect: (path: string) => void }) {
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [filter, setFilter] = useState("");

  useEffect(() => {
    invoke<ProjectEntry[]>("list_projects").then(setProjects).catch(() => {});
  }, []);

  const filtered = projects.filter(
    (p) =>
      p.name.toLowerCase().includes(filter.toLowerCase()) ||
      p.path.toLowerCase().includes(filter.toLowerCase())
  );

  async function handleOpen() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      await invoke("open_project", { path: selected });
      onSelect(selected);
    }
  }

  async function handleSelectProject(path: string) {
    await invoke("open_project", { path });
    onSelect(path);
  }

  return (
    <div className="flex h-screen flex-col bg-background">
      {/* Content */}
      <div className="flex-1 overflow-hidden px-4 py-4">
        {/* Search and Open */}
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

        {/* Project list */}
        <div className="space-y-1 overflow-y-auto" style={{ maxHeight: "calc(100vh - 120px)" }}>
          {filtered.length === 0 && (
            <div className="py-12 text-center text-sm text-muted-foreground">
              {projects.length === 0
                ? "No recent projects. Click \"Open\" to select a directory."
                : "No projects match your search."}
            </div>
          )}
          {filtered.map((project) => (
            <button
              key={project.path}
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
    </div>
  );
}
