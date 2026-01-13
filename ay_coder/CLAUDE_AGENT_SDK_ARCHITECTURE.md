# Claude Agent SDK Architecture - Comprehensive Technical Guide

> **Document Version**: 1.0
> **Last Updated**: 2025-11-19
> **Author**: Technical Research

---

## Table of Contents
1. [Executive Overview](#executive-overview)
2. [Core Architecture](#core-architecture)
3. [SDK Implementations](#sdk-implementations)
4. [Tool System](#tool-system)
5. [Context Management](#context-management)
6. [Extensibility Mechanisms](#extensibility-mechanisms)
7. [Model Context Protocol (MCP)](#model-context-protocol-mcp)
8. [Configuration System](#configuration-system)
9. [Agent Types and Use Cases](#agent-types-and-use-cases)
10. [Implementation Patterns](#implementation-patterns)
11. [Best Practices](#best-practices)
12. [Security Considerations](#security-considerations)

---

## Executive Overview

The **Claude Agent SDK** is a comprehensive framework for building AI-powered agents that can interact with tools, manage context, and execute complex tasks autonomously. It provides multiple language implementations (TypeScript, Python) and integrates with the Model Context Protocol (MCP) for extensibility.

### Key Capabilities
- **Multi-modal agent execution** with tool use
- **Flexible context management** with automatic compaction
- **Extensible tool system** with permission controls
- **Subagent orchestration** for complex tasks
- **MCP integration** for custom tool and service connections
- **Streaming and batch processing** support

---

## Core Architecture

### Architectural Components

```
┌─────────────────────────────────────────────────────────────┐
│                      Agent Runtime Layer                     │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Context    │  │     Tool     │  │  Permission  │     │
│  │   Manager    │  │   Executor   │  │   Manager    │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
├─────────────────────────────────────────────────────────────┤
│                     SDK Interface Layer                      │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │  TypeScript  │  │    Python    │  │     Rust     │     │
│  │     SDK      │  │     SDK      │  │   (Future)   │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
├─────────────────────────────────────────────────────────────┤
│                   Integration Layer (MCP)                    │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ External DB  │  │   Custom     │  │   External   │     │
│  │   Servers    │  │    Tools     │  │     APIs     │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### Agent Execution Model

The agent execution follows this lifecycle:

```
┌─────────────┐
│ User Input  │
└──────┬──────┘
       │
       ▼
┌─────────────────────┐
│ Context Preprocessing│
│ - Load CLAUDE.md    │
│ - Apply hooks       │
│ - Validate input    │
└──────┬──────────────┘
       │
       ▼
┌─────────────────────┐
│  Agent Selection    │
│ - Choose subagent   │
│ - Load skills       │
└──────┬──────────────┘
       │
       ▼
┌─────────────────────┐
│ Tool Resolution     │
│ - Parse tool calls  │
│ - Check permissions │
│ - Validate schemas  │
└──────┬──────────────┘
       │
       ▼
┌─────────────────────┐
│  Tool Execution     │
│ - Isolated context  │
│ - Error handling    │
│ - Result collection │
└──────┬──────────────┘
       │
       ▼
┌─────────────────────┐
│ Response Generation │
│ - Format results    │
│ - Update context    │
│ - Trigger post-hooks│
└──────┬──────────────┘
       │
       ▼
┌─────────────┐
│   Output    │
└─────────────┘
```

---

## SDK Implementations

### TypeScript SDK

**Package**: `@anthropic-ai/sdk`

#### Core Features
- Type-safe API interactions
- Streaming support with event handlers
- Tool execution with `toolRunner()`
- Automatic retry logic
- File upload support (streams, buffers, web File API)

#### Basic Initialization

```typescript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic({
  apiKey: process.env.ANTHROPIC_API_KEY,
});

// Create a message
const message = await client.messages.create({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Hello, Claude' }],
});
```

#### Tool Definition with Zod

```typescript
import { z } from 'zod';
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic();

// Define tool schema
const GetWeatherTool = {
  name: 'get_weather',
  description: 'Get the current weather in a given location',
  input_schema: {
    type: 'object',
    properties: {
      location: {
        type: 'string',
        description: 'The city and state, e.g. San Francisco, CA',
      },
      unit: {
        type: 'string',
        enum: ['celsius', 'fahrenheit'],
        description: 'The unit of temperature',
      },
    },
    required: ['location'],
  },
} as const satisfies Anthropic.Tool;

// Use tool in conversation
const response = await client.messages.create({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  tools: [GetWeatherTool],
  messages: [
    { role: 'user', content: 'What is the weather in San Francisco?' }
  ],
});
```

#### Streaming Example

```typescript
const stream = await client.messages.stream({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Write a haiku' }],
});

// Handle events
stream.on('text', (text) => {
  console.log(text);
});

stream.on('message', (message) => {
  console.log('Final message:', message);
});

await stream.finalMessage();
```

#### Error Handling

```typescript
import Anthropic from '@anthropic-ai/sdk';

try {
  const message = await client.messages.create({
    model: 'claude-3-5-sonnet-20241022',
    max_tokens: 1024,
    messages: [{ role: 'user', content: 'Hello' }],
  });
} catch (error) {
  if (error instanceof Anthropic.APIError) {
    console.error('API Error:', error.status, error.message);
  } else {
    console.error('Unexpected error:', error);
  }
}
```

---

### Python SDK

**Package**: `anthropic`

#### Core Features
- Synchronous and asynchronous clients
- Typed request/response handling (Pydantic)
- Automatic pagination
- Server-Side Events (SSE) streaming
- Token counting
- Message batching

#### Basic Initialization

```python
from anthropic import Anthropic

# Synchronous client
client = Anthropic(
    api_key="my_api_key",  # defaults to os.environ.get("ANTHROPIC_API_KEY")
)

# Asynchronous client
from anthropic import AsyncAnthropic
async_client = AsyncAnthropic()
```

#### Simple Message Creation

```python
message = client.messages.create(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    messages=[
        {"role": "user", "content": "Hello, Claude"}
    ]
)
print(message.content)
```

#### Tool Definition with Beta Decorator

```python
from anthropic import Anthropic
from anthropic.types.beta import BetaToolUnion

client = Anthropic()

# Define a tool function
def get_weather(location: str, unit: str = "fahrenheit") -> dict:
    """Get the current weather in a given location.

    Args:
        location: The city and state, e.g. San Francisco, CA
        unit: The unit of temperature (celsius or fahrenheit)
    """
    # Mock implementation
    return {
        "location": location,
        "temperature": 72,
        "unit": unit,
        "conditions": "sunny"
    }

# Use tool in conversation
response = client.messages.create(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    tools=[
        {
            "name": "get_weather",
            "description": "Get the current weather in a given location",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city and state, e.g. San Francisco, CA"
                    },
                    "unit": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "description": "The unit of temperature"
                    }
                },
                "required": ["location"]
            }
        }
    ],
    messages=[
        {"role": "user", "content": "What's the weather in San Francisco?"}
    ]
)
```

#### Async Streaming

```python
import asyncio
from anthropic import AsyncAnthropic

async def main():
    client = AsyncAnthropic()

    async with client.messages.stream(
        model="claude-3-5-sonnet-20241022",
        max_tokens=1024,
        messages=[{"role": "user", "content": "Write a haiku"}]
    ) as stream:
        async for text in stream.text_stream:
            print(text, end="", flush=True)

    # Get final message
    message = await stream.get_final_message()
    print("\n\nFinal message:", message)

asyncio.run(main())
```

#### Platform-Specific Clients

```python
# AWS Bedrock
from anthropic import AnthropicBedrock

client = AnthropicBedrock(
    aws_region="us-west-2",
)

# Google Vertex AI
from anthropic import AnthropicVertex

client = AnthropicVertex(
    region="us-central1",
    project_id="my-project-id",
)
```

---

## Tool System

### Tool Architecture

Tools in the Claude Agent SDK are first-class citizens with the following characteristics:

1. **Schema-based validation** using JSON Schema
2. **Permission-controlled execution**
3. **Isolated execution context**
4. **Composable and chainable**
5. **Error-resilient with retry logic**

### Tool Definition Structure

```typescript
interface Tool {
  name: string;
  description: string;
  input_schema: {
    type: 'object';
    properties: Record<string, JSONSchema>;
    required?: string[];
  };
}
```

### Built-in Tool Categories

#### File Operations
- `Read` - Read file contents
- `Write` - Write file contents
- `Edit` - Modify existing files
- `Glob` - Pattern-based file search

#### Code Execution
- `Bash` - Execute shell commands
- `BashOutput` - Read background process output
- `KillShell` - Terminate background processes

#### Search and Analysis
- `Grep` - Content search with regex
- `WebFetch` - Fetch and analyze web content
- `WebSearch` - Search the web

#### Agent Orchestration
- `Task` - Launch specialized subagents
- `TodoWrite` - Manage task lists
- `Skill` - Execute custom skills
- `SlashCommand` - Execute slash commands

#### User Interaction
- `AskUserQuestion` - Interactive user prompts

### Tool Permission System

```typescript
interface ToolPermissions {
  permissionMode?: 'allow_all' | 'allow_list' | 'deny_list';
  allowedTools?: string[];
  disallowedTools?: string[];
}

// Example configuration
const agentConfig = {
  permissionMode: 'allow_list',
  allowedTools: ['Read', 'Grep', 'WebFetch'],
  disallowedTools: [],
};
```

### Tool Execution Flow

```
┌──────────────────┐
│  Tool Call       │
│  Requested       │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Schema           │
│ Validation       │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Permission       │
│ Check            │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Pre-execution    │
│ Hooks            │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Isolated         │
│ Execution        │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Result           │
│ Collection       │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Post-execution   │
│ Hooks            │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Return to        │
│ Agent            │
└──────────────────┘
```

### Custom Tool Implementation

#### TypeScript Example

```typescript
import Anthropic from '@anthropic-ai/sdk';

// Define custom tool
const DatabaseQueryTool = {
  name: 'query_database',
  description: 'Execute a SQL query against the database',
  input_schema: {
    type: 'object',
    properties: {
      query: {
        type: 'string',
        description: 'The SQL query to execute',
      },
      database: {
        type: 'string',
        description: 'The database name',
        enum: ['users', 'products', 'orders'],
      },
    },
    required: ['query', 'database'],
  },
} as const satisfies Anthropic.Tool;

// Implement tool handler
async function handleDatabaseQuery(input: { query: string; database: string }) {
  // Validate and sanitize query
  if (!isValidQuery(input.query)) {
    throw new Error('Invalid SQL query');
  }

  // Execute query
  const results = await db.query(input.database, input.query);

  return {
    rows: results,
    count: results.length,
  };
}

// Use in agent
const response = await client.messages.create({
  model: 'claude-3-5-sonnet-20241022',
  max_tokens: 1024,
  tools: [DatabaseQueryTool],
  messages: [
    { role: 'user', content: 'How many users do we have?' }
  ],
});

// Process tool use
if (response.content[0].type === 'tool_use') {
  const toolUse = response.content[0];
  const result = await handleDatabaseQuery(toolUse.input);

  // Continue conversation with result
  const followUp = await client.messages.create({
    model: 'claude-3-5-sonnet-20241022',
    max_tokens: 1024,
    tools: [DatabaseQueryTool],
    messages: [
      { role: 'user', content: 'How many users do we have?' },
      { role: 'assistant', content: response.content },
      {
        role: 'user',
        content: [
          {
            type: 'tool_result',
            tool_use_id: toolUse.id,
            content: JSON.stringify(result),
          },
        ],
      },
    ],
  });
}
```

#### Python Example

```python
from anthropic import Anthropic
from typing import Dict, Any
import json

client = Anthropic()

def query_database(query: str, database: str) -> Dict[str, Any]:
    """Execute a SQL query against the database."""
    # Validate and sanitize
    if not is_valid_query(query):
        raise ValueError("Invalid SQL query")

    # Execute (mock)
    results = db.query(database, query)

    return {
        "rows": results,
        "count": len(results)
    }

# Define tool schema
database_tool = {
    "name": "query_database",
    "description": "Execute a SQL query against the database",
    "input_schema": {
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "The SQL query to execute"
            },
            "database": {
                "type": "string",
                "description": "The database name",
                "enum": ["users", "products", "orders"]
            }
        },
        "required": ["query", "database"]
    }
}

# Initial request
response = client.messages.create(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    tools=[database_tool],
    messages=[
        {"role": "user", "content": "How many users do we have?"}
    ]
)

# Handle tool use
if response.content[0].type == "tool_use":
    tool_use = response.content[0]
    tool_result = query_database(**tool_use.input)

    # Continue conversation
    follow_up = client.messages.create(
        model="claude-3-5-sonnet-20241022",
        max_tokens=1024,
        tools=[database_tool],
        messages=[
            {"role": "user", "content": "How many users do we have?"},
            {"role": "assistant", "content": response.content},
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_use.id,
                        "content": json.dumps(tool_result)
                    }
                ]
            }
        ]
    )
```

---

## Context Management

### Context Window Architecture

The Claude Agent SDK implements sophisticated context management to handle large conversations and prevent context overflow.

#### Key Features
1. **Automatic context compaction** - Summarizes old messages
2. **CLAUDE.md integration** - Project-level context injection
3. **Dynamic window sizing** - Adjusts based on model limits
4. **Semantic indexing** - Retrieves relevant past context
5. **LRU cache** - Removes least-recently-used context

### CLAUDE.md Files

CLAUDE.md files provide persistent context to agents at different scope levels:

#### Project-Level Context

```markdown
# Project: E-Commerce Platform

## Overview
This is a Node.js e-commerce platform using Express, PostgreSQL, and React.

## Architecture
- Backend: Express.js REST API
- Database: PostgreSQL with Prisma ORM
- Frontend: React with TypeScript
- Authentication: JWT with refresh tokens

## Coding Standards
- Use TypeScript strict mode
- Follow ESLint Airbnb config
- Write tests for all business logic
- Use async/await, not callbacks

## Database Schema
- users: id, email, password_hash, created_at
- products: id, name, price, description, inventory
- orders: id, user_id, status, total, created_at
- order_items: id, order_id, product_id, quantity, price

## Environment Variables
- DATABASE_URL: PostgreSQL connection
- JWT_SECRET: Token signing key
- STRIPE_API_KEY: Payment processing
```

Place at: `./.claude/CLAUDE.md`

#### User-Level Context

```markdown
# User Preferences

## Coding Style
- Prefer functional programming patterns
- Use descriptive variable names
- Add JSDoc comments for all functions

## Tools Preference
- Use npm, not yarn
- Prefer TypeScript over JavaScript
- Use Prettier for formatting
```

Place at: `~/.config/claude/CLAUDE.md`

### Context Compaction Strategy

```typescript
interface ContextManager {
  // Add message to context
  addMessage(message: Message): void;

  // Compact context when approaching limit
  compactContext(threshold: number): void;

  // Retrieve relevant context for query
  getRelevantContext(query: string): Message[];

  // Clear context
  clearContext(): void;
}

class ContextManager implements ContextManager {
  private messages: Message[] = [];
  private maxTokens: number;
  private currentTokens: number = 0;

  addMessage(message: Message): void {
    this.messages.push(message);
    this.currentTokens += this.countTokens(message);

    // Compact if approaching limit
    if (this.currentTokens > this.maxTokens * 0.8) {
      this.compactContext(this.maxTokens * 0.6);
    }
  }

  compactContext(targetTokens: number): void {
    // Keep most recent messages
    const recentMessages = this.messages.slice(-10);

    // Summarize older messages
    const olderMessages = this.messages.slice(0, -10);
    const summary = this.summarizeMessages(olderMessages);

    // Replace with compacted version
    this.messages = [
      { role: 'system', content: `Previous conversation summary: ${summary}` },
      ...recentMessages
    ];

    this.currentTokens = this.countTokens(this.messages);
  }

  getRelevantContext(query: string): Message[] {
    // Use semantic search to find relevant messages
    const relevant = this.semanticSearch(query, this.messages);
    return relevant.slice(0, 5);
  }
}
```

### Token Management

```python
from anthropic import Anthropic

client = Anthropic()

# Count tokens in messages
def count_message_tokens(messages):
    total = 0
    for message in messages:
        # Approximate token count (actual implementation varies)
        total += len(message['content'].split()) * 1.3
    return int(total)

# Manage context window
def manage_context(messages, max_tokens=100000):
    current_tokens = count_message_tokens(messages)

    if current_tokens > max_tokens * 0.8:
        # Keep system message and recent messages
        system_msg = [m for m in messages if m['role'] == 'system']
        recent_msgs = messages[-20:]

        # Summarize middle messages
        middle_msgs = messages[len(system_msg):-20]
        summary = summarize_messages(middle_msgs)

        return system_msg + [
            {"role": "system", "content": f"Previous context: {summary}"}
        ] + recent_msgs

    return messages
```

---

## Extensibility Mechanisms

### 1. Hooks System

Hooks allow you to inject custom behavior at various points in the agent execution lifecycle.

#### Hook Types
- **Pre-execution hooks** - Run before tool execution
- **Post-execution hooks** - Run after tool execution
- **Error hooks** - Run when errors occur
- **Context hooks** - Modify context before/after agent runs

#### Hook Configuration

Create hooks in `./.claude/hooks/`:

```bash
# ./.claude/hooks/pre-commit.sh
#!/bin/bash
# Run linter before git commits
npm run lint
```

```bash
# ./.claude/hooks/post-read.sh
#!/bin/bash
# Log all file reads
echo "[$(date)] File read: $1" >> ~/.claude/audit.log
```

#### Programmatic Hooks (TypeScript)

```typescript
interface Hook {
  name: string;
  event: 'pre' | 'post' | 'error';
  handler: (context: HookContext) => Promise<void>;
}

class HookManager {
  private hooks: Map<string, Hook[]> = new Map();

  registerHook(hook: Hook): void {
    const existing = this.hooks.get(hook.event) || [];
    this.hooks.set(hook.event, [...existing, hook]);
  }

  async executeHooks(event: string, context: HookContext): Promise<void> {
    const hooks = this.hooks.get(event) || [];
    for (const hook of hooks) {
      await hook.handler(context);
    }
  }
}

// Usage
const hookManager = new HookManager();

hookManager.registerHook({
  name: 'audit-logger',
  event: 'post',
  handler: async (context) => {
    await logToAudit({
      tool: context.toolName,
      timestamp: new Date(),
      result: context.result,
    });
  },
});
```

### 2. Slash Commands

Custom commands that expand to prompts or trigger workflows.

#### Command Structure

Create in `./.claude/commands/`:

```markdown
<!-- ./.claude/commands/review-pr.md -->
# Review Pull Request

Please review the current pull request:

1. Check code quality and style
2. Verify tests are passing
3. Look for security issues
4. Suggest improvements
5. Provide a summary

Focus on:
- Type safety
- Error handling
- Performance
- Security
```

#### Dynamic Commands (TypeScript)

```typescript
interface SlashCommand {
  name: string;
  description: string;
  handler: (args: string[]) => Promise<string>;
}

class CommandRegistry {
  private commands: Map<string, SlashCommand> = new Map();

  registerCommand(command: SlashCommand): void {
    this.commands.set(command.name, command);
  }

  async executeCommand(name: string, args: string[]): Promise<string> {
    const command = this.commands.get(name);
    if (!command) {
      throw new Error(`Command not found: ${name}`);
    }
    return await command.handler(args);
  }
}

// Register custom commands
const registry = new CommandRegistry();

registry.registerCommand({
  name: 'deploy',
  description: 'Deploy to specified environment',
  handler: async (args) => {
    const env = args[0] || 'staging';
    // Trigger deployment
    await deploy(env);
    return `Deployed to ${env}`;
  },
});
```

### 3. Subagents

Specialized agents for specific tasks, defined in `./.claude/agents/`:

```markdown
<!-- ./.claude/agents/code-reviewer.md -->
# Code Reviewer Agent

You are a code review specialist focused on:

## Responsibilities
- Identifying bugs and code smells
- Ensuring adherence to best practices
- Checking for security vulnerabilities
- Validating test coverage

## Approach
1. Read all modified files
2. Run static analysis tools
3. Check test coverage
4. Review for common vulnerabilities
5. Provide actionable feedback

## Tools Available
- Read, Grep, Bash
- Restricted: No Write, No Edit
```

#### Spawning Subagents

```typescript
// Spawn subagent programmatically
const result = await client.agents.spawn({
  agent: 'code-reviewer',
  task: 'Review the authentication module',
  context: {
    files: ['src/auth/*.ts'],
    focus: 'security',
  },
});
```

### 4. Skills

Reusable capabilities defined in `SKILL.md` files:

```markdown
<!-- ./.claude/skills/database-migration.md -->
# Database Migration Skill

## Description
Create and run database migrations safely.

## Steps
1. Analyze schema changes needed
2. Generate migration file with timestamp
3. Review migration for safety
4. Create rollback migration
5. Run migration in transaction
6. Verify schema matches expectations

## Safety Checks
- No data loss
- Reversible operations
- Backup before execution
```

---

## Model Context Protocol (MCP)

### MCP Architecture

MCP is an open protocol for connecting LLMs with external data sources and tools.

```
┌──────────────────────────────────────────────────────────┐
│                    LLM Application                        │
│                    (Claude Agent)                         │
└────────────────────┬─────────────────────────────────────┘
                     │
                     │ MCP Protocol
                     │
┌────────────────────┴─────────────────────────────────────┐
│                    MCP Layer                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │   Server     │  │   Server     │  │   Server     │   │
│  │  Discovery   │  │  Registry    │  │  Auth        │   │
│  └──────────────┘  └──────────────┘  └──────────────┘   │
└────────────────────┬─────────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        │            │            │
        ▼            ▼            ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  Database   │ │   File      │ │  External   │
│   Server    │ │   System    │ │    API      │
└─────────────┘ └─────────────┘ └─────────────┘
```

### MCP Server Implementation

#### TypeScript MCP Server

```typescript
import { MCPServer, Tool } from '@modelcontextprotocol/sdk';

class DatabaseMCPServer extends MCPServer {
  constructor() {
    super({
      name: 'database-server',
      version: '1.0.0',
      description: 'MCP server for database access',
    });

    this.registerTools();
  }

  private registerTools(): void {
    this.addTool({
      name: 'query',
      description: 'Execute a SQL query',
      inputSchema: {
        type: 'object',
        properties: {
          query: { type: 'string' },
          database: { type: 'string' },
        },
        required: ['query', 'database'],
      },
      handler: async (input) => {
        return await this.executeQuery(input.query, input.database);
      },
    });

    this.addTool({
      name: 'get_schema',
      description: 'Get database schema information',
      inputSchema: {
        type: 'object',
        properties: {
          database: { type: 'string' },
        },
        required: ['database'],
      },
      handler: async (input) => {
        return await this.getSchema(input.database);
      },
    });
  }

  private async executeQuery(query: string, database: string) {
    // Implementation
  }

  private async getSchema(database: string) {
    // Implementation
  }
}

// Start server
const server = new DatabaseMCPServer();
server.listen(3000);
```

#### Python MCP Server

```python
from mcp import MCPServer, Tool
from typing import Dict, Any

class DatabaseMCPServer(MCPServer):
    def __init__(self):
        super().__init__(
            name="database-server",
            version="1.0.0",
            description="MCP server for database access"
        )
        self.register_tools()

    def register_tools(self):
        @self.tool(
            name="query",
            description="Execute a SQL query",
            input_schema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "database": {"type": "string"}
                },
                "required": ["query", "database"]
            }
        )
        async def query_handler(query: str, database: str) -> Dict[str, Any]:
            return await self.execute_query(query, database)

        @self.tool(
            name="get_schema",
            description="Get database schema information",
            input_schema={
                "type": "object",
                "properties": {
                    "database": {"type": "string"}
                },
                "required": ["database"]
            }
        )
        async def schema_handler(database: str) -> Dict[str, Any]:
            return await self.get_schema(database)

    async def execute_query(self, query: str, database: str):
        # Implementation
        pass

    async def get_schema(self, database: str):
        # Implementation
        pass

# Start server
if __name__ == "__main__":
    server = DatabaseMCPServer()
    server.run(port=3000)
```

### MCP Client Configuration

Configure MCP servers in `./.claude/mcp-config.json`:

```json
{
  "mcpServers": {
    "database": {
      "url": "http://localhost:3000",
      "apiKey": "${DATABASE_MCP_KEY}",
      "enabled": true
    },
    "filesystem": {
      "url": "http://localhost:3001",
      "enabled": true
    },
    "slack": {
      "url": "http://localhost:3002",
      "apiKey": "${SLACK_MCP_KEY}",
      "enabled": false
    }
  }
}
```

### MCP Protocol Flow

```
Agent                           MCP Server
  │                                  │
  │  1. Discover available tools     │
  ├─────────────────────────────────>│
  │                                  │
  │  2. Tool list response           │
  │<─────────────────────────────────┤
  │                                  │
  │  3. Execute tool (with auth)     │
  ├─────────────────────────────────>│
  │                                  │
  │  4. Validate request             │
  │                                  │
  │  5. Process tool execution       │
  │                                  │
  │  6. Return results               │
  │<─────────────────────────────────┤
  │                                  │
```

---

## Configuration System

### Configuration Hierarchy

```
1. Environment Variables (highest priority)
   ↓
2. Runtime Configuration
   ↓
3. Project Config (./.claude/config.yaml)
   ↓
4. User Config (~/.config/claude/config.yaml)
   ↓
5. Global Defaults (lowest priority)
```

### Project Configuration

`./.claude/config.yaml`:

```yaml
# Agent Configuration
agent:
  model: "claude-3-5-sonnet-20241022"
  max_tokens: 4096
  temperature: 0.7

# Tool Permissions
tools:
  permission_mode: "allow_list"
  allowed_tools:
    - Read
    - Write
    - Grep
    - Bash
    - WebFetch
  disallowed_tools:
    - Write # Explicitly blocked even in allow_list

# Context Management
context:
  max_tokens: 100000
  auto_compact: true
  compact_threshold: 0.8

# MCP Servers
mcp:
  enabled: true
  servers:
    - name: database
      url: http://localhost:3000
      enabled: true
    - name: filesystem
      url: http://localhost:3001
      enabled: true

# Hooks
hooks:
  pre_execution:
    - name: linter
      command: "npm run lint"
      enabled: true
  post_execution:
    - name: audit_log
      command: "./hooks/audit.sh"
      enabled: true

# Security
security:
  allow_network: true
  allow_file_write: true
  sandbox_mode: false
  max_file_size: 10485760  # 10MB
```

### User Configuration

`~/.config/claude/config.yaml`:

```yaml
# User Preferences
preferences:
  code_style: "functional"
  verbosity: "normal"
  auto_commit: false

# Default Tools
tools:
  favorites:
    - Grep
    - Read
    - WebFetch

# API Keys (use environment variables instead)
# DO NOT commit API keys
api:
  anthropic_key: "${ANTHROPIC_API_KEY}"

# Editor Integration
editor:
  type: "vscode"
  format_on_save: true
```

### Environment Variables

```bash
# API Authentication
export ANTHROPIC_API_KEY="sk-ant-..."
export ANTHROPIC_BASE_URL="https://api.anthropic.com"

# MCP Server Keys
export DATABASE_MCP_KEY="..."
export SLACK_MCP_KEY="..."

# Configuration Overrides
export CLAUDE_MODEL="claude-3-5-sonnet-20241022"
export CLAUDE_MAX_TOKENS="8192"
export CLAUDE_SANDBOX_MODE="true"

# Logging
export CLAUDE_LOG_LEVEL="debug"
export CLAUDE_LOG_FILE="/var/log/claude-agent.log"
```

### Runtime Configuration (TypeScript)

```typescript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic({
  apiKey: process.env.ANTHROPIC_API_KEY,
  baseURL: process.env.ANTHROPIC_BASE_URL,
  maxRetries: 3,
  timeout: 60000,
  defaultHeaders: {
    'X-Custom-Header': 'value',
  },
});

// Agent-specific config
const agentConfig = {
  model: 'claude-3-5-sonnet-20241022',
  maxTokens: 4096,
  temperature: 0.7,
  systemPrompt: 'You are a helpful coding assistant.',
  tools: [/* tool definitions */],
  toolPermissions: {
    permissionMode: 'allow_list',
    allowedTools: ['Read', 'Grep', 'WebFetch'],
  },
};
```

---

## Agent Types and Use Cases

### 1. Coding Agents

#### SRE Diagnostic Tool
```yaml
name: sre-diagnostics
description: Diagnose production issues
capabilities:
  - Read logs
  - Query metrics
  - Check service health
  - Analyze error patterns
tools:
  - Bash
  - Grep
  - WebFetch
  - MCP:monitoring
```

#### Security Review Bot
```yaml
name: security-reviewer
description: Automated security analysis
capabilities:
  - Static code analysis
  - Dependency vulnerability scan
  - Secret detection
  - OWASP compliance check
tools:
  - Read
  - Grep
  - Bash
restrictions:
  - No file writes
  - No external network calls
```

#### Code Review Assistant
```yaml
name: code-reviewer
description: Comprehensive code review
capabilities:
  - Style checking
  - Bug detection
  - Test coverage analysis
  - Documentation review
tools:
  - Read
  - Grep
  - Bash
focus_areas:
  - Type safety
  - Error handling
  - Performance
  - Maintainability
```

### 2. Business Agents

#### Legal Contract Reviewer
```yaml
name: legal-reviewer
description: Contract analysis and risk assessment
capabilities:
  - Clause extraction
  - Risk identification
  - Compliance checking
  - Comparison with templates
tools:
  - Read
  - WebFetch
  - MCP:legal-database
domain_knowledge:
  - Contract law
  - Regulatory compliance
```

#### Financial Analysis Assistant
```yaml
name: financial-analyst
description: Financial data analysis
capabilities:
  - Financial statement analysis
  - Ratio calculations
  - Trend identification
  - Risk assessment
tools:
  - Read
  - MCP:financial-database
  - WebFetch
output_formats:
  - Markdown reports
  - JSON data
  - Excel-compatible CSV
```

---

## Implementation Patterns

### Pattern 1: Tool Chaining

Execute multiple tools in sequence, passing results between them:

```typescript
async function analyzeCodeQuality(filePath: string) {
  // Step 1: Read file
  const fileContent = await agent.executeTool('Read', {
    file_path: filePath
  });

  // Step 2: Run linter
  const lintResults = await agent.executeTool('Bash', {
    command: `eslint ${filePath} --format json`
  });

  // Step 3: Check test coverage
  const coverage = await agent.executeTool('Bash', {
    command: `jest --coverage --testPathPattern=${filePath}`
  });

  // Step 4: Analyze and report
  return {
    file: filePath,
    content: fileContent,
    lintIssues: JSON.parse(lintResults),
    coverage: JSON.parse(coverage),
  };
}
```

### Pattern 2: Multi-Agent Coordination

Spawn multiple specialized agents for complex tasks:

```typescript
async function conductFullCodeReview(prNumber: number) {
  // Spawn parallel agents
  const [security, quality, tests, docs] = await Promise.all([
    agent.spawnSubagent('security-reviewer', {
      task: `Review PR #${prNumber} for security issues`
    }),
    agent.spawnSubagent('code-quality', {
      task: `Analyze PR #${prNumber} for code quality`
    }),
    agent.spawnSubagent('test-validator', {
      task: `Validate tests in PR #${prNumber}`
    }),
    agent.spawnSubagent('doc-checker', {
      task: `Check documentation in PR #${prNumber}`
    }),
  ]);

  // Aggregate results
  return {
    security: security.findings,
    quality: quality.issues,
    tests: tests.coverage,
    documentation: docs.status,
    approved: allPassed([security, quality, tests, docs]),
  };
}
```

### Pattern 3: Iterative Refinement

Progressively improve results through multiple iterations:

```typescript
async function generateOptimizedCode(specification: string) {
  let code = '';
  let iteration = 0;
  const maxIterations = 5;

  while (iteration < maxIterations) {
    // Generate or refine code
    code = await agent.generate({
      prompt: iteration === 0
        ? `Generate code for: ${specification}`
        : `Improve this code: ${code}`,
    });

    // Run tests
    const testResults = await agent.executeTool('Bash', {
      command: 'npm test'
    });

    // Check quality
    const lintResults = await agent.executeTool('Bash', {
      command: 'eslint --format json'
    });

    // If all checks pass, we're done
    if (testResults.passed && lintResults.errors.length === 0) {
      break;
    }

    iteration++;
  }

  return code;
}
```

### Pattern 4: Context-Aware Tool Selection

Dynamically select tools based on context:

```typescript
class ContextAwareAgent {
  selectTools(task: string, context: AgentContext): Tool[] {
    const tools: Tool[] = [];

    // Always include basic tools
    tools.push(ReadTool, GrepTool);

    // Add based on task type
    if (task.includes('web') || task.includes('fetch')) {
      tools.push(WebFetchTool);
    }

    if (task.includes('code') || task.includes('implement')) {
      tools.push(WriteTool, BashTool);
    }

    if (task.includes('database') || task.includes('query')) {
      tools.push(MCPDatabaseTool);
    }

    // Add based on permissions
    const allowedTools = tools.filter(tool =>
      context.permissions.allowedTools.includes(tool.name)
    );

    return allowedTools;
  }
}
```

### Pattern 5: Error Recovery

Handle errors gracefully with retry logic:

```typescript
async function resilientToolExecution<T>(
  toolName: string,
  input: any,
  options: {
    maxRetries?: number;
    backoff?: number;
    fallback?: () => Promise<T>;
  } = {}
): Promise<T> {
  const { maxRetries = 3, backoff = 1000, fallback } = options;

  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      return await agent.executeTool(toolName, input);
    } catch (error) {
      console.error(`Attempt ${attempt + 1} failed:`, error);

      if (attempt === maxRetries - 1) {
        // Last attempt failed
        if (fallback) {
          console.log('Using fallback strategy');
          return await fallback();
        }
        throw error;
      }

      // Wait before retry with exponential backoff
      await sleep(backoff * Math.pow(2, attempt));
    }
  }

  throw new Error('All retries exhausted');
}
```

---

## Best Practices

### 1. Tool Design

- **Single Responsibility**: Each tool should do one thing well
- **Clear Schemas**: Provide detailed JSON schemas with descriptions
- **Validate Inputs**: Always validate and sanitize inputs
- **Idempotent Operations**: Tools should be safe to retry
- **Error Messages**: Return clear, actionable error messages

### 2. Context Management

- **Use CLAUDE.md**: Provide project context upfront
- **Prune Regularly**: Don't let context grow unbounded
- **Semantic Chunking**: Break large contexts into logical sections
- **Cache Effectively**: Reuse context when possible
- **Monitor Usage**: Track token consumption

### 3. Security

- **Principle of Least Privilege**: Grant minimal permissions
- **Validate All Inputs**: Never trust user input
- **Audit Tool Usage**: Log all tool executions
- **Secure Credentials**: Use environment variables, not config files
- **Sandbox Execution**: Isolate untrusted code

### 4. Performance

- **Parallel Execution**: Run independent operations concurrently
- **Lazy Loading**: Load tools only when needed
- **Cache Results**: Avoid redundant computations
- **Stream Responses**: Use streaming for long-running operations
- **Optimize Context**: Keep context minimal and relevant

### 5. Testing

- **Unit Test Tools**: Test each tool independently
- **Integration Tests**: Test tool chains and workflows
- **Mock External Services**: Don't depend on external APIs in tests
- **Test Error Paths**: Verify error handling works correctly
- **Measure Coverage**: Aim for high test coverage

### 6. Monitoring

- **Log Everything**: Comprehensive logging aids debugging
- **Track Metrics**: Monitor token usage, latency, errors
- **Alert on Failures**: Set up alerts for critical failures
- **Analyze Patterns**: Identify common issues and optimize
- **User Feedback**: Collect and act on user feedback

---

## Security Considerations

### Authentication

```typescript
// Use environment variables
const client = new Anthropic({
  apiKey: process.env.ANTHROPIC_API_KEY, // Never hardcode
});

