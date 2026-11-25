@AGENTS.md

# Claude Code notes

The import above pulls in the repo's canonical guide, so edit `AGENTS.md`
rather than repeating project rules here.

- **Skills:** `.claude/skills/` mirrors `.agents/skills/` (where Codex looks);
  Claude Code and OpenCode read this copy. Edit the `.agents/skills/` copy, then
  copy it over; CI runs `diff -r .claude/skills .agents/skills` and fails if
  they drift.
- **Search:** for broad codebase questions, use the `Explore` agent when it's
  available instead of reading files one by one.
- **Permissions:** no project-specific MCP servers, hooks or permission
  settings are configured. Ask before adding any.
