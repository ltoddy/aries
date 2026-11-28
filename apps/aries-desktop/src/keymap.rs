use gpui_kit::component::dialog::Cancel;
use gpui_kit::component::input::{Escape, Search};
use gpui_kit::*;

use crate::terminal::{ClearTerminal, CopyTerminal, PasteTerminal};
use crate::workspace::files::{
    ConfirmEntry, CopyFile, CopyRelativePath, DeleteEntry, FindEntry, FindFile, NextMatch,
    PreviousFocus, PreviousMatch, RenameEntry, SaveFile,
};
use crate::workspace::settings::SendKey;
use crate::workspace::skill_editor::SaveSkill;
use crate::{
    CloseTab, NextTab, NextUserMessage, OpenWorkspace, PreviousTab, PreviousUserMessage, Quit,
    ShowFilesSidebar, ShowGitSidebar, ShowSessionsSidebar, ToggleSidebar,
};
#[cfg(target_os = "macos")]
use crate::{Hide, NewSession, OpenSettings, ToggleTerminal};

const MACOS: bool = cfg!(target_os = "macos");

const GIT_DIFF: &str = "GitDiff";
const DIALOG: &str = "Dialog || Dialog > (Input || TextView)";
const TERMINAL_CONTEXT: &str = "Terminal";
const FILE_VIEW_CONTEXT: &str = "FileView";
const FILE_TREE_CONTEXT: &str = "FileTree";
const FILE_TREE_CONTENT_CONTEXT: &str = "FileTreeContent";
const SKILL_EDITOR_CONTEXT: &str = "SkillEditor";

const QUIT: &str = if MACOS { "cmd-q" } else { "ctrl-q" };
#[cfg(target_os = "macos")]
const QUIT_CTRL: &str = "ctrl-q";
const HIDE: &str = "cmd-h";
const OPEN_SETTINGS: &str = "cmd-,";
const NEW_SESSION: &str = "cmd-n";
const TOGGLE_TERMINAL: &str = "cmd-j";
const TOGGLE_SIDEBAR: &str = if MACOS { "cmd-b" } else { "ctrl-b" };
const OPEN_WORKSPACE: &str = if MACOS { "cmd-alt-o" } else { "ctrl-alt-o" };
const SHOW_SESSIONS_SIDEBAR: &str = if MACOS { "cmd-1" } else { "ctrl-1" };
const SHOW_FILES_SIDEBAR: &str = if MACOS { "cmd-2" } else { "ctrl-2" };
const SHOW_GIT_SIDEBAR: &str = if MACOS { "cmd-3" } else { "ctrl-3" };
const CLOSE_TAB: &str = if MACOS { "cmd-w" } else { "ctrl-w" };
const PREVIOUS_TAB: &str = if MACOS { "cmd-shift-[" } else { "ctrl-shift-[" };
const NEXT_TAB: &str = if MACOS { "cmd-shift-]" } else { "ctrl-shift-]" };
const PREVIOUS_TAB_ALTERNATE: &str = "ctrl-shift-tab";
const NEXT_TAB_ALTERNATE: &str = "ctrl-tab";
const PREVIOUS_USER_MESSAGE: &str = if MACOS { "cmd-alt-up" } else { "ctrl-alt-up" };
const NEXT_USER_MESSAGE: &str = if MACOS { "cmd-alt-down" } else { "ctrl-alt-down" };
const ESCAPE: &str = "escape";

const CLEAR_TERMINAL: &str = "cmd-k";
const COPY_TERMINAL: &str = "cmd-c";
const PASTE_TERMINAL: &str = "cmd-v";

const FIND: &str = if MACOS { "cmd-f" } else { "ctrl-f" };
const SAVE: &str = if MACOS { "cmd-s" } else { "ctrl-s" };
const COPY: &str = if MACOS { "cmd-shift-c" } else { "ctrl-shift-c" };
const NEXT_MATCH: &str = if MACOS { "cmd-g" } else { "ctrl-g" };
const PREVIOUS_MATCH: &str = if MACOS { "cmd-shift-g" } else { "ctrl-shift-g" };
const NEXT_MATCH_ALTERNATE: &str = "f3";
const PREVIOUS_MATCH_ALTERNATE: &str = "shift-f3";
const CONFIRM: &str = "enter";
const PREVIOUS_FOCUS: &str = "shift-tab";
const RENAME: &str = "f2";
const DELETE: &str = if MACOS { "cmd-backspace" } else { "ctrl-backspace" };