// Validate API keys
function validateApiKey(key: string): boolean {
  return key.startsWith('sk-ant-') && key.length > 50;
}
```

### Input Validation

```typescript
function validateToolInput(input: any, schema: JSONSchema): boolean {
  const validator = new JSONSchemaValidator(schema);
  const result = validator.validate(input);

  if (!result.valid) {
    throw new ValidationError(result.errors);
  }

  // Additional sanitization
  sanitizeInput(input);

  return true;
}

function sanitizeInput(input: any): void {
  // Remove dangerous patterns
  const dangerous = /<script|javascript:|onerror=/gi;

  if (typeof input === 'string' && dangerous.test(input)) {
    throw new SecurityError('Dangerous input detected');
  }

  // Recursively sanitize objects
  if (typeof input === 'object') {
    for (const key in input) {
      sanitizeInput(input[key]);
    }
  }
}
```

### Permission System

```typescript
interface ToolPermission {
  tool: string;
  allowed: boolean;
  requiredRole?: string;
  auditLog: boolean;
}

class PermissionManager {
  checkPermission(
    tool: string,
    user: User,
    permissions: ToolPermission[]
  ): boolean {
    const permission = permissions.find(p => p.tool === tool);

    if (!permission || !permission.allowed) {
      throw new PermissionError(`Tool ${tool} not allowed`);
    }

    if (permission.requiredRole && user.role !== permission.requiredRole) {
      throw new PermissionError(`Insufficient role for ${tool}`);
    }

    if (permission.auditLog) {
      this.logAccess(tool, user);
    }

    return true;
  }
}
```

### Sandboxing

```typescript
// Execute code in isolated environment
async function executeSandboxed(code: string): Promise<any> {
  const sandbox = new VM({
    timeout: 5000,
    sandbox: {
      // Provide minimal safe globals
      console: sandboxConsole,
      Math: Math,
    },
  });

  try {
    return await sandbox.run(code);
  } catch (error) {
    throw new SandboxError('Code execution failed', error);
  }
}
```

### Rate Limiting

```typescript
class RateLimiter {
  private requests: Map<string, number[]> = new Map();

