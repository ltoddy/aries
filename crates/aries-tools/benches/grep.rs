use std::hint::black_box;
use std::path::{Path, PathBuf};

use aries_tools::grep::{GrepArgs, GrepTool, OutputMode};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rig::tool::{Tool, ToolContext};
use tempfile::TempDir;

struct BenchRepo {
    _tempdir: TempDir,
    root: PathBuf,
}

impl BenchRepo {
    fn new(files: usize, lines_per_file: usize) -> Self {
        let tempdir = TempDir::new().unwrap();
        let root = tempdir.path().to_path_buf();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join("vendor")).unwrap();

        for i in 0..files {
            let dir = match i % 3 {
                0 => root.join("src"),
                1 => root.join("docs"),
                _ => root.join("vendor"),
            };
            let ext = if i % 3 == 0 { "rs" } else { "txt" };
            let path = dir.join(format!("file_{i}.{ext}"));
            write_fixture_file(&path, i, lines_per_file);
        }

        Self { _tempdir: tempdir, root }
    }
}

fn write_fixture_file(path: &Path, file_index: usize, lines_per_file: usize) {
    let mut content = String::new();
    for line in 0..lines_per_file {
        if line % 17 == 0 {
            content.push_str(&format!(
                "fn hot_path_{file_index}_{line}() {{ let needle = {line}; }}\n"
            ));
        } else if line % 29 == 0 {
            content.push_str(&format!("Needle appears in mixed case at {file_index}:{line}\n"));
        } else {
            content
                .push_str(&format!("line {line} filler content for benchmark file {file_index}\n"));
        }
    }
    std::fs::write(path, content).unwrap();
}

fn grep_args(pattern: &str) -> GrepArgs {
    GrepArgs {
        pattern: pattern.to_string(),
        include: None,
        output_mode: OutputMode::FilesWithMatches,
        case_insensitive: false,
        show_line_numbers: true,
        context_before: None,
        context_after: None,
        context: None,
        respect_gitignore: false,
        head_limit: 0,
    }
}

fn run_grep(tool: &GrepTool, args: GrepArgs) {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let mut context = ToolContext::new();
        let output = tool.call(&mut context, args).await.unwrap();
        black_box(output);
    });
}

fn bench_grep(c: &mut Criterion) {
    let repo = BenchRepo::new(180, 240);
    let tool = GrepTool::new(&repo.root);

    let scenarios = [
        ("files_with_matches", {
            let mut args = grep_args("needle");
            args.output_mode = OutputMode::FilesWithMatches;
            args
        }),
        ("content_include_rs", {
            let mut args = grep_args("hot_path");
            args.output_mode = OutputMode::Content;
            args.include = Some("src/**/*.rs".to_string());
            args
        }),
        ("count_case_insensitive", {
            let mut args = grep_args("needle");
            args.output_mode = OutputMode::Count;
            args.case_insensitive = true;
            args
        }),
    ];

    let total_bytes: u64 = std::fs::read_dir(&repo.root)
        .unwrap()
        .flat_map(|entry| std::fs::read_dir(entry.unwrap().path()).unwrap())
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();

    let mut group = c.benchmark_group("grep_tool");
    group.throughput(Throughput::Bytes(total_bytes));

    for (name, args) in scenarios {
        group.bench_with_input(BenchmarkId::from_parameter(name), &args, |b, args| {
            b.iter(|| run_grep(&tool, args.clone()));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_grep);
criterion_main!(benches);
