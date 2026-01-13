# Claude Agent SDK - Quick Reference Guide

A concise reference for working with the Claude Agent SDK.

---

## Installation

### TypeScript
```bash
npm install @anthropic-ai/sdk
```

### Python
```bash
pip install anthropic
```

---

## Basic Usage

### TypeScript

```typescript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic({
  apiKey: process.env.ANTHROPIC_API_KEY,
});

const message = await client.messages.create({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Hello!' }],
});
```

### Python

```python
from anthropic import Anthropic

client = Anthropic(api_key="your-api-key")

message = client.messages.create(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Hello!"}]
)
```

---

## Tool Definition

### TypeScript

```typescript
const tool = {
  name: 'get_weather',
  description: 'Get weather for a location',
  input_schema: {
    type: 'object',
    properties: {
      location: { type: 'string' },
      unit: { type: 'string', enum: ['celsius', 'fahrenheit'] },
    },
    required: ['location'],
  },
};

const response = await client.messages.create({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  tools: [tool],
  messages: [{ role: 'user', content: 'Weather in SF?' }],
});
```

### Python

```python
tool = {
    "name": "get_weather",
    "description": "Get weather for a location",
    "input_schema": {
        "type": "object",
        "properties": {
            "location": {"type": "string"},
            "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
        },
        "required": ["location"]
    }
}

response = client.messages.create(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    tools=[tool],
    messages=[{"role": "user", "content": "Weather in SF?"}]
)
```

---

## Streaming

### TypeScript

```typescript
const stream = await client.messages.stream({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Tell me a story' }],
});

stream.on('text', (text) => console.log(text));
await stream.finalMessage();
```

### Python

```python
with client.messages.stream(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Tell me a story"}]
) as stream:
    for text in stream.text_stream:
        print(text, end="", flush=True)
```

---

## Key Concepts

### 1. Messages
- **Role**: `user`, `assistant`, or `system`
- **Content**: Text or structured content (images, documents)

### 2. Tools
- Extend agent capabilities
- Defined with JSON Schema
- Execute custom functions

### 3. Context
- Managed automatically
- Use `CLAUDE.md` for project context
- Compaction prevents overflow

### 4. Permissions
- `allow_all`: All tools available
- `allow_list`: Only specified tools
- `deny_list`: Block specific tools

---

## File Structure

```
project/
├── .claude/
│   ├── config.yaml           # Project configuration
│   ├── CLAUDE.md             # Project context
│   ├── agents/               # Custom subagents
│   │   └── reviewer.md
│   ├── commands/             # Slash commands
│   │   └── deploy.md
│   ├── hooks/                # Event hooks
│   │   ├── pre-commit.sh
│   │   └── post-read.sh
│   └── mcp-config.json       # MCP server config
└── src/
    └── ...
```

---

## Configuration

### Project Config (`.claude/config.yaml`)

```yaml
agent:
  model: "claude-3-5-sonnet-20241022"
  max_tokens: 4096

tools:
  permission_mode: "allow_list"
  allowed_tools:
    - Read
    - Write
    - Bash

context:
  max_tokens: 100000
  auto_compact: true

mcp:
  enabled: true
  servers:
    - name: database
      url: http://localhost:3000
```

### Environment Variables

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export CLAUDE_MODEL="claude-3-5-sonnet-20241022"
export CLAUDE_MAX_TOKENS="8192"
```

---

## Common Patterns

### 1. Tool Chaining

```typescript
// Read → Analyze → Report
const content = await readFile('data.json');
const analysis = await analyzeData(content);
const report = await generateReport(analysis);
```

### 2. Error Handling

```typescript
try {
  const result = await agent.executeTool('query', input);
} catch (error) {
  if (error instanceof Anthropic.APIError) {
    console.error(`API Error: ${error.status}`);
  }
  // Fallback strategy
}
```

### 3. Retry Logic

```typescript
async function withRetry<T>(
  fn: () => Promise<T>,
  maxRetries = 3
): Promise<T> {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fn();
    } catch (error) {
      if (i === maxRetries - 1) throw error;
      await sleep(1000 * Math.pow(2, i));
    }
  }
}
```

### 4. Context Management

```typescript
function manageContext(messages: Message[], maxTokens: number) {
  const currentTokens = countTokens(messages);

  if (currentTokens > maxTokens * 0.8) {
    // Keep recent messages, summarize old
    const recent = messages.slice(-10);
    const summary = summarize(messages.slice(0, -10));
    return [summary, ...recent];
  }

  return messages;
}
```

---

## MCP Server Example

### TypeScript

```typescript
import { MCPServer } from '@modelcontextprotocol/sdk';

const server = new MCPServer({
  name: 'my-server',
  version: '1.0.0',
});

server.addTool({
  name: 'custom_tool',
  description: 'Does something useful',
  inputSchema: {
    type: 'object',
    properties: {
      input: { type: 'string' },
    },
  },
  handler: async (input) => {
    return { result: process(input) };
  },
});

server.listen(3000);
```

---

## Built-in Tools

| Tool | Description |
|------|-------------|
| `Read` | Read file contents |
| `Write` | Write file contents |
| `Edit` | Modify files |
| `Glob` | Find files by pattern |
| `Grep` | Search file contents |
| `Bash` | Execute shell commands |
| `WebFetch` | Fetch web content |
| `Task` | Spawn subagents |

---

## Best Practices

1. **Security**
   - Use environment variables for secrets
   - Validate all inputs
   - Apply principle of least privilege

2. **Performance**
   - Use streaming for long responses
   - Execute independent operations in parallel
   - Cache when possible

3. **Context**
   - Keep context relevant and minimal
   - Use CLAUDE.md for project info
   - Enable auto-compaction

4. **Tools**
   - Single responsibility per tool
   - Clear, detailed schemas
   - Idempotent operations

5. **Error Handling**
   - Catch and handle API errors
   - Implement retry logic
   - Provide fallback strategies

---

## Common Issues

### Issue: Rate Limiting
**Solution**: Implement exponential backoff, use streaming

### Issue: Context Overflow
**Solution**: Enable auto-compaction, manage context manually

### Issue: Tool Permission Errors
**Solution**: Check `allowed_tools` in config, verify permission mode

### Issue: API Authentication
**Solution**: Verify `ANTHROPIC_API_KEY` environment variable

---

## Models

| Model | Context | Use Case |
|-------|---------|----------|
| `claude-3-5-sonnet-20241022` | 200K | Most tasks |
| `claude-3-5-haiku-20241022` | 200K | Fast, cost-effective |
| `claude-3-opus-20240229` | 200K | Complex reasoning |

---

## Resources

- **Docs**: https://platform.claude.com/docs
- **TypeScript SDK**: https://github.com/anthropics/anthropic-sdk-typescript
- **Python SDK**: https://github.com/anthropics/anthropic-sdk-python
- **MCP**: https://github.com/modelcontextprotocol

---

## Cheat Sheet

```bash
# Install
npm install @anthropic-ai/sdk  # TypeScript
pip install anthropic          # Python

# Set API key
export ANTHROPIC_API_KEY="sk-ant-..."

# Run agent with config
claude --config .claude/config.yaml

# List available tools
claude tools list

# Test MCP server
curl http://localhost:3000/health
```

---

*Quick reference for Claude Agent SDK. See CLAUDE_AGENT_SDK_ARCHITECTURE.md for comprehensive details.*