  checkLimit(userId: string, limit: number, windowMs: number): boolean {
    const now = Date.now();
    const userRequests = this.requests.get(userId) || [];

    // Remove old requests outside window
    const recentRequests = userRequests.filter(
      time => now - time < windowMs
    );

    if (recentRequests.length >= limit) {
      throw new RateLimitError('Rate limit exceeded');
    }

    recentRequests.push(now);
    this.requests.set(userId, recentRequests);

    return true;
  }
}
```

---

## Conclusion

The Claude Agent SDK provides a powerful, flexible framework for building AI-powered agents. Key takeaways:

1. **Modular Architecture**: Compose tools, hooks, and subagents flexibly
2. **Extensibility**: MCP enables unlimited custom integrations
3. **Context Management**: Sophisticated handling of large conversations
4. **Security**: Built-in permission system and sandboxing
5. **Performance**: Streaming, caching, and parallel execution
6. **Best Practices**: Clear patterns for common use cases

### Next Steps

1. **Explore the SDKs**: Try TypeScript or Python implementations
2. **Build Custom Tools**: Create tools specific to your domain
3. **Implement MCP Servers**: Connect external data sources
4. **Configure Agents**: Set up specialized agents for your workflow
5. **Monitor and Optimize**: Track usage and improve performance

### Resources

- **TypeScript SDK**: https://github.com/anthropics/anthropic-sdk-typescript
- **Python SDK**: https://github.com/anthropics/anthropic-sdk-python
- **MCP Specification**: https://github.com/modelcontextprotocol
- **Documentation**: https://platform.claude.com/docs

---

*This document provides a comprehensive technical overview of the Claude Agent SDK architecture. For the most current information, consult the official documentation and SDK repositories.*
