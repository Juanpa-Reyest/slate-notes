// Extracts the release notes for one version from CHANGELOG.md.
//
// Usage: node .github/scripts/extract-changelog.cjs <version>
//
// Prints everything under the "## [<version>]" heading up to (but not including)
// the next "## [" heading. Prints nothing if there is no section for the version,
// which lets the release workflow fall back to GitHub's auto-generated notes.

const fs = require("fs");
const path = require("path");

const version = process.argv[2];
if (!version) process.exit(0);

let markdown;
try {
  markdown = fs.readFileSync(path.join(process.cwd(), "CHANGELOG.md"), "utf8");
} catch {
  process.exit(0); // no CHANGELOG.md -> fall back to auto-generated notes
}

const out = [];
let inSection = false;
for (const line of markdown.split(/\r?\n/)) {
  if (/^##\s+\[/.test(line)) {
    if (inSection) break; // reached the next version's section
    inSection = line.includes(`[${version}]`);
    continue; // skip the heading line itself
  }
  if (inSection) out.push(line);
}

process.stdout.write(out.join("\n").trim());
