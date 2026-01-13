#!/usr/bin/env python3
"""
Filter Claude's stream-json to remove bloated tool outputs.
Keeps: assistant messages, tool calls, errors, session info
Removes: exec_command_output_delta, content_block_delta, full tool results

Usage: claude ... --output-format stream-json | ./filter_log.py | tee log.jsonl
"""

import sys
import json

# Message types to completely skip
SKIP_TYPES = {
    'exec_command_output_delta',
    'content_block_delta',
    'token_count',
    'agent_reasoning_section_break',
}

# Max chars to keep from tool results
MAX_RESULT_CHARS = 500

def truncate_content(content, max_chars=MAX_RESULT_CHARS):
    """Truncate content while preserving structure"""
    if isinstance(content, str):
        if len(content) > max_chars:
            return content[:max_chars] + f'... [{len(content) - max_chars} chars truncated]'
        return content
    elif isinstance(content, list):
        result = []
        total = 0
        for item in content:
            if isinstance(item, dict) and item.get('type') == 'text':
                text = item.get('text', '')
                if total + len(text) > max_chars:
                    remaining = max_chars - total
                    if remaining > 0:
                        result.append({
                            'type': 'text',
                            'text': text[:remaining] + f'... [{len(text) - remaining} chars truncated]'
                        })
                    break
                result.append(item)
                total += len(text)
            else:
                result.append(item)
        return result
    return content

def filter_line(line):
    """Filter a single JSONL line, return None to skip entirely"""
    try:
        data = json.loads(line)
    except json.JSONDecodeError:
        return line  # Pass through non-JSON

    msg = data.get('msg', data)
    msg_type = msg.get('type', '')

    # Skip bloated message types entirely
    if msg_type in SKIP_TYPES:
        return None

    # For user messages (tool results), truncate the content
    if msg_type == 'user':
        inner = msg.get('message', {})
        content = inner.get('content', [])

        if isinstance(content, list):
            new_content = []
            for block in content:
                if block.get('type') == 'tool_result':
                    block = block.copy()
                    block['content'] = truncate_content(block.get('content', ''))
                new_content.append(block)

            # Mutate in place - check if data actually has 'msg' key
            if 'msg' in data and 'message' in data['msg']:
                data['msg']['message']['content'] = new_content
            elif 'message' in data:
                data['message']['content'] = new_content
            else:
                data['content'] = new_content

    return json.dumps(data)

def main():
    try:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue

            filtered = filter_line(line)
            if filtered:
                print(filtered)
                sys.stdout.flush()

    except KeyboardInterrupt:
        sys.exit(0)
    except BrokenPipeError:
        sys.exit(0)

if __name__ == '__main__':
    main()
