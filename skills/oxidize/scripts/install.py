"""Install this skill into an explicitly selected project; never overwrite a skill."""
import argparse
import pathlib
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--agent', choices=['codex', 'claude', 'opencode', 'gemini'], required=True)
parser.add_argument('--project', type=pathlib.Path, required=True)
args = parser.parse_args()
project = args.project.resolve(strict=True)
if not project.is_dir():
    parser.error('--project must be a directory')
location = {'codex': '.agents', 'claude': '.claude', 'opencode': '.opencode', 'gemini': '.gemini'}[args.agent]
parent = project / location / 'skills'
for path in [project / location, parent]:
    if path.is_symlink():
        parser.error('refusing to install through a symlink')
destination = parent / 'oxidize'
if destination.exists() or destination.is_symlink():
    parser.error(f'skill already exists: {destination}')
parent.mkdir(parents=True, exist_ok=True)
source = pathlib.Path(__file__).resolve().parents[1]
shutil.copytree(source, destination, ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
print(f'Installed {destination}; reload skills or restart your agent.')
