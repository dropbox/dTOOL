# Claude Agent SDK - Implementation Examples

Practical, production-ready examples for building with the Claude Agent SDK.

---

## Table of Contents

1. [Basic Agent](#basic-agent)
2. [Code Review Agent](#code-review-agent)
3. [Database Query Agent](#database-query-agent)
4. [Multi-Agent Orchestration](#multi-agent-orchestration)
5. [MCP Server](#mcp-server)
6. [Custom Tool Development](#custom-tool-development)
7. [Error Handling & Retry](#error-handling--retry)
8. [Streaming Responses](#streaming-responses)

---

## Basic Agent

### TypeScript

```typescript
import Anthropic from '@anthropic-ai/sdk';

class BasicAgent {
  private client: Anthropic;

  constructor(apiKey: string) {
    this.client = new Anthropic({ apiKey });
  }

  async chat(userMessage: string): Promise<string> {
    const response = await this.client.messages.create({
      model: 'claude-3-5-sonnet-20241022',
      max_tokens: 1024,
      messages: [
        {
          role: 'user',
          content: userMessage,
        },
      ],
    });

    return response.content[0].type === 'text'
      ? response.content[0].text
      : '';
  }
}

// Usage
const agent = new BasicAgent(process.env.ANTHROPIC_API_KEY!);
const response = await agent.chat('Hello, how are you?');
console.log(response);
```

### Python

```python
from anthropic import Anthropic
from typing import Optional

class BasicAgent:
    def __init__(self, api_key: str):
        self.client = Anthropic(api_key=api_key)

    def chat(self, user_message: str) -> str:
        response = self.client.messages.create(
            model="claude-3-5-sonnet-20241022",
            max_tokens=1024,
            messages=[
                {"role": "user", "content": user_message}
            ]
        )

        if response.content[0].type == "text":
            return response.content[0].text
        return ""

# Usage
import os
agent = BasicAgent(os.environ["ANTHROPIC_API_KEY"])
response = agent.chat("Hello, how are you?")
print(response)
```

---

## Code Review Agent

### TypeScript

```typescript
import Anthropic from '@anthropic-ai/sdk';
import { readFileSync } from 'fs';
import { glob } from 'glob';

interface ReviewResult {
  file: string;
  issues: Issue[];
  score: number;
}

interface Issue {
  line: number;
  severity: 'error' | 'warning' | 'info';
  message: string;
  suggestion?: string;
}

class CodeReviewAgent {
  private client: Anthropic;

  constructor(apiKey: string) {
    this.client = new Anthropic({ apiKey });
  }

  async reviewFile(filePath: string): Promise<ReviewResult> {
    const code = readFileSync(filePath, 'utf-8');

    const reviewTool = {
      name: 'analyze_code',
      description: 'Analyze code for issues and best practices',
      input_schema: {
        type: 'object',
        properties: {
          issues: {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                line: { type: 'number' },
                severity: { type: 'string', enum: ['error', 'warning', 'info'] },
                message: { type: 'string' },
                suggestion: { type: 'string' },
              },
              required: ['line', 'severity', 'message'],
            },
          },
          score: { type: 'number', minimum: 0, maximum: 100 },
        },
        required: ['issues', 'score'],
      },
    };

    const response = await this.client.messages.create({
      model: 'claude-3-5-sonnet-20241022',
      max_tokens: 4096,
      tools: [reviewTool],
      messages: [
        {
          role: 'user',
          content: `Review this code and provide feedback:\n\n\`\`\`\n${code}\n\`\`\`\n\nLook for:
- Bug patterns
- Security issues
- Performance problems
- Code style violations
- Maintainability concerns`,
        },
      ],
    });

    // Extract tool use results
    const toolUse = response.content.find(c => c.type === 'tool_use');
    if (toolUse?.type === 'tool_use') {
      return {
        file: filePath,
        issues: toolUse.input.issues,
        score: toolUse.input.score,
      };
    }

    return { file: filePath, issues: [], score: 100 };
  }

  async reviewProject(pattern: string): Promise<ReviewResult[]> {
    const files = await glob(pattern);
    const reviews = await Promise.all(
      files.map(file => this.reviewFile(file))
    );
    return reviews;
  }

  generateReport(reviews: ReviewResult[]): string {
    let report = '# Code Review Report\n\n';

    for (const review of reviews) {
      report += `## ${review.file} (Score: ${review.score}/100)\n\n`;

      if (review.issues.length === 0) {
        report += 'No issues found.\n\n';
        continue;
      }

      for (const issue of review.issues) {
        const emoji = issue.severity === 'error' ? '❌' : issue.severity === 'warning' ? '⚠️' : 'ℹ️';
        report += `${emoji} **Line ${issue.line}** (${issue.severity}): ${issue.message}\n`;
        if (issue.suggestion) {
          report += `   Suggestion: ${issue.suggestion}\n`;
        }
        report += '\n';
      }
    }

    return report;
  }
}

// Usage
const agent = new CodeReviewAgent(process.env.ANTHROPIC_API_KEY!);
const reviews = await agent.reviewProject('src/**/*.ts');
const report = agent.generateReport(reviews);
console.log(report);
```

---

## Database Query Agent

### Python

```python
from anthropic import Anthropic
from typing import Dict, Any, List
import json
import sqlite3

class DatabaseQueryAgent:
    def __init__(self, api_key: str, db_path: str):
        self.client = Anthropic(api_key=api_key)
        self.db_path = db_path

    def get_schema(self) -> str:
        """Get database schema information."""
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()

        cursor.execute("""
            SELECT sql FROM sqlite_master
            WHERE type='table'
        """)

        schema = "\n".join([row[0] for row in cursor.fetchall()])
        conn.close()
        return schema

    def execute_query(self, query: str) -> List[Dict[str, Any]]:
        """Execute SQL query and return results."""
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        cursor = conn.cursor()

        try:
            cursor.execute(query)
            results = [dict(row) for row in cursor.fetchall()]
            conn.close()
            return results
        except Exception as e:
            conn.close()
            raise Exception(f"Query failed: {str(e)}")

    def natural_language_query(self, question: str) -> Dict[str, Any]:
        """Convert natural language to SQL and execute."""
        schema = self.get_schema()

        # Define SQL generation tool
        sql_tool = {
            "name": "generate_sql",
            "description": "Generate SQL query from natural language",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The SQL query"
                    },
                    "explanation": {
                        "type": "string",
                        "description": "Explanation of the query"
                    }
                },
                "required": ["query", "explanation"]
            }
        }

        # Generate SQL
        response = self.client.messages.create(
            model="claude-3-5-sonnet-20241022",
            max_tokens=1024,
            tools=[sql_tool],
            messages=[{
                "role": "user",
                "content": f"""Given this database schema:

{schema}

Convert this question to SQL: {question}

Generate a safe, read-only SELECT query."""
            }]
        )

        # Extract SQL
        tool_use = next(
            (c for c in response.content if c.type == "tool_use"),
            None
        )

        if not tool_use:
            return {"error": "Could not generate SQL"}

        sql_query = tool_use.input["query"]
        explanation = tool_use.input["explanation"]

        # Execute query
        try:
            results = self.execute_query(sql_query)
            return {
                "question": question,
                "sql": sql_query,
                "explanation": explanation,
                "results": results,
                "count": len(results)
            }
        except Exception as e:
            return {
                "question": question,
                "sql": sql_query,
                "error": str(e)
            }

