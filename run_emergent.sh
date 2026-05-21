#!/bin/bash
cd /Users/rowan/Projects/agent-world/agent-runtime
source .venv/bin/activate

for i in 1 2 3 4 5; do
  python -m agent_runtime spawn --name "Agent$i" --world-url http://localhost:3000 --max-ticks 20 --llm-provider ollama --llm-model qwen3:1.7b --tick-interval 30 2>&1 | grep -E "decided|failed|fallback|stopped|ticks|errors|duration|shutdown" &
done
echo "All 5 agents launched, waiting..."
wait
echo "All agents completed"
