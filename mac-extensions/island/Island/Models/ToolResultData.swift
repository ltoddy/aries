import Foundation

enum ToolResultData: Equatable {
    case read(ReadResult)
    case edit(EditResult)
    case write(WriteResult)
    case bash(BashResult)
    case grep(GrepResult)
    case glob(GlobResult)
    case todoWrite(TodoWriteResult)
    case task(TaskResult)
    case webFetch(WebFetchResult)
    case webSearch(WebSearchResult)
    case askUserQuestion(AskUserQuestionResult)
    case bashOutput(BashOutputResult)
    case killShell(KillShellResult)
    case exitPlanMode(ExitPlanModeResult)
    case mcp(MCPResult)
    case generic(GenericResult)
}

struct ReadResult: Equatable {
    let filePath: String
    let content: String
    let numLines: Int
    let startLine: Int
    let totalLines: Int

    var filename: String {
        URL(fileURLWithPath: filePath).lastPathComponent
    }
}

struct EditResult: Equatable {
    let filePath: String
    let oldString: String
    let newString: String
    let replaceAll: Bool
    let userModified: Bool
    let structuredPatch: [PatchHunk]?

    var filename: String {
        URL(fileURLWithPath: filePath).lastPathComponent
    }
}

struct PatchHunk: Equatable {
    let oldStart: Int
    let oldLines: Int
    let newStart: Int
    let newLines: Int
    let lines: [String]
}

struct WriteResult: Equatable {
    enum WriteType: String, Equatable {
        case create
        case overwrite
    }

    let type: WriteType
    let filePath: String
    let content: String
    let structuredPatch: [PatchHunk]?

    var filename: String {
        URL(fileURLWithPath: filePath).lastPathComponent
    }
}

struct BashResult: Equatable {
    let stdout: String
    let stderr: String
    let interrupted: Bool
    let isImage: Bool
    let returnCodeInterpretation: String?
    let backgroundTaskId: String?

    var hasOutput: Bool {
        !stdout.isEmpty || !stderr.isEmpty
    }
}

struct GrepResult: Equatable {
    enum Mode: String, Equatable {
        case filesWithMatches = "files_with_matches"
        case content
        case count
    }

    let mode: Mode
    let filenames: [String]
    let numFiles: Int
    let content: String?
    let numLines: Int?
    let appliedLimit: Int?
}

struct GlobResult: Equatable {
    let filenames: [String]
    let durationMs: Int
    let numFiles: Int
    let truncated: Bool
}

struct TodoWriteResult: Equatable {
    let oldTodos: [TodoItem]
    let newTodos: [TodoItem]
}

struct TodoItem: Equatable {
    let content: String
    let status: String
    let activeForm: String?
}

struct TaskResult: Equatable {
    let agentId: String
    let status: String
    let content: String
    let prompt: String?
    let totalDurationMs: Int?
    let totalTokens: Int?
    let totalToolUseCount: Int?
}

struct WebFetchResult: Equatable {
    let url: String
    let code: Int
    let codeText: String
    let bytes: Int
    let durationMs: Int
    let result: String
}

struct WebSearchResult: Equatable {
    let query: String
    let durationSeconds: Double
    let results: [SearchResultItem]
}

struct SearchResultItem: Equatable {
    let title: String
    let url: String
    let snippet: String
}

struct AskUserQuestionResult: Equatable {
    let questions: [QuestionItem]
    let answers: [String: String]
}

struct QuestionItem: Equatable {
    let question: String
    let header: String?
    let options: [QuestionOption]
}

struct QuestionOption: Equatable {
    let label: String
    let description: String?
}

struct BashOutputResult: Equatable {
    let shellId: String
    let status: String
    let stdout: String
    let stderr: String
    let stdoutLines: Int
    let stderrLines: Int
    let exitCode: Int?
    let command: String?
    let timestamp: String?
}

struct KillShellResult: Equatable {
    let shellId: String
    let message: String
}

struct ExitPlanModeResult: Equatable {
    let filePath: String?
    let plan: String?
    let isAgent: Bool
}

struct MCPResult: Equatable, @unchecked Sendable {
    let serverName: String
    let toolName: String
    let rawResult: [String: Any]

    static func == (lhs: MCPResult, rhs: MCPResult) -> Bool {
        lhs.serverName == rhs.serverName &&
            lhs.toolName == rhs.toolName &&
            NSDictionary(dictionary: lhs.rawResult).isEqual(to: rhs.rawResult)
    }
}

struct GenericResult: Equatable, @unchecked Sendable {
    let rawContent: String?
    let rawData: [String: Any]?

