// This file contains tests generated with AI assistance.

use super::*;

fn memory(file_name: &str, description: &str, memory_type: MemoryType) -> Memory {
    Memory::new(
        file_name,
        file_name,
        MemoryFrontmatter {
            name: file_name.to_owned(),
            description: description.to_owned(),
            memory_type,
        },
        "body",
    )
}

#[test]
fn retriever_line_includes_type_and_description() {
    let mem = memory("prefers_go.md", "偏好使用 Go", MemoryType::User);
    assert_eq!(mem.to_retriever_line(), "- [prefers_go.md] (user) — 偏好使用 Go");
}

#[test]
fn retriever_line_renders_each_memory_type() {
    assert_eq!(
        memory("a.md", "d", MemoryType::Feedback).to_retriever_line(),
        "- [a.md] (feedback) — d"
    );
    assert_eq!(
        memory("b.md", "d", MemoryType::Project).to_retriever_line(),
        "- [b.md] (project) — d"
    );
    assert_eq!(
        memory("c.md", "d", MemoryType::Reference).to_retriever_line(),
        "- [c.md] (reference) — d"
    );
}