# Usage
import os
agent = DatabaseQueryAgent(
    api_key=os.environ["ANTHROPIC_API_KEY"],
    db_path="./database.db"
)

result = agent.natural_language_query(
    "How many users signed up in the last 30 days?"
)

print(f"SQL: {result['sql']}")
print(f"Results: {len(result['results'])} rows")
print(json.dumps(result['results'][:5], indent=2))
```

---

## Multi-Agent Orchestration

### TypeScript

```typescript
import Anthropic from '@anthropic-ai/sdk';

interface AgentConfig {
  name: string;
  systemPrompt: string;
  tools: Anthropic.Tool[];
}

class MultiAgentOrchestrator {
  private client: Anthropic;
  private agents: Map<string, AgentConfig>;

  constructor(apiKey: string) {
    this.client = new Anthropic({ apiKey });
    this.agents = new Map();
  }

  registerAgent(config: AgentConfig): void {
    this.agents.set(config.name, config);
  }

  async executeAgent(
    agentName: string,
    task: string
  ): Promise<string> {
    const agent = this.agents.get(agentName);
    if (!agent) {
      throw new Error(`Agent ${agentName} not found`);
    }

    const response = await this.client.messages.create({
      model: 'claude-3-5-sonnet-20241022',
      max_tokens: 4096,
      system: agent.systemPrompt,
      tools: agent.tools,
      messages: [{ role: 'user', content: task }],
    });

    return response.content
      .filter(c => c.type === 'text')
      .map(c => (c as Anthropic.TextBlock).text)
      .join('\n');
  }