    static func == (lhs: GenericResult, rhs: GenericResult) -> Bool {
        guard lhs.rawContent == rhs.rawContent else { return false }

        if let lhsData = lhs.rawData, let rhsData = rhs.rawData {
            return NSDictionary(dictionary: lhsData).isEqual(to: rhsData)
        }
        return lhs.rawData == nil && rhs.rawData == nil
    }
}

struct ToolStatusDisplay {
    let text: String

    static func running(for toolName: String, input: [String: String]) -> ToolStatusDisplay {
        switch toolName {
        case "Read":
            return ToolStatusDisplay(text: "Reading...")
        case ToolName.edit:
            return ToolStatusDisplay(text: "Editing...")
        case "Write":
            return ToolStatusDisplay(text: "Writing...")
        case "Bash":
            if let desc = input["description"], !desc.isEmpty {
                return ToolStatusDisplay(text: desc)
            }
            return ToolStatusDisplay(text: "Running...")
        case "Grep", "Glob":
            if let pattern = input["pattern"] {
                return ToolStatusDisplay(text: "Searching: \(pattern)")
            }
            return ToolStatusDisplay(text: "Searching...")
        case "WebSearch":
            if let query = input["query"] {
                return ToolStatusDisplay(text: "Searching: \(query)")
            }
            return ToolStatusDisplay(text: "Searching...")
        case "WebFetch":
            return ToolStatusDisplay(text: "Fetching...")
        case ToolName.task, ToolName.agent:
            if let desc = input["description"], !desc.isEmpty {
                return ToolStatusDisplay(text: desc)
            }
            return ToolStatusDisplay(text: "Running agent...")
        case "TodoWrite":
            return ToolStatusDisplay(text: "Updating todos...")
        case "EnterPlanMode":
            return ToolStatusDisplay(text: "Entering plan mode...")
        case "ExitPlanMode":
            return ToolStatusDisplay(text: "Exiting plan mode...")
        default:
            return ToolStatusDisplay(text: "Running...")
        }
    }

    static func completed(for _: String, result: ToolResultData?) -> ToolStatusDisplay {
        guard let result else {
            return ToolStatusDisplay(text: "Completed")
        }

        switch result {
        case let .read(r):
            let lineText = r.totalLines > r.numLines ? "\(r.numLines)+ lines" : "\(r.numLines) lines"
            return ToolStatusDisplay(text: "Read \(r.filename) (\(lineText))")

        case let .edit(r):
            return ToolStatusDisplay(text: "Edited \(r.filename)")

        case let .write(r):
            let action = r.type == .create ? "Created" : "Wrote"
            return ToolStatusDisplay(text: "\(action) \(r.filename)")

        case let .bash(r):
            if let bgId = r.backgroundTaskId {
                return ToolStatusDisplay(text: "Running in background (\(bgId))")
            }
            if let interpretation = r.returnCodeInterpretation {
                return ToolStatusDisplay(text: interpretation)
            }
            return ToolStatusDisplay(text: "Completed")

        case let .grep(r):
            let fileWord = r.numFiles == 1 ? "file" : "files"
            return ToolStatusDisplay(text: "Found \(r.numFiles) \(fileWord)")

        case let .glob(r):
            let fileWord = r.numFiles == 1 ? "file" : "files"
            if r.numFiles == 0 {
                return ToolStatusDisplay(text: "No files found")
            }
            return ToolStatusDisplay(text: "Found \(r.numFiles) \(fileWord)")

        case .todoWrite:
            return ToolStatusDisplay(text: "Updated todos")

        case let .task(r):
            return ToolStatusDisplay(text: r.status.capitalized)

        case let .webFetch(r):
            return ToolStatusDisplay(text: "\(r.code) \(r.codeText)")

        case let .webSearch(r):
            let time = r.durationSeconds >= 1 ?
                "\(Int(r.durationSeconds))s" :
                "\(Int(r.durationSeconds * 1000))ms"
            let searchWord = r.results.count == 1 ? "search" : "searches"
            return ToolStatusDisplay(text: "Did 1 \(searchWord) in \(time)")

        case .askUserQuestion:
            return ToolStatusDisplay(text: "Answered")

        case let .bashOutput(r):
            return ToolStatusDisplay(text: "Status: \(r.status)")

        case .killShell:
            return ToolStatusDisplay(text: "Terminated")

        case .exitPlanMode:
            return ToolStatusDisplay(text: "Plan ready")

        case .mcp:
            return ToolStatusDisplay(text: "Completed")

        case .generic:
            return ToolStatusDisplay(text: "Completed")
        }
    }
}
