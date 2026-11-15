# Install and use the agent skill

From the Oxidize checkout, install into an explicitly chosen project. No global configuration or credentials are changed:

```bash
python3 skills/oxidize/scripts/install.py --agent codex --project /path/to/project
```

The complete folder, including references and installer, is copied. Existing skills are never overwritten. These project layouts are documented by the respective agents:

| Agent | Project destination | Use |
| --- | --- | --- |
| Codex terminal | `.agents/skills/oxidize/` | Reload/restart skills, request `$oxidize` with the target and goal. |
| Claude Code terminal | `.claude/skills/oxidize/` | Invoke `/oxidize` with the target and goal. |
| OpenCode | `.opencode/skills/oxidize/` | Ask the agent to load the `oxidize` skill. |
| Gemini CLI | `.gemini/skills/oxidize/` | `/skills reload`, inspect `/skills list`, then request Rustification and activate the skill when offered. |

Use `--agent claude`, `--agent opencode`, or `--agent gemini` to select the corresponding layout. Cloud/desktop clients can have different plugin/skill distribution rules; a project folder is not guaranteed to install into every hosted client. This package has been structurally validated and its CLI-assisted offline methodology exercised in a Codex coding session; external client discovery and autonomous execution in all four agents have not been tested.

Example request: “Use oxidize to inspect this Python utility, migrate one compute component through PyO3 in a separate workspace, run existing tests and differential cases, and accept only if measured performance meets the configured policy.”

With the CLI installed, the skill coordinates its commands. Without it, the skill guides real implementation using the agent's existing execution tools. The agent-native method also covers languages and Rust optimization outside the current CLI adapter, but each result still needs independent validation. It does not install toolchains, spend inference credits, bypass sandbox policies, or push changes merely by being discovered.

Official references checked during initial implementation:

- [OpenAI CLI skill installation and Codex project layout](https://developers.openai.com/api/reference/cli)
- [OpenAI skill examples](https://github.com/openai/skills) (deprecated catalog; points to current plugin packaging)
- [Claude Code skills](https://code.claude.com/docs/en/skills)
- [OpenCode skills](https://opencode.ai/docs/skills/)
- [Gemini CLI skills](https://geminicli.com/docs/cli/skills/)
- [PyO3 0.30.1](https://github.com/PyO3/pyo3/releases/tag/v0.30.1)