  async orchestrate(
    task: string,
    agentSequence: string[]
  ): Promise<Record<string, string>> {
    const results: Record<string, string> = {};

    for (const agentName of agentSequence) {
      const previousResults = Object.entries(results)
        .map(([name, result]) => `${name}: ${result}`)
        .join('\n\n');

      const enhancedTask = previousResults
        ? `${task}\n\nPrevious results:\n${previousResults}`
        : task;

      results[agentName] = await this.executeAgent(
        agentName,
        enhancedTask
      );
    }

    return results;
  }

  async executeParallel(
    task: string,
    agentNames: string[]
  ): Promise<Record<string, string>> {
    const promises = agentNames.map(async name => ({
      name,
      result: await this.executeAgent(name, task),
    }));

    const results = await Promise.all(promises);

    return results.reduce(
      (acc, { name, result }) => ({ ...acc, [name]: result }),
      {}
    );
  }
}

// Setup
const orchestrator = new MultiAgentOrchestrator(
  process.env.ANTHROPIC_API_KEY!
);

// Register specialized agents
orchestrator.registerAgent({
  name: 'security-analyst',
  systemPrompt: `You are a security expert. Analyze code for vulnerabilities,
focusing on OWASP Top 10, authentication issues, and data exposure.`,
  tools: [],
});

orchestrator.registerAgent({
  name: 'performance-analyst',
  systemPrompt: `You are a performance optimization expert. Identify bottlenecks,
inefficient algorithms, and resource usage issues.`,
  tools: [],
});

orchestrator.registerAgent({
  name: 'code-quality-analyst',
  systemPrompt: `You are a code quality expert. Review for maintainability,
readability, testing, and adherence to best practices.`,
  tools: [],
});

// Execute sequential analysis
const sequentialResults = await orchestrator.orchestrate(
  'Review the authentication module in src/auth',
  ['security-analyst', 'code-quality-analyst']
);

// Execute parallel analysis
const parallelResults = await orchestrator.executeParallel(
  'Analyze the checkout flow',
  ['security-analyst', 'performance-analyst', 'code-quality-analyst']
);

console.log('Security:', parallelResults['security-analyst']);
console.log('Performance:', parallelResults['performance-analyst']);
console.log('Quality:', parallelResults['code-quality-analyst']);
```

---

## MCP Server

### TypeScript

```typescript
import { MCPServer, Tool } from '@modelcontextprotocol/sdk';
import { Pool } from 'pg';

class DatabaseMCPServer extends MCPServer {
  private pool: Pool;

  constructor(databaseUrl: string) {
    super({
      name: 'database-server',
      version: '1.0.0',
      description: 'PostgreSQL database access via MCP',
    });

    this.pool = new Pool({ connectionString: databaseUrl });
    this.registerTools();
  }

