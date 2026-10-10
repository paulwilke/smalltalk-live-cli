#!/bin/bash
# Regenerates docs/commands.md from the CLI's own help text.
cd "$(dirname "$0")/.." && S=target/release/stlive
{ echo "# Command reference"; echo; echo "Generated from \`stlive --help\` (\`scripts/gen-reference.sh\`). Every command prints JSON unless \`--text\` is given."; echo; echo '```'; $S --help; echo '```'
for c in init attach run drift start stop status ping instances eval obj find class method changes test package debug image save load ui open-ui raw; do
  echo; echo "## \`stlive $c\`"; echo; echo '```'; $S $c --help 2>&1; echo '```'
  for sub in $($S $c --help 2>/dev/null | awk '/^Commands:/{f=1;next} /^Options:/{f=0} f&&NF{print $1}' | grep -v '^help$'); do
    echo; echo "### \`stlive $c $sub\`"; echo; echo '```'; $S $c $sub --help 2>&1; echo '```'; done
done; } > docs/commands.md
