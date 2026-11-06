## moai v0.49.0
### What's new
- The moai block `moai init` writes into AGENTS.md now keeps only the standing rules (about 45KB down to 6KB); the detail lives in the `moai` skill
- Supervisor and worker steps read only the branch they need — a subagent worker reads about 35KB instead of 51KB
- Run `moai init` and `moai skill install` again after updating

Release notes: <https://github.com/buzzni/moai/releases/tag/v0.49.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