  private registerTools(): void {
    // Query tool
    this.addTool({
      name: 'query',
      description: 'Execute a SQL query',
      inputSchema: {
        type: 'object',
        properties: {
          sql: {
            type: 'string',
            description: 'SQL query to execute',
          },
          params: {
            type: 'array',
            items: { type: 'string' },
            description: 'Query parameters',
          },
        },
        required: ['sql'],
      },
      handler: async (input) => {
        try {
          const result = await this.pool.query(
            input.sql,
            input.params || []
          );
          return {
            rows: result.rows,
            rowCount: result.rowCount,
          };
        } catch (error: any) {
          return {
            error: error.message,
          };
        }
      },
    });

    // Schema tool
    this.addTool({
      name: 'get_schema',
      description: 'Get database schema information',
      inputSchema: {
        type: 'object',
        properties: {
          table: {
            type: 'string',
            description: 'Table name (optional)',
          },
        },
      },
      handler: async (input) => {
        const query = input.table
          ? `SELECT column_name, data_type FROM information_schema.columns
             WHERE table_name = $1`
          : `SELECT table_name FROM information_schema.tables
             WHERE table_schema = 'public'`;

        const params = input.table ? [input.table] : [];
        const result = await this.pool.query(query, params);

        return {
          schema: result.rows,
        };
      },
    });

    // Transaction tool
    this.addTool({
      name: 'transaction',
      description: 'Execute multiple queries in a transaction',
      inputSchema: {
        type: 'object',
        properties: {
          queries: {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                sql: { type: 'string' },
                params: { type: 'array' },
              },
            },
          },
        },
        required: ['queries'],
      },
      handler: async (input) => {
        const client = await this.pool.connect();

        try {
          await client.query('BEGIN');

          const results = [];
          for (const query of input.queries) {
            const result = await client.query(
              query.sql,
              query.params || []
            );
            results.push({
              rowCount: result.rowCount,
              rows: result.rows,
            });
          }

          await client.query('COMMIT');
          return { success: true, results };
        } catch (error: any) {
          await client.query('ROLLBACK');
          return { success: false, error: error.message };
        } finally {
          client.release();
        }
      },
    });
  }

  async shutdown(): Promise<void> {
    await this.pool.end();
  }
}

// Start server
const server = new DatabaseMCPServer(
  process.env.DATABASE_URL!
);

server.listen(3000);

console.log('MCP Server running on port 3000');

// Graceful shutdown
process.on('SIGINT', async () => {
  await server.shutdown();
  process.exit(0);
});
```

---

## Custom Tool Development

### TypeScript

```typescript
import Anthropic from '@anthropic-ai/sdk';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

class CustomToolAgent {
  private client: Anthropic;
  private tools: Map<string, Anthropic.Tool>;
  private handlers: Map<string, (input: any) => Promise<any>>;

  constructor(apiKey: string) {
    this.client = new Anthropic({ apiKey });
    this.tools = new Map();
    this.handlers = new Map();
  }

  registerTool(
    tool: Anthropic.Tool,
    handler: (input: any) => Promise<any>
  ): void {
    this.tools.set(tool.name, tool);
    this.handlers.set(tool.name, handler);
  }

  async executeTool(name: string, input: any): Promise<any> {
    const handler = this.handlers.get(name);
    if (!handler) {
      throw new Error(`Tool ${name} not found`);
    }
    return await handler(input);
  }

  async chat(message: string): Promise<string> {
    const tools = Array.from(this.tools.values());

    const response = await this.client.messages.create({
      model: 'claude-3-5-sonnet-20241022',
      max_tokens: 4096,
      tools,
      messages: [{ role: 'user', content: message }],
    });

    // Process tool uses
    const toolResults: Anthropic.MessageParam[] = [];

    for (const content of response.content) {
      if (content.type === 'tool_use') {
        const result = await this.executeTool(
          content.name,
          content.input
        );

        toolResults.push({
          role: 'user',
          content: [
            {
              type: 'tool_result',
              tool_use_id: content.id,
              content: JSON.stringify(result),
            },
          ],
        });
      }
    }

    // If tools were used, continue conversation
    if (toolResults.length > 0) {
      const followUp = await this.client.messages.create({
        model: 'claude-3-5-sonnet-20241022',
        max_tokens: 4096,
        tools,
        messages: [
          { role: 'user', content: message },
          { role: 'assistant', content: response.content },
          ...toolResults,
        ],
      });

      return followUp.content
        .filter(c => c.type === 'text')
        .map(c => (c as Anthropic.TextBlock).text)
        .join('\n');
    }

    return response.content
      .filter(c => c.type === 'text')
      .map(c => (c as Anthropic.TextBlock).text)
      .join('\n');
  }
}