pub fn init(cx: &mut App) {
    let mut bindings = vec![
        KeyBinding::new(QUIT, Quit, None),
        KeyBinding::new(OPEN_WORKSPACE, OpenWorkspace::new(), None),
        KeyBinding::new(TOGGLE_SIDEBAR, ToggleSidebar, None),
        KeyBinding::new(SHOW_SESSIONS_SIDEBAR, ShowSessionsSidebar::new(), None),
        KeyBinding::new(SHOW_FILES_SIDEBAR, ShowFilesSidebar::new(), None),
        KeyBinding::new(SHOW_GIT_SIDEBAR, ShowGitSidebar::new(), None),
        KeyBinding::new(CLOSE_TAB, CloseTab, None),
        KeyBinding::new(PREVIOUS_TAB, PreviousTab, None),
        KeyBinding::new(NEXT_TAB, NextTab, None),
        KeyBinding::new(PREVIOUS_TAB_ALTERNATE, PreviousTab, None),
        KeyBinding::new(NEXT_TAB_ALTERNATE, NextTab, None),
        KeyBinding::new(PREVIOUS_USER_MESSAGE, PreviousUserMessage, None),
        KeyBinding::new(NEXT_USER_MESSAGE, NextUserMessage, None),
        KeyBinding::new(FIND, Search, Some(GIT_DIFF)),
        KeyBinding::new(ESCAPE, Escape, Some(GIT_DIFF)),
        KeyBinding::new(ESCAPE, Cancel, Some(DIALOG)),
        KeyBinding::new(CLEAR_TERMINAL, ClearTerminal, Some(TERMINAL_CONTEXT)),
        KeyBinding::new(COPY_TERMINAL, CopyTerminal, Some(TERMINAL_CONTEXT)),
        KeyBinding::new(PASTE_TERMINAL, PasteTerminal, Some(TERMINAL_CONTEXT)),
        KeyBinding::new(FIND, FindFile::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(SAVE, SaveFile::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(COPY, CopyFile::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(NEXT_MATCH, NextMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(PREVIOUS_MATCH, PreviousMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(NEXT_MATCH_ALTERNATE, NextMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(PREVIOUS_MATCH_ALTERNATE, PreviousMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(ESCAPE, Escape, Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(CONFIRM, ConfirmEntry::new(), Some(FILE_TREE_CONTENT_CONTEXT)),
        KeyBinding::new(PREVIOUS_FOCUS, PreviousFocus::new(), Some(FILE_TREE_CONTEXT)),
        KeyBinding::new(FIND, FindEntry::new(), Some(FILE_TREE_CONTEXT)),
        KeyBinding::new(ESCAPE, Escape, Some(FILE_TREE_CONTEXT)),
        KeyBinding::new(RENAME, RenameEntry::new(), Some(FILE_TREE_CONTENT_CONTEXT)),
        KeyBinding::new(DELETE, DeleteEntry::new(), Some(FILE_TREE_CONTENT_CONTEXT)),
        KeyBinding::new(COPY, CopyRelativePath::new(), Some(FILE_TREE_CONTENT_CONTEXT)),
        KeyBinding::new(SAVE, SaveSkill::new(), Some(SKILL_EDITOR_CONTEXT)),
    ];
    #[cfg(target_os = "macos")]
    bindings.extend([
        KeyBinding::new(QUIT_CTRL, Quit, None),
        KeyBinding::new(HIDE, Hide, None),
        KeyBinding::new(OPEN_SETTINGS, OpenSettings, None),
        KeyBinding::new(NEW_SESSION, NewSession, None),
        KeyBinding::new(TOGGLE_TERMINAL, ToggleTerminal, None),
    ]);
    cx.bind_keys(bindings);
}

pub fn groups(send_key: SendKey) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let mut sessions = vec![shortcut("关闭当前标签", &[CLOSE_TAB])];
    if MACOS {
        sessions.insert(0, shortcut("新建会话", &[NEW_SESSION]));
    }
    sessions.push(shortcut("下一个标签", &[NEXT_TAB, NEXT_TAB_ALTERNATE]));
    sessions.push(shortcut("上一个标签", &[PREVIOUS_TAB, PREVIOUS_TAB_ALTERNATE]));
    sessions.push(shortcut("下一条用户消息", &[NEXT_USER_MESSAGE]));
    sessions.push(shortcut("上一条用户消息", &[PREVIOUS_USER_MESSAGE]));

    let mut panels = vec![
        shortcut("显示 / 隐藏侧栏", &[TOGGLE_SIDEBAR]),
        shortcut("显示会话列表", &[SHOW_SESSIONS_SIDEBAR]),
        shortcut("显示文件树", &[SHOW_FILES_SIDEBAR]),
        shortcut("显示 Git 变更", &[SHOW_GIT_SIDEBAR]),
    ];
    if MACOS {
        panels.insert(0, shortcut("显示 / 隐藏终端", &[TOGGLE_TERMINAL]));
        panels.push(shortcut("打开设置", &[OPEN_SETTINGS]));
        panels.push(shortcut("隐藏窗口", &[HIDE]));
    }
    panels.push(shortcut("打开 Aries", &[OPEN_WORKSPACE]));
    panels.push(shortcut("退出应用", &[QUIT]));

    vec![
        ("会话与标签", sessions),
        ("面板与窗口", panels),
        (
            "输入与弹框",
            vec![
                ("发送消息", send_key.send_label().into()),
                ("输入框换行", send_key.newline_label().into()),
                ("关闭弹框 / 退出搜索", format_keystroke(ESCAPE)),
            ],
        ),
        (
            "文件树",
            vec![
                shortcut("打开", &[CONFIRM]),
                shortcut("重命名", &[RENAME]),
                shortcut("删除", &[DELETE]),
                shortcut("复制相对路径", &[COPY]),
                shortcut("查找", &[FIND]),
                shortcut("切换焦点", &[PREVIOUS_FOCUS]),
                shortcut("退出查找", &[ESCAPE]),
            ],
        ),
        (
            "文件编辑器",
            vec![
                shortcut("保存", &[SAVE]),
                shortcut("查找", &[FIND]),
                shortcut("下一个匹配", &[NEXT_MATCH, NEXT_MATCH_ALTERNATE]),
                shortcut("上一个匹配", &[PREVIOUS_MATCH, PREVIOUS_MATCH_ALTERNATE]),
                shortcut("复制文件路径", &[COPY]),
                shortcut("退出查找", &[ESCAPE]),
            ],
        ),
        (
            "终端",
            vec![
                shortcut("清屏", &[CLEAR_TERMINAL]),
                shortcut("复制", &[COPY_TERMINAL]),
                shortcut("粘贴", &[PASTE_TERMINAL]),
            ],
        ),
        ("Git 差异", vec![shortcut("查找", &[FIND]), shortcut("退出查找", &[ESCAPE])]),
        ("技能编辑器", vec![shortcut("保存", &[SAVE])]),
    ]
}

fn shortcut(label: &'static str, keystrokes: &[&str]) -> (&'static str, String) {
    let keys = keystrokes.iter().copied().map(format_keystroke).collect::<Vec<_>>().join(" / ");
    (label, keys)
}

fn format_keystroke(keystroke: &str) -> String {
    let separator = if MACOS { " " } else { " + " };
    keystroke.split('-').map(format_key).collect::<Vec<_>>().join(separator)
}

fn format_key(key: &str) -> String {
    let label = match key {
        "cmd" => "⌘",
        "ctrl" => "Ctrl",
        "alt" if MACOS => "⌥",
        "alt" => "Alt",
        "shift" if MACOS => "⇧",
        "shift" => "Shift",
        "escape" => "Esc",
        "enter" => "Enter",
        "tab" => "Tab",
        "space" => "Space",
        "backspace" if MACOS => "⌫",
        "backspace" => "Backspace",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        other => return other.to_uppercase(),
    };
    label.to_owned()
}
