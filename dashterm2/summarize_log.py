#!/usr/bin/env python3
"""
Summarize a worker JSONL log to extract just the meaningful parts.
Filters out tool output deltas and keeps only:
- Assistant messages
- Tool calls (name + params, not full output)
- Errors
- Final results

Usage: ./summarize_log.py worker_logs/some_log.jsonl > summary.txt
       ./summarize_log.py worker_logs/some_log.jsonl --json > summary.jsonl
"""

import sys
import json
import argparse
from pathlib import Path

def summarize_tool_result(content, max_lines=5):
    """Summarize tool result to first few lines"""
    if isinstance(content, list):
        text_parts = []
        for item in content:
            if isinstance(item, dict) and item.get('type') == 'text':
                text_parts.append(item.get('text', ''))
        text = '\n'.join(text_parts)
    else:
        text = str(content)

    lines = text.strip().split('\n')
    if len(lines) <= max_lines:
        return text
    return '\n'.join(lines[:max_lines]) + f'\n... ({len(lines) - max_lines} more lines)'

def process_log(log_path, output_json=False):
    """Process a JSONL log file and output summary"""

    pending_tools = {}
    events = []

    with open(log_path, 'r') as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue

            try:
                data = json.loads(line)
            except json.JSONDecodeError:
                continue

            # Skip streaming deltas - these are the bulk of the bloat
            msg = data.get('msg', data)
            msg_type = msg.get('type', '')

            if msg_type in ('exec_command_output_delta', 'content_block_delta',
                           'token_count', 'agent_reasoning_section_break'):
                continue

            # Extract meaningful content
            if msg_type == 'assistant':
                inner = msg.get('message', {})
                content = inner.get('content', [])

                if isinstance(content, str):
                    events.append({
                        'type': 'assistant_text',
                        'text': content[:500] + ('...' if len(content) > 500 else '')
                    })
                elif isinstance(content, list):
                    for block in content:
                        if block.get('type') == 'text':
                            text = block.get('text', '')
                            if text.strip():
                                events.append({
                                    'type': 'assistant_text',
                                    'text': text[:500] + ('...' if len(text) > 500 else '')
                                })
                        elif block.get('type') == 'tool_use':
                            tool_id = block.get('id', '')
                            tool_name = block.get('name', '')
                            tool_input = block.get('input', {})

                            # Summarize input
                            summary_input = {}
                            for k, v in tool_input.items():
                                if isinstance(v, str) and len(v) > 100:
                                    summary_input[k] = v[:100] + '...'
                                else:
                                    summary_input[k] = v

                            pending_tools[tool_id] = tool_name
                            events.append({
                                'type': 'tool_call',
                                'tool': tool_name,
                                'input': summary_input
                            })
                        elif block.get('type') == 'thinking':
                            thinking = block.get('thinking', '')
                            if len(thinking) > 50:
                                events.append({
                                    'type': 'thinking',
                                    'length': len(thinking)
                                })

            elif msg_type == 'user':
                inner = msg.get('message', {})
                content = inner.get('content', [])

                if isinstance(content, list):
                    for block in content:
                        if block.get('type') == 'tool_result':
                            tool_id = block.get('tool_use_id', '')
                            tool_name = pending_tools.get(tool_id, 'unknown')
                            result = block.get('content', '')
                            is_error = block.get('is_error', False)

                            if is_error:
                                events.append({
                                    'type': 'tool_error',
                                    'tool': tool_name,
                                    'error': summarize_tool_result(result, max_lines=10)
                                })
                            else:
                                # Just note success, don't include full output
                                events.append({
                                    'type': 'tool_result',
                                    'tool': tool_name,
                                    'preview': summarize_tool_result(result, max_lines=3)
                                })

            elif msg_type == 'result':
                stats = msg.get('stats', {})
                events.append({
                    'type': 'session_end',
                    'stats': stats
                })

            elif msg_type == 'task_started':
                events.append({
                    'type': 'session_start',
                    'model_context_window': msg.get('model_context_window')
                })

    # Output
    if output_json:
        for event in events:
            print(json.dumps(event))
    else:
        for event in events:
            t = event['type']
            if t == 'session_start':
                print(f"\n{'='*60}")
                print(f"SESSION START (context: {event.get('model_context_window')})")
                print('='*60)
            elif t == 'assistant_text':
                print(f"\n💬 {event['text']}")
            elif t == 'tool_call':
                inp = event.get('input', {})
                # Format input nicely
                if event['tool'] == 'Bash':
                    print(f"  → bash: {inp.get('command', '')[:80]}")
                elif event['tool'] == 'Read':
                    print(f"  → read: {inp.get('file_path', '')}")
                elif event['tool'] == 'Write':
                    print(f"  → write: {inp.get('file_path', '')}")
                elif event['tool'] == 'Edit':
                    print(f"  → edit: {inp.get('file_path', '')}")
                elif event['tool'] == 'Grep':
                    print(f"  → grep: '{inp.get('pattern', '')}' in {inp.get('path', '.')}")
                elif event['tool'] == 'Glob':
                    print(f"  → glob: {inp.get('pattern', '')}")
                else:
                    print(f"  → {event['tool']}: {inp}")
            elif t == 'tool_result':
                # Compact result display
                preview = event.get('preview', '')
                if preview and len(preview) < 100:
                    print(f"    ✓ {preview}")
            elif t == 'tool_error':
                print(f"  ✗ ERROR in {event['tool']}:")
                for line in event.get('error', '').split('\n')[:5]:
                    print(f"    {line}")
            elif t == 'thinking':
                print(f"  💭 thinking ({event['length']} chars)")
            elif t == 'session_end':
                stats = event.get('stats', {})
                print(f"\n{'='*60}")
                print(f"SESSION END")
                if stats:
                    print(f"  Tokens: {stats.get('input_tokens', 0):,} in / {stats.get('output_tokens', 0):,} out")
                print('='*60)

def main():
    parser = argparse.ArgumentParser(description='Summarize worker JSONL logs')
    parser.add_argument('log_file', help='Path to JSONL log file')
    parser.add_argument('--json', action='store_true', help='Output as JSONL')
    args = parser.parse_args()

    if not Path(args.log_file).exists():
        print(f"Error: {args.log_file} not found", file=sys.stderr)
        sys.exit(1)

    process_log(args.log_file, output_json=args.json)

if __name__ == '__main__':
    main()