// Create agent
const agent = new CustomToolAgent(process.env.ANTHROPIC_API_KEY!);

// Register Git tool
agent.registerTool(
  {
    name: 'git_status',
    description: 'Get Git repository status',
    input_schema: {
      type: 'object',
      properties: {
        path: {
          type: 'string',
          description: 'Repository path',
        },
      },
    },
  },
  async (input) => {
    const { stdout } = await execAsync('git status', {
      cwd: input.path || process.cwd(),
    });
    return { status: stdout };
  }
);

// Register file search tool
agent.registerTool(
  {
    name: 'search_files',
    description: 'Search for files by pattern',
    input_schema: {
      type: 'object',
      properties: {
        pattern: {
          type: 'string',
          description: 'Glob pattern',
        },
        content: {
          type: 'string',
          description: 'Content to search for',
        },
      },
      required: ['pattern'],
    },
  },
  async (input) => {
    const { glob } = await import('glob');
    const files = await glob(input.pattern);

    if (input.content) {
      const { readFileSync } = await import('fs');
      const matches = files.filter(file => {
        const content = readFileSync(file, 'utf-8');
        return content.includes(input.content);
      });
      return { files: matches };
    }

    return { files };
  }
);

// Use agent
const response = await agent.chat(
  'What is the status of the Git repository and find all TypeScript files?'
);

console.log(response);
```

---

## Error Handling & Retry

### TypeScript

```typescript
import Anthropic from '@anthropic-ai/sdk';

class ResilientAgent {
  private client: Anthropic;

  constructor(apiKey: string) {
    this.client = new Anthropic({ apiKey });
  }

  async withRetry<T>(
    fn: () => Promise<T>,
    options: {
      maxRetries?: number;
      backoff?: number;
      onRetry?: (attempt: number, error: Error) => void;
    } = {}
  ): Promise<T> {
    const { maxRetries = 3, backoff = 1000, onRetry } = options;

    for (let attempt = 0; attempt < maxRetries; attempt++) {
      try {
        return await fn();
      } catch (error) {
        if (attempt === maxRetries - 1) {
          throw error;
        }

        if (onRetry) {
          onRetry(attempt + 1, error as Error);
        }

        // Exponential backoff
        const delay = backoff * Math.pow(2, attempt);
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }

    throw new Error('Max retries exceeded');
  }

  async chat(message: string): Promise<string> {
    return this.withRetry(
      async () => {
        const response = await this.client.messages.create({
          model: 'claude-3-5-sonnet-20241022',
          max_tokens: 1024,
          messages: [{ role: 'user', content: message }],
        });

        if (response.content[0].type === 'text') {
          return response.content[0].text;
        }

        throw new Error('Unexpected response format');
      },
      {
        maxRetries: 3,
        backoff: 1000,
        onRetry: (attempt, error) => {
          console.log(
            `Retry attempt ${attempt} after error: ${error.message}`
          );
        },
      }
    );
  }

  async chatWithFallback(
    message: string,
    fallbackModels: string[] = ['claude-3-5-haiku-20241022']
  ): Promise<string> {
    const models = [
      'claude-3-5-sonnet-20241022',
      ...fallbackModels,
    ];

    for (const model of models) {
      try {
        const response = await this.client.messages.create({
          model,
          max_tokens: 1024,
          messages: [{ role: 'user', content: message }],
        });

        if (response.content[0].type === 'text') {
          return response.content[0].text;
        }
      } catch (error) {
        console.error(`Model ${model} failed:`, error);
        if (model === models[models.length - 1]) {
          throw error;
        }
      }
    }

    throw new Error('All models failed');
  }
}

// Usage
const agent = new ResilientAgent(process.env.ANTHROPIC_API_KEY!);

try {
  const response = await agent.chat('Hello!');
  console.log(response);
} catch (error) {
  console.error('Chat failed:', error);
}

// With fallback
const responseWithFallback = await agent.chatWithFallback(
  'Analyze this complex problem...'
);
```

---

## Streaming Responses

### TypeScript

```typescript
import Anthropic from '@anthropic-ai/sdk';

class StreamingAgent {
  private client: Anthropic;

