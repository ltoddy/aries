"""
SWE-bench evaluation driver script for aries agent.

Usage:
    uv run aries_swe_bench.py download
    uv run aries_swe_bench.py list-ids
    uv run aries_swe_bench.py run --limit 5
    uv run aries_swe_bench.py run --instance-ids cli__cli-10388
"""

import click
import json
import os
import subprocess
from pathlib import Path

from huggingface_hub import hf_hub_download, list_repo_files


WORKDIR = Path("/tmp/swe-bench-workspaces")
PREDICTIONS_FILE = Path("predictions.jsonl")
DATASET = "ByteDance-Seed/Multi-SWE-bench"


def dataset_files() -> list[str]:
    return [
        f for f in list_repo_files(DATASET, repo_type="dataset") if f.endswith(".jsonl")
    ]


def load_tasks() -> list[dict]:
    tasks = []
    for filename in dataset_files():
        path = hf_hub_download(DATASET, filename, repo_type="dataset")
        with open(path) as f:
            for line in f:
                tasks.append(json.loads(line))
    return tasks


def prepare_repo(repo: str, base_commit: str, work_dir: Path) -> Path:
    repo_url = f"https://github.com/{repo}.git"
    repo_dir = work_dir / repo.replace(os.sep, "__")

    if not repo_dir.exists():
        subprocess.run(
            ["git", "clone", repo_url, str(repo_dir)],
            check=True,
            capture_output=True,
        )

    subprocess.run(
        ["git", "checkout", base_commit],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "clean", "-fdx"],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "checkout", "."],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )
    return repo_dir


def run_aries(repo_dir: Path, problem_statement: str):
    prompt = (
        "You are a software engineer. Fix the following issue in this repository.\n\n"
        f"## Issue\n\n{problem_statement}\n\n"
        "## Instructions\n"
        "1. Read the relevant code to understand the problem.\n"
        "2. Make the minimal code changes needed to fix the issue.\n"
        "3. Do NOT run tests or commit - just edit the files.\n"
    )

    proc = subprocess.Popen(
        ["aries"],
        cwd=repo_dir,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        proc.communicate(input=prompt, timeout=600)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.communicate()
        raise


def collect_patch(repo_dir: Path) -> str:
    result = subprocess.run(
        ["git", "diff"],
        cwd=repo_dir,
        capture_output=True,
        text=True,
    )
    return result.stdout


def current_model() -> str:
    model = subprocess.run(
        ["aries", "model", "current"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    return f"[aries] {model}"


@click.group()
def cli():
    pass


@cli.command()
@click.option("--limit", type=int, default=5, help="Number of tasks to run")
@click.option("--instance-ids", multiple=True, help="Specific instance IDs to run")
@click.option(
    "--output",
    type=click.Path(path_type=Path),
    default=PREDICTIONS_FILE,
    help="Output predictions file",
)
def run(limit: int, instance_ids: tuple[str, ...], output: Path):
    tasks = load_tasks()

    if instance_ids:
        tasks = [t for t in tasks if t["instance_id"] in instance_ids]
    else:
        tasks = tasks[:limit]

    WORKDIR.mkdir(parents=True, exist_ok=True)

    predictions = []

    for i, task in enumerate(tasks):
        instance_id = task["instance_id"]
        repo = f"{task['org']}/{task['repo']}"
        base_commit = task["base"]["sha"]
        problem_statement = task["resolved_issues"][0]["body"]
        print(f"[{i + 1}/{len(tasks)}] Processing {instance_id}...")

        try:
            repo_dir = prepare_repo(repo, base_commit, WORKDIR)
            run_aries(repo_dir, problem_statement)
            patch = collect_patch(repo_dir)

            model_name = current_model()

            prediction = {
                "instance_id": instance_id,
                "model_name_or_path": model_name,
                "model_patch": patch,
            }
            predictions.append(prediction)

            if patch:
                print(f"  ✓ Generated patch ({len(patch)} bytes)")
            else:
                print("  ✗ No patch generated")

        except subprocess.TimeoutExpired:
            print("  ✗ Timeout (10min)")
            predictions.append(
                {
                    "instance_id": instance_id,
                    "model_name_or_path": current_model(),
                    "model_patch": "",
                }
            )
        except Exception as e:
            print(f"  ✗ Error: {e}")
            predictions.append(
                {
                    "instance_id": instance_id,
                    "model_name_or_path": current_model(),
                    "model_patch": "",
                }
            )

    with open(output, "w") as f:
        for pred in predictions:
            f.write(json.dumps(pred) + "\n")

    print(f"\nDone. {len(predictions)} predictions written to {output}")
    print(
        f"Patches generated: {sum(1 for p in predictions if p['model_patch'])}/{len(predictions)}"
    )


@cli.command("list-ids")
def list_ids():
    tasks = load_tasks()
    for task in tasks:
        print(task["instance_id"])


@cli.command("download")
def download():
    for filename in dataset_files():
        hf_hub_download(DATASET, filename, repo_type="dataset")
    print(f"Dataset '{DATASET}' downloaded successfully.")


if __name__ == "__main__":
    cli()
