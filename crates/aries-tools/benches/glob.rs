use std::hint::black_box;
use std::path::{Path, PathBuf};

use aries_tools::glob::{GlobArgs, GlobTool};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rig::tool::{Tool, ToolContext};
use tempfile::TempDir;

struct BenchRepo {
    _tempdir: TempDir,
    root: PathBuf,
}

impl BenchRepo {
    fn new(files: usize, nested_dirs: usize) -> Self {
        let tempdir = TempDir::new().unwrap();
        let root = tempdir.path().to_path_buf();

        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join("vendor")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();

        std::fs::write(root.join(".gitignore"), "vendor/\n").unwrap();

        for i in 0..nested_dirs {
            std::fs::create_dir_all(root.join("src").join(format!("nested_{i}")).join("deep"))
                .unwrap();
        }

        for i in 0..files {
            let path = match i % 4 {
                0 => root.join("src").join(format!("file_{i}.rs")),
                1 => root.join("docs").join(format!("doc_{i}.md")),
                2 => root.join("vendor").join(format!("vendored_{i}.rs")),
                _ => {
                    let nested = i % nested_dirs.max(1);
                    root.join("src")
                        .join(format!("nested_{nested}"))
                        .join("deep")
                        .join(format!("deep_{i}.rs"))
                },
            };
            write_fixture_file(&path, i);
        }

        for i in 0..(files / 10).max(1) {
            write_fixture_file(&root.join(".hidden").join(format!("hidden_{i}.rs")), i);
        }

        Self { _tempdir: tempdir, root }
    }
}

fn write_fixture_file(path: &Path, index: usize) {
    std::fs::write(path, format!("fixture file {index}\n")).unwrap();
}

fn glob_args(pattern: &str) -> GlobArgs {
    GlobArgs {
        pattern: pattern.to_string(),
        base_dir: None,
        hidden: false,
        respect_gitignore: true,
    }
}

fn total_bytes(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().unwrap().len();
    }

    std::fs::read_dir(path).unwrap().map(|entry| total_bytes(&entry.unwrap().path())).sum()
}

fn run_glob(tool: &GlobTool, args: GlobArgs) {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let mut context = ToolContext::new();
        let output = tool.call(&mut context, args).await.unwrap();
        black_box(output);
    });
}

fn bench_glob(c: &mut Criterion) {
    let repo = BenchRepo::new(2_000, 32);
    let tool = GlobTool::new(&repo.root);

    let scenarios = [
        ("flat_rs", glob_args("src/*.rs")),
        ("nested_rs", glob_args("src/**/*.rs")),
        ("all_rs_ignore_vendor", glob_args("**/*.rs")),
        ("all_rs_include_hidden", {
            let mut args = glob_args("**/*.rs");
            args.hidden = true;
            args
        }),
        ("all_rs_without_gitignore", {
            let mut args = glob_args("**/*.rs");
            args.respect_gitignore = false;
            args
        }),
    ];

    let total_bytes = total_bytes(&repo.root);

    let mut group = c.benchmark_group("glob_tool");
    group.throughput(Throughput::Bytes(total_bytes));

    for (name, args) in scenarios {
        group.bench_with_input(BenchmarkId::from_parameter(name), &args, |b, args| {
            b.iter(|| run_glob(&tool, args.clone()));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_glob);
criterion_main!(benches);