  constructor(apiKey: string) {
    this.client = new Anthropic({ apiKey });
  }

  async streamChat(
    message: string,
    callbacks: {
      onText?: (text: string) => void;
      onComplete?: (fullText: string) => void;
      onError?: (error: Error) => void;
    }
  ): Promise<void> {
    try {
      let fullText = '';

      const stream = await this.client.messages.stream({
        model: 'claude-3-5-sonnet-20241022',
        max_tokens: 1024,
        messages: [{ role: 'user', content: message }],
      });

      stream.on('text', (text) => {
        fullText += text;
        if (callbacks.onText) {
          callbacks.onText(text);
        }
      });

      stream.on('message', (message) => {
        if (callbacks.onComplete) {
          callbacks.onComplete(fullText);
        }
      });

      stream.on('error', (error) => {
        if (callbacks.onError) {
          callbacks.onError(error);
        }
      });

      await stream.finalMessage();
    } catch (error) {
      if (callbacks.onError) {
        callbacks.onError(error as Error);
      }
    }
  }

  async *streamChatGenerator(
    message: string
  ): AsyncGenerator<string, void, unknown> {
    const stream = await this.client.messages.stream({
      model: 'claude-3-5-sonnet-20241022',
      max_tokens: 1024,
      messages: [{ role: 'user', content: message }],
    });

    for await (const chunk of stream) {
      if (
        chunk.type === 'content_block_delta' &&
        chunk.delta.type === 'text_delta'
      ) {
        yield chunk.delta.text;
      }
    }
  }
}

// Usage with callbacks
const agent = new StreamingAgent(process.env.ANTHROPIC_API_KEY!);

await agent.streamChat('Tell me a story', {
  onText: (text) => {
    process.stdout.write(text);
  },
  onComplete: (fullText) => {
    console.log('\n\nComplete! Total length:', fullText.length);
  },
  onError: (error) => {
    console.error('Error:', error);
  },
});

// Usage with async generator
console.log('\n\nUsing generator:');
for await (const chunk of agent.streamChatGenerator(
  'Count to 10'
)) {
  process.stdout.write(chunk);
}
```

### Python

```python
from anthropic import AsyncAnthropic
import asyncio

class StreamingAgent:
    def __init__(self, api_key: str):
        self.client = AsyncAnthropic(api_key=api_key)

    async def stream_chat(self, message: str, on_text=None, on_complete=None):
        """Stream chat with callbacks."""
        full_text = ""

        async with self.client.messages.stream(
            model="claude-3-5-sonnet-20241022",
            max_tokens=1024,
            messages=[{"role": "user", "content": message}]
        ) as stream:
            async for text in stream.text_stream:
                full_text += text
                if on_text:
                    on_text(text)

        if on_complete:
            on_complete(full_text)

        return full_text

    async def stream_chat_generator(self, message: str):
        """Stream chat as async generator."""
        async with self.client.messages.stream(
            model="claude-3-5-sonnet-20241022",
            max_tokens=1024,
            messages=[{"role": "user", "content": message}]
        ) as stream:
            async for text in stream.text_stream:
                yield text

# Usage
import os

async def main():
    agent = StreamingAgent(os.environ["ANTHROPIC_API_KEY"])

    # With callbacks
    def print_text(text):
        print(text, end="", flush=True)

    def print_complete(full_text):
        print(f"\n\nComplete! Length: {len(full_text)}")

    await agent.stream_chat(
        "Tell me a story",
        on_text=print_text,
        on_complete=print_complete
    )

    # With async generator
    print("\n\nUsing generator:")
    async for chunk in agent.stream_chat_generator("Count to 10"):
        print(chunk, end="", flush=True)

asyncio.run(main())
```

---

*These examples demonstrate production-ready patterns for building with the Claude Agent SDK. Adapt them to your specific use cases.*
