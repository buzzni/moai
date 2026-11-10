## moai v1.1.0
### What's new
- The web viewer supports footnotes, MathML math, Mermaid diagrams, CJK-friendly emphasis and file snippets in issue bodies and wiki pages
- The explorer and `moai show` render numbered footnotes, math source and marked Mermaid code blocks
- `GET /api/snippet` reads a permitted text file or region for the selected checkout, with the shared snippet path and sensitive-file rules
- Web issue bodies and wiki pages use markdown-it and CommonMark
- The web page loads third-party markdown and diagram code in `/vendor.js` before its own app bundle

Release notes: <https://github.com/buzzni/moai/releases/tag/v1.1.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
