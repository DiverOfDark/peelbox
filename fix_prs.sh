#!/bin/bash
cd /root/.openclaw/workspace/peelbox || exit 1

# List of failing PRs based on previous checks
PRS=(132 131 130 129 128 127 126 125 124 112)

for pr in "${PRS[@]}"; do
    echo "Processing PR $pr..."
    gh pr checkout $pr
    # Run cargo clippy and automatically apply fixes
    cargo clippy --fix --allow-dirty --allow-staged
    
    # Check if there are any changes
    if ! git diff --quiet; then
        git add .
        git commit -m "style: fix cargo clippy warnings"
        git push
        echo "Pushed fixes for PR $pr."
    else
        echo "No changes for PR $pr."
    fi
done

# Switch back to master
git checkout master
echo "All done."
