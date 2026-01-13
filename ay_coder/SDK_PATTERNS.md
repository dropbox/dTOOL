# SDK Patterns for AI Coding Systems

Research compiled from Anthropic Claude, OpenAI, Google Gemini, Aider, and Continue.dev documentation.

## Table of Contents
1. [API Design Patterns](#api-design-patterns)
2. [Anthropic Claude API](#anthropic-claude-api)
3. [OpenAI API](#openai-api)
4. [Google Gemini API](#google-gemini-api)
5. [Aider Patterns](#aider-patterns)
6. [Continue.dev Patterns](#continuedev-patterns)
7. [Cross-Platform Best Practices](#cross-platform-best-practices)

---

## API Design Patterns

### Common Patterns Across All Platforms

1. **Client Initialization**
   - Use environment variables for API keys
   - Support both sync and async clients
   - Type-safe request/response handling

2. **Message Structure**
   - Role-based messages (user, assistant, system)
   - Multi-turn conversation support
   - Context preservation across messages

3. **Streaming Support**
   - Enable real-time response processing
   - Event-driven chunk handling
   - Support both sync and async streaming

4. **Function/Tool Calling**
   - Declarative tool definitions
   - JSON schema for parameters
   - Model generates arguments, client executes functions

---

## Anthropic Claude API

### Messages API Design

#### Python SDK

**Basic Usage:**
```python
from anthropic import Anthropic
import os

client = Anthropic(api_key=os.environ.get("ANTHROPIC_API_KEY"))

response = client.messages.create(
    model="claude-sonnet-4-5-20250929",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Hello, Claude"}]
)
```

**Multi-Turn Conversation:**
```python
# First message
response1 = client.messages.create(
    max_tokens=1024,
    messages=[{"role": "user", "content": "Hello!"}],
    model="claude-sonnet-4-5-20250929"
)

# Continue conversation
response2 = client.messages.create(
    max_tokens=1024,
    messages=[
        {"role": "user", "content": "Hello!"},
        {"role": response1.role, "content": response1.content},
        {"role": "user", "content": "How are you?"}
    ],
    model="claude-sonnet-4-5-20250929"
)
```

#### TypeScript SDK

**Basic Usage:**
```typescript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic({
  apiKey: process.env['ANTHROPIC_API_KEY']
});

const message = await client.messages.create({
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Hello, Claude' }],
  model: 'claude-sonnet-4-5-20250929'
});
```

**Streaming Implementation:**
```typescript
const stream = await client.messages.create({
  stream: true,
  max_tokens: 1024,
  messages: [{ role: 'user', content: 'Hello' }],
  model: 'claude-sonnet-4-5-20250929'
});

for await (const messageStreamEvent of stream) {
  console.log(messageStreamEvent.type);
}
```

### Tool Use / Function Calling

#### Tool Definition Format

```python
from anthropic.types import ToolParam

tools: list[ToolParam] = [
    {
        "name": "get_weather",
        "description": "Get the current weather for a specific location",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City and state, e.g., San Francisco, CA"
                }
            },
            "required": ["location"]
        }
    }
]
```

#### Tool Use Workflow

1. **Define Tools:** Create tool definitions with names, descriptions, and schemas
2. **Send Message with Tools:** Include tools parameter in message creation
3. **Check Stop Reason:** Look for `stop_reason == "tool_use"`
4. **Extract Tool Use:** Parse tool use content from response
5. **Execute Function:** Run the function on your system
6. **Return Results:** Send tool results back to Claude
7. **Get Final Response:** Claude generates response using tool results

```python
# Step 1: Initial message with tools
response = client.messages.create(
    model="claude-sonnet-4-5-20250929",
    max_tokens=1024,
    tools=tools,
    messages=[{"role": "user", "content": "What's the weather in San Francisco?"}]
)

# Step 2: Check if tool was used
if response.stop_reason == "tool_use":
    # Step 3: Extract tool use
    tool_use = next(
        (block for block in response.content if block.type == "tool_use"),
        None
    )

    # Step 4: Execute function (implement your own logic)
    tool_result = execute_tool(tool_use.name, tool_use.input)

    # Step 5: Send results back
    follow_up = client.messages.create(
        model="claude-sonnet-4-5-20250929",
        max_tokens=1024,
        tools=tools,
        messages=[
            {"role": "user", "content": "What's the weather in San Francisco?"},
            {"role": "assistant", "content": response.content},
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_use.id,
                        "content": tool_result
                    }
                ]
            }
        ]
    )
```

#### TypeScript Tool Use with Zod

```typescript
import { z } from 'zod';
import { betaZodTool } from '@anthropic-ai/sdk/helpers/zod';

const weatherTool = betaZodTool({
  name: 'get_weather',
  description: 'Get the current weather for a location',
  inputSchema: z.object({
    location: z.string().describe('The city and state, e.g., San Francisco, CA')
  }),
  run: async (input) => {
    // Your implementation
    return `The weather in ${input.location} is foggy and 60°F`;
  }
});

const finalMessage = await client.beta.messages.toolRunner({
  model: 'claude-sonnet-4-5-20250929',
  max_tokens: 1024,
  tools: [weatherTool],
  messages: [{ role: 'user', content: 'What is the weather in San Francisco?' }]
});
```

### Tool Types

1. **Client Tools:**
   - Executed on user's system
   - Require custom implementation
   - User-defined and Anthropic-defined tools (e.g., computer use)

2. **Server Tools:**
   - Executed on Anthropic's servers
   - Examples: web search, web fetch
   - Automatically processed without client implementation

### Best Practices

1. **Tool Design:**
   - Provide clear, specific tool descriptions
   - Define precise input schemas
   - Handle missing parameter scenarios
   - Use strict mode for guaranteed schema conformance

2. **Context Management:**
   - Include conversation history for context
   - Preserve tool use and results in message history
   - Keep track of multi-turn tool interactions

3. **Error Handling:**
   - Check stop reason before processing
   - Validate tool use content exists
   - Handle execution errors gracefully

4. **Performance:**
   - Tool definitions count toward input tokens
   - Be mindful of token limits
   - Consider prompt caching for repeated tool definitions

---

## OpenAI API

### Chat Completions API

#### Python SDK

**Basic Usage:**
```python
from openai import OpenAI
import os

client = OpenAI(api_key=os.environ.get("OPENAI_API_KEY"))

response = client.responses.create(
    model="gpt-4o",
    input="Explain quantum computing in simple terms"
)
```

**Streaming Responses:**
```python
# Synchronous streaming
response = client.completions.create(
    model="gpt-3.5-turbo-instruct",
    prompt="1,2,3,",
    max_tokens=5,
    temperature=0,
    stream=True
)

# Manual extraction of first response
first_chunk = next(response)
print(first_chunk)

# Iterate through entire response
for chunk in response:
    print(chunk)
```

**Async Streaming:**
```python
from openai import AsyncOpenAI

async_client = AsyncOpenAI()

async def stream_response():
    response = await async_client.completions.create(
        model="gpt-3.5-turbo-instruct",
        prompt="1,2,3,",
        max_tokens=5,
        temperature=0,
        stream=True
    )

    # Manual extraction
    first_chunk = await response.__anext__()

    # Iterate through response
    async for chunk in response:
        print(chunk)
```

#### TypeScript SDK

**Basic Usage:**
```typescript
import OpenAI from 'openai';

const client = new OpenAI({
  apiKey: process.env['OPENAI_API_KEY']
});

const response = await client.responses.create({
  model: 'gpt-4o',
  input: 'Are semicolons optional in JavaScript?'
});
```

**Streaming:**
```typescript
const stream = await client.responses.create({
  model: 'gpt-4o',
  input: 'Say "Sheep sleep deep" ten times fast!',
  stream: true
});

for await (const event of stream) {
  console.log(event);
}
```

### Function Calling

#### Function Specification Format

```python
tools = [
    {
        "type": "function",
        "function": {
            "name": "get_current_weather",
            "description": "Get the current weather in a given location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city and state, e.g. San Francisco, CA"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "description": "The temperature unit to use"
                    }
                },
                "required": ["location", "format"]
            }
        }
    }
]
```

#### Function Calling Workflow

```python
# Step 1: Send message with function definitions
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "What's the weather in Boston?"}],
    tools=tools
)

# Step 2: Check if model wants to call a function
if response.choices[0].finish_reason == "tool_calls":
    tool_calls = response.choices[0].message.tool_calls

    # Step 3: Execute each function call
    for tool_call in tool_calls:
        function_name = tool_call.function.name
        function_args = json.loads(tool_call.function.arguments)

        # Execute function (implement your logic)
        function_result = execute_function(function_name, function_args)

        # Step 4: Send function results back
        messages.append({
            "role": "function",
            "name": function_name,
            "content": function_result
        })

    # Step 5: Get final response
    final_response = client.chat.completions.create(
        model="gpt-4o",
        messages=messages,
        tools=tools
    )
```

#### Key Features

1. **Parallel Function Calling:**
   - Model can generate multiple function calls simultaneously
   - Handle multiple tool calls in a single response

2. **Tool Choice:**
   - `auto`: Model decides whether to call functions
   - `none`: Model will not call functions
   - `{"type": "function", "function": {"name": "my_function"}}`: Force specific function

3. **Important Note:**
   - The API generates function calls but does NOT execute them
   - Developers must execute functions using model outputs

### Error Handling

```typescript
import OpenAI from 'openai';

try {
  const result = await client.chat.completions.create({
    model: 'gpt-4o',
    messages: [{ role: 'user', content: 'Hello' }]
  });
} catch (err) {
  if (err instanceof OpenAI.APIError) {
    console.log(`Error ${err.status}: ${err.name}`);
    console.log(err.message);
  }
}
```

### Best Practices

1. **Function Definitions:**
   - Provide clear, specific descriptions
   - Use descriptive parameter names
   - Specify required vs. optional parameters
   - Use enums for constrained values

2. **Streaming:**
   - Support both manual and automatic chunk processing
   - Handle streaming errors gracefully
   - Use async streaming for non-blocking operations

3. **Model Selection:**
   - Use GPT-4o for complex reasoning and function calling
   - Use GPT-3.5 for simpler tasks and cost optimization

---

## Google Gemini API

### Basic Text Generation

#### Python SDK

**Installation:**
```bash
pip install -q -U google-genai
```

**Basic Usage:**
```python
from google import genai

# Client gets API key from GEMINI_API_KEY environment variable
client = genai.Client()

response = client.models.generate_content(
    model="gemini-2.5-flash",
    contents="Explain how AI works in a few words"
)
print(response.text)
```

### Function Calling

#### Function Declaration Format

```python
schedule_meeting_function = {
    "name": "schedule_meeting",
    "description": "Schedules a meeting with specified attendees at a given date and time",
    "parameters": {
        "type": "object",
        "properties": {
            "attendees": {
                "type": "array",
                "description": "List of email addresses for meeting participants",
                "items": {"type": "string"}
            },
            "date": {
                "type": "string",
                "description": "Meeting date in YYYY-MM-DD format"
            },
            "time": {
                "type": "string",
                "description": "Meeting time in HH:MM format"
            }
        },
        "required": ["attendees", "date", "time"]
    }
}
```

#### Function Calling Process

1. **Define function declarations**
2. **Send prompt with function declarations**
3. **Model analyzes and potentially calls functions**
4. **Application executes function**
5. **Send function results back to model**
6. **Model generates final response**

#### Function Calling Modes

- `AUTO`: Model decides when to use functions
- `ANY`: Model must use at least one function
- `NONE`: Model will not use functions
- `VALIDATED`: Validate function calls before execution

#### Unique Features

1. **Automatic Function Calling:**
   - Python SDK offers automatic function calling
   - Simplifies tool integration
   - Reduces boilerplate code

2. **Parallel Function Calling:**
   - Can use multiple tools simultaneously
   - Supports compositional function calls

3. **Context Handling:**
   - Uses "thought signatures" to maintain context
   - Supports stateless interactions through careful response management
   - Allows multi-turn conversations with function calls

### Best Practices

1. **Function Design:**
   - Provide clear, specific function descriptions
   - Use strong typing for parameters
   - Limit total number of tools (10-20 recommended)
   - Be mindful of token limits

2. **Reliability:**
   - Use low temperature for more deterministic calls
   - Validate high-consequence function calls
   - Implement robust error handling

3. **Model Selection:**
   - Gemini 2.5 Pro: Best for complex reasoning
   - Gemini 2.5 Flash: Fast and efficient
   - Gemini 2.5 Flash-Lite: Lightweight applications

### Key Capabilities

- **Augment Knowledge:** Provide real-time information
- **Extend Capabilities:** Add custom functionality
- **Take Actions:** Execute operations on behalf of users

---

## Aider Patterns

### Architecture Overview

Aider is a terminal-based AI pair programming tool with tight Git integration.

#### Core Design Principles

1. **Multi-Modal Configuration:**
   - Command line switches
   - YAML config file (`.aider.conf.yml`)
   - Environment variables (prefixed with `AIDER_`)
   - `.env` file

2. **Configuration Example:**
```bash
# Command line
aider --dark-mode

# YAML (.aider.conf.yml)
dark-mode: true

# Environment variable
export AIDER_DARK_MODE=true

# .env file
AIDER_DARK_MODE=true
```

### Repository Mapping Strategy

#### Context Provision

Aider creates a "concise map of your whole git repository" to provide context to LLMs:

1. **Map Contents:**
   - Most important classes, functions, types
   - Key call signatures
   - Critical code definition lines

2. **Optimization Approach:**
   - Use graph-based ranking algorithms
   - Dynamically adjust map based on chat state
   - Focus on "most often referenced" code identifiers
   - Default map size: 1k tokens (configurable)

3. **Benefits:**
   - Helps LLM understand code relationships
   - Provides API and module usage insights
   - Allows LLM to identify which files need deeper examination

### Edit Format: Unified Diffs

#### Design Principles

1. **FAMILIAR:** Use format GPT already understands from training
2. **SIMPLE:** Minimize syntactic overhead
3. **HIGH LEVEL:** Encourage editing substantial code blocks
4. **FLEXIBLE:** Maximize ability to interpret edit instructions

#### Flexible Patch Application

Aider implements multiple strategies for applying diffs:

1. **Normalization:**
   - Normalize hunks if they don't apply cleanly
   - Discover missing line additions
   - Apply hunks with flexible indentation

2. **Adaptive Processing:**
   - Break large hunks into smaller sub-hunks
   - Dynamically adjust context windows
   - Ignore line numbers in diffs

3. **High-Level Diffs:**
   - Encourage showing entire function/method versions
   - Transform from line-by-line to holistic modifications

#### Performance Impact

- Reduced "lazy coding" by 3X
- Increased successful code refactoring from 20% to 61%
- Dramatically improved code editing accuracy

### LLM Integration

#### Supported Models

**Best Performing:**
1. Gemini 2.5 Pro
2. DeepSeek R1 and V3
3. Claude 3.7 Sonnet
4. OpenAI o3, o4-mini, and GPT-4.1

**Free Options:**
- OpenRouter (with limitations)
- Google's Gemini 2.5 Pro Exp

**Local Models:**
- Ollama integration
- OpenAI-compatible API local models

#### Key Recommendation

> "Be aware that aider may not work well with less capable models."

Models weaker than GPT-3.5 may struggle with code editing tasks.

### Usage Patterns

#### Basic Command Structure

```bash
# Start with files
aider file1.py file2.py

# Model specification
aider --model sonnet --api-key anthropic=<key>

# In-chat commands
/add newfile.py    # Add file to chat
/undo             # Revert AI changes
/help             # Show help
```

#### Interaction Design

1. **Conversational Interface:**
   - Natural language code modification requests
   - Contextually aware of project structure
   - Shows diffs before making changes

2. **Change Management:**
   - Automatic git commits for modifications
   - Clear diff visualization
   - Reversible changes with `/undo`

3. **File Management:**
   - Specify files at launch or add during session
   - Recommendation: Add only relevant files to avoid overwhelming AI
   - Can create new files or modify existing ones

### Best Practices

1. **Configuration:**
   - Choose configuration method that fits workflow
   - Use YAML for project-specific settings
   - Use environment variables for personal preferences

2. **Context Management:**
   - Add only relevant files to chat
   - Leverage repository map for broad context
   - Be specific about which files need editing

3. **Model Selection:**
   - Consult Aider's LLM leaderboards for performance
   - Use capable models for complex refactoring
   - Consider free options for simple tasks

4. **Change Management:**
   - Review diffs before accepting changes
   - Use git integration for version control
   - Leverage `/undo` for quick reversals

---

## Continue.dev Patterns

### Architecture Overview

Continue.dev is a multi-modal AI agent platform with three deployment modes:

1. **Cloud Agents:** Automated workflows triggered by events
2. **CLI Agents:** Real-time interactive terminal workflows (TUI mode)
3. **IDE Agents:** VS Code and JetBrains extensions

### Design Philosophy

> "The future of coding isn't writing more code. It's delegating the boring parts"

Focus on automating repetitive coding tasks to allow developers to "build the interesting stuff."

### Integration Patterns

#### Multi-Model Support

- OpenAI (GPT-4, GPT-3.5)
- Anthropic (Claude)
- Google (Gemini)
- Local models via Ollama

#### Configuration

- YAML and JSON configuration files
- Customizable AI agents with configurable:
  - Model providers
  - Roles (chat, autocomplete, edit)
  - Tools and context selection

### Extension Design

#### Operating Modes

1. **Agent Mode:**
   - Autonomous task execution
   - Multi-step workflows
   - Event-driven triggers

2. **Chat Mode:**
   - Conversational code assistance
   - Context-aware responses
   - Multi-turn interactions

3. **Autocomplete:**
   - Real-time code suggestions
   - Context-aware completions
   - IDE integration

4. **Edit Mode:**
   - Direct code modifications
   - Diff-based changes
   - Version control integration

5. **Plan Mode:**
   - Safe, read-only code exploration
   - Architecture analysis
   - No modifications made

#### Deployment Modes

**TUI Mode (Terminal):**
- Interactive terminal experience
- Real-time workflows
- Command-line interface

**Headless Mode:**
- Background agents
- CI/CD automation
- Event-driven execution

### Tool Integrations

Continue.dev connects to existing developer tools:

- **GitHub:** PR automation, code review
- **Slack:** Notifications, team collaboration
- **Sentry:** Error tracking integration
- **Snyk:** Security scanning

### Technical Stack

- **Primary Language:** TypeScript (83.4%)
- **License:** Apache 2.0
- **Extensibility:** Plugin ecosystem for VS Code and JetBrains

### Best Practices

1. **Context Management:**
   - Use context-aware tools
   - Leverage IDE integration for file context
   - Configure context providers appropriately

2. **Workflow Automation:**
   - Use Plan Mode for safe exploration
   - Leverage Agent Mode for recurring tasks
   - Configure event triggers for automation

3. **Customization:**
   - Customize prompts for specific use cases
   - Configure model settings for performance
   - Use workflow automation for team consistency

4. **IDE Integration:**
   - Leverage autocomplete for productivity
   - Use inline editing for quick fixes
   - Integrate with existing workflows

---

## Cross-Platform Best Practices

### API Design

#### 1. Client Initialization

**Common Pattern:**
```python
# Environment-based API keys
client = APIClient(api_key=os.environ.get("API_KEY"))

# Support both sync and async
sync_client = APIClient()
async_client = AsyncAPIClient()
```

**Best Practices:**
- Use environment variables for secrets
- Support multiple authentication methods
- Provide clear error messages for missing credentials

#### 2. Message Structure

**Standard Format:**
```python
messages = [
    {"role": "system", "content": "You are a helpful assistant"},
    {"role": "user", "content": "Hello"},
    {"role": "assistant", "content": "Hi! How can I help?"},
    {"role": "user", "content": "What's the weather?"}
]
```

**Best Practices:**
- Use role-based message structure
- Preserve conversation history for context
- Support system messages for behavior modification
- Keep messages organized and timestamped

#### 3. Error Handling

**Common Pattern:**
```python
try:
    response = client.create_message(...)
except APIError as e:
    if e.status_code == 429:
        # Rate limit - implement exponential backoff
        handle_rate_limit(e)
    elif e.status_code >= 500:
        # Server error - retry with backoff
        handle_server_error(e)
    else:
        # Client error - log and handle
        handle_client_error(e)
```

**Best Practices:**
- Implement exponential backoff for rate limits
- Retry on server errors (5xx)
- Log errors with context
- Provide user-friendly error messages
- Handle network timeouts gracefully

### Streaming Patterns

#### Universal Streaming Pattern

**Sync Streaming:**
```python
stream = client.create_message(stream=True, ...)

# Manual extraction
first_chunk = next(stream)

# Full iteration
for chunk in stream:
    process_chunk(chunk)
```

**Async Streaming:**
```python
async def stream_response():
    stream = await async_client.create_message(stream=True, ...)

    # Manual extraction
    first_chunk = await stream.__anext__()

    # Full iteration
    async for chunk in stream:
        await process_chunk(chunk)
```

**Best Practices:**
- Support both sync and async streaming
- Allow manual and automatic chunk processing
- Handle streaming interruptions
- Implement timeout mechanisms
- Buffer chunks appropriately

### Function/Tool Calling Patterns

#### Universal Tool Definition Format

```json
{
  "name": "function_name",
  "description": "Clear description of what this function does",
  "parameters": {
    "type": "object",
    "properties": {
      "param1": {
        "type": "string",
        "description": "Description of param1"
      },
      "param2": {
        "type": "integer",
        "description": "Description of param2",
        "enum": [1, 2, 3]
      }
    },
    "required": ["param1"]
  }
}
```

#### Tool Execution Workflow

1. **Define Tools:** Clear names, descriptions, and schemas
2. **Send Message:** Include tool definitions with user message
3. **Check Response:** Look for tool use indicator
4. **Extract Tool Calls:** Parse tool name and arguments
5. **Execute Functions:** Run functions on your system
6. **Return Results:** Send results back to model
7. **Get Final Response:** Model generates answer using results

**Best Practices:**
- Provide clear, specific tool descriptions
- Use JSON Schema for parameter validation
- Limit number of tools (10-20 recommended)
- Handle tool execution errors gracefully
- Return structured results
- Support parallel tool execution
- Validate high-consequence tool calls

### Context Management

#### Repository/Code Context

**Patterns from Aider:**
- Create concise repository maps
- Use graph-based ranking for relevance
- Include key symbols and signatures
- Limit context size to token budget

**Patterns from Continue.dev:**
- IDE integration for file context
- Context-aware tools and providers
- Dynamic context selection

**Best Practices:**
- Provide relevant, not exhaustive, context
- Use summarization for large codebases
- Include file structure and relationships
- Update context dynamically
- Cache frequently used context

### Configuration Management

#### Multi-Method Configuration

**Pattern from Aider:**
1. Command line arguments
2. Configuration files (YAML/JSON)
3. Environment variables
4. `.env` files

**Best Practices:**
- Support multiple configuration sources
- Clear precedence order
- Validate configuration
- Provide sensible defaults
- Document all options

### Edit/Modification Patterns

#### Diff-Based Editing

**Pattern from Aider:**
- Use unified diff format
- High-level, whole-function edits
- Flexible patch application
- Multiple fallback strategies

**Best Practices:**
- Show diffs before applying changes
- Support multiple edit formats
- Implement flexible matching
- Provide rollback mechanisms
- Version control integration

### Model Selection

#### Best Practices Across Platforms

1. **Task Complexity:**
   - Simple tasks: Use smaller, faster models
   - Complex reasoning: Use frontier models
   - Code editing: Use models proven for code

2. **Cost Optimization:**
   - Cache prompts when possible
   - Use appropriate model sizes
   - Implement request batching

3. **Performance:**
   - Monitor token usage
   - Implement timeout mechanisms
   - Use streaming for long responses
   - Consider local models for privacy

### Token Management

**Best Practices:**
- Track input and output tokens
- Implement token counting before requests
- Optimize prompts for token efficiency
- Cache repeated content
- Truncate context intelligently
- Use compression for large contexts

### Testing and Validation

**Best Practices:**
- Test with various model configurations
- Validate tool definitions and schemas
- Test streaming interruption handling
- Verify error handling paths
- Test rate limit handling
- Validate context management

### Security

**Best Practices:**
- Never log API keys
- Use environment variables for secrets
- Validate tool execution permissions
- Sanitize user inputs
- Implement rate limiting
- Use secure communication (HTTPS)
- Validate function call permissions

---

## Key Takeaways

### 1. Consistency Across Platforms

All major AI platforms follow similar patterns:
- Role-based message structure
- JSON Schema for tool definitions
- Streaming support
- Async and sync APIs
- Environment-based configuration

### 2. Tool/Function Calling is Standard

All platforms support function calling with similar workflows:
- Declarative tool definitions
- Model generates arguments
- Client executes functions
- Results sent back to model

### 3. Streaming is Essential

Real-time response processing is a core feature:
- Support both sync and async
- Event-driven chunk handling
- Graceful error handling

### 4. Context Management is Critical

For coding assistants:
- Repository mapping (Aider)
- IDE integration (Continue.dev)
- Selective context provision
- Dynamic context updates

### 5. Edit Formats Matter

Different approaches to code modification:
- Unified diffs (Aider) - proven effective
- Direct replacement
- Multi-strategy fallback

### 6. Configuration Flexibility

Multiple configuration methods:
- Command line
- Config files
- Environment variables
- Programmatic

### 7. Model Selection is Important

Choose models based on:
- Task complexity
- Cost considerations
- Performance requirements
- Code-specific capabilities

### 8. Error Handling is Non-Negotiable

Robust error handling includes:
- Rate limit handling with backoff
- Server error retries
- Clear error messages
- Graceful degradation

### 9. Type Safety Improves DX

TypeScript/Python type hints improve:
- Developer experience
- Error detection
- IDE support
- Documentation

### 10. Open Source Integration

Many tools are open source:
- Aider: Terminal-based
- Continue.dev: Multi-modal
- Learn from their approaches
- Contribute improvements

---

## Implementation Recommendations for ay_coder

Based on these patterns, here are recommendations for ay_coder:

### 1. API Design
- Follow Anthropic's message-based pattern
- Support both sync and async clients
- Use TypeScript for type safety
- Implement streaming from the start

### 2. Tool System
- Use JSON Schema for tool definitions
- Implement tool execution workflow similar to Claude
- Support parallel tool execution
- Validate tool permissions

### 3. Context Management
- Implement repository mapping like Aider
- Use graph-based ranking for context selection
- Keep context within token limits
- Support dynamic context updates

### 4. Edit Format
- Start with unified diffs (proven effective)
- Implement flexible patch application
- Show diffs before applying
- Support rollback

### 5. Configuration
- Support multiple configuration methods
- Use YAML for project settings
- Use environment variables for secrets
- Provide sensible defaults

### 6. Error Handling
- Implement exponential backoff
- Provide clear error messages
- Log errors with context
- Handle all common error scenarios

### 7. Model Support
- Support multiple model providers
- Allow model switching
- Implement model-specific optimizations
- Track model performance

### 8. Testing
- Test with various models
- Validate tool definitions
- Test streaming and error handling
- Implement integration tests

### 9. Documentation
- Provide clear API documentation
- Include code examples
- Document best practices
- Create cookbook-style guides

### 10. Open Source Approach
- Learn from existing tools
- Contribute improvements back
- Build on proven patterns
- Foster community engagement

---

## Additional Resources

### Anthropic
- Docs: https://docs.anthropic.com
- Python SDK: https://github.com/anthropics/anthropic-sdk-python
- TypeScript SDK: https://github.com/anthropics/anthropic-sdk-typescript
- Cookbooks: https://github.com/anthropics/anthropic-cookbook

### OpenAI
- Docs: https://platform.openai.com/docs
- Python SDK: https://github.com/openai/openai-python
- Node SDK: https://github.com/openai/openai-node
- Cookbook: https://cookbook.openai.com

### Google Gemini
- Docs: https://ai.google.dev/gemini-api/docs
- Cookbook: https://github.com/google-gemini/cookbook

### Aider
- Docs: https://aider.chat/docs
- GitHub: https://github.com/paul-gauthier/aider

### Continue.dev
- Docs: https://docs.continue.dev
- GitHub: https://github.com/continuedev/continue

---

## Conclusion

The AI coding assistant space has converged on several key patterns:

1. **Message-based APIs** with role structure
2. **JSON Schema tool definitions** for function calling
3. **Streaming responses** for real-time interaction
4. **Context management** for code understanding
5. **Flexible configuration** for different use cases
6. **Robust error handling** for production use
7. **Type safety** for better developer experience

By following these established patterns, ay_coder can build on proven approaches while innovating where it adds value. The key is to start with solid foundations and iterate based on real usage.
