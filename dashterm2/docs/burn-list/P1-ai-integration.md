# AI Integration

**Priority:** P1
**Total Issues:** 33
**Fixed:** 5
**In Progress:** 0
**Skip (Feature Requests):** 18
**External:** 4
**Cannot Reproduce:** 5
**Wontfix:** 1
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1296 investigation)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12654](https://gitlab.com/gnachman/iterm2/-/issues/12654) | Running iterm2_shell_integration.zsh breaks AI agents | Shell integration conflicts with Google Antigravity IDE | 2025-12-26 | - | - | - | External | External environment issue - AI agent env, not iTerm2 |
| [#12640](https://gitlab.com/gnachman/iterm2/-/issues/12640) | Ability to configure AI settings locally or per-machine | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12595](https://gitlab.com/gnachman/iterm2/-/issues/12595) | Lost ability to "fork/clone/delete" a chat message in AI Chat | Right-click menu missing after Liquid Glass UI update | 2025-12-27 | 2025-12-27 | 9bcf94bb1 | - | Fixed | Already fixed in DashTerm2 - MessageCellView.menu property |
| [#12539](https://gitlab.com/gnachman/iterm2/-/issues/12539) | AI Agent Not Loading | Plugin not detected despite installation | 2025-12-26 | - | - | - | External | User installation/config issue |
| [#12521](https://gitlab.com/gnachman/iterm2/-/issues/12521) | Support Zero Data Retention orgs in AI Chat | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12506](https://gitlab.com/gnachman/iterm2/-/issues/12506) | AI Chats cannot see data from restored sessions | History not visible after session restore | 2025-12-27 | - | - | - | Cannot Reproduce | Cannot repro - chats load from SQLite DB, session GUIDs preserved in arrangements |
| [#12479](https://gitlab.com/gnachman/iterm2/-/issues/12479) | rendering issue in openai codex | Rendering problems with Codex in terminal | 2025-12-27 | - | - | - | Cannot Reproduce | Insufficient info - third-party tool rendering in terminal, needs specific repro |
| [#12451](https://gitlab.com/gnachman/iterm2/-/issues/12451) | AI Chat says "Plugin not found" even if plugin installed | Plugin detection failure | 2025-12-26 | - | - | - | External | User installation/config issue |
| [#12387](https://gitlab.com/gnachman/iterm2/-/issues/12387) | Support AWS Bedrock for AI assistant | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12380](https://gitlab.com/gnachman/iterm2/-/issues/12380) | AI API responds "Unsupported parameter: 'max_tokens'" | max_tokens not supported by some APIs | 2025-12-26 | 2025-12-26 | LLMProvider | - | Fixed | LLMProvider.maxTokens() now respects per-model limits |
| [#12330](https://gitlab.com/gnachman/iterm2/-/issues/12330) | Customize the AI LLM deepseek | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12292](https://gitlab.com/gnachman/iterm2/-/issues/12292) | Custom AI generation command | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12260](https://gitlab.com/gnachman/iterm2/-/issues/12260) | DeepSeek AI integration | - | - | - | - | - | Skip | Feature request - now supported |
| [#12237](https://gitlab.com/gnachman/iterm2/-/issues/12237) | Support for Microsoft Copilot AI | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12182](https://gitlab.com/gnachman/iterm2/-/issues/12182) | Error from OpenAI api: Missing required parameter: 'messages' | Old API version incompatibility | 2025-12-26 | 2025-12-26 | AIMetadata | - | Fixed | Modern APIs use responses API - old v3.5.x issue |
| [#11900](https://gitlab.com/gnachman/iterm2/-/issues/11900) | Option to use Google Gemini API for AI completions | - | - | - | - | - | Skip | Feature request - now supported |
| [#11869](https://gitlab.com/gnachman/iterm2/-/issues/11869) | AI configuration screen should contemplate other solutions | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11856](https://gitlab.com/gnachman/iterm2/-/issues/11856) | add ai models: o1-preview & o1-mini | - | - | - | - | - | Skip | Feature request - o3/o4 now supported |
| [#11808](https://gitlab.com/gnachman/iterm2/-/issues/11808) | Configurable OpenAI API options aren't accounted for in Settings | Project-based API key documentation | 2025-12-26 | - | - | - | Cannot Reproduce | Feature request/docs - settings exist |
| [#11800](https://gitlab.com/gnachman/iterm2/-/issues/11800) | control-k preceding the command line when gen AI creates command | ^K prepended to generated commands | 2025-12-27 | - | - | - | Wontfix | By design - composerClearSequence sends ^U^K to clear line. Configurable in Settings > Advanced. |
| [#11677](https://gitlab.com/gnachman/iterm2/-/issues/11677) | Hide "Engage Artificial Inteligence" from the menu when AI disabled | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11612](https://gitlab.com/gnachman/iterm2/-/issues/11612) | openAI key not working | API key auth error on Mac mini | 2025-12-26 | - | - | - | Cannot Reproduce | Works on laptop, env-specific |
| [#11582](https://gitlab.com/gnachman/iterm2/-/issues/11582) | AI token counter seems to work in reverse? | Token display incorrect | 2025-12-26 | 2025-12-26 | AIMetadata | - | Fixed | Token counting rewritten with model metadata |
| [#11561](https://gitlab.com/gnachman/iterm2/-/issues/11561) | Allow OpenAI-compatible server | - | - | - | - | - | Skip | Feature request - now supported |
| [#11541](https://gitlab.com/gnachman/iterm2/-/issues/11541) | AI completion using a local LLM is prepending shell name | fish prepended to commands | 2025-12-26 | - | - | - | Cannot Reproduce | May be Ollama/phi3 specific |
| [#11535](https://gitlab.com/gnachman/iterm2/-/issues/11535) | AI Returns an error - max_tokens is too large: 127795 | Token limit exceeds model max | 2025-12-26 | 2025-12-26 | LLMProvider | - | Fixed | LLMProvider.maxTokens() caps at model.maxResponseTokens |
| [#11512](https://gitlab.com/gnachman/iterm2/-/issues/11512) | Enable/disable AI per profile | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11509](https://gitlab.com/gnachman/iterm2/-/issues/11509) | AI features should have an option for using local LLMs | - | - | - | - | - | Skip | Feature request - Llama now supported |
| [#11493](https://gitlab.com/gnachman/iterm2/-/issues/11493) | AI Prompt Sends max tokens in Composer | Max token error in composer | 2025-12-26 | 2025-12-26 | LLMProvider | - | Fixed | Same root cause as #11535 - token limits fixed |
| [#11427](https://gitlab.com/gnachman/iterm2/-/issues/11427) | AI prompt suggestions | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11416](https://gitlab.com/gnachman/iterm2/-/issues/11416) | Using AI suggestions with Azure OpenAI backend | - | - | - | - | - | Skip | Feature request - Azure now detected |
| [#11260](https://gitlab.com/gnachman/iterm2/-/issues/11260) | Need help to setup AI feature of iterm2 | - | - | - | - | - | Skip | Support question - not a bug |
| [#6955](https://gitlab.com/gnachman/iterm2/-/issues/6955) | enhancement - us AI to capture errors / look up solutions | - | - | - | - | - | Skip | Feature request - not a bug |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 33 |
| Fixed | 5 |
| In Progress | 0 |
| Inspected | 0 |
| Open | 0 |
| Skip | 18 |
| External | 4 |
| Cannot Reproduce | 5 |
| Wontfix | 1 |

---

## Category Notes

DashTerm2 has significantly modernized AI integration since the original iTerm2 issues were filed. Key improvements:

1. **Multi-provider support**: OpenAI (GPT-5, o3/o4), Anthropic (Claude 4.x), Google (Gemini 2.x), DeepSeek, and Llama (via Ollama)
2. **Proper token handling**: `LLMProvider.maxTokens()` respects per-model `maxResponseTokens` limits
3. **Modern APIs**: Uses OpenAI responses API instead of legacy completions
4. **Model metadata**: `AIMetadata.swift` defines accurate context windows and response limits

### Common Patterns

- **max_tokens errors**: Fixed by per-model limits in AIMetadata
- **Plugin not found**: External installation issues, not code bugs
- **API compatibility**: Modern code uses newer API endpoints

### Related Files

- `sources/AIMetadata.swift` - Model definitions and token limits
- `sources/LLMProvider.swift` - Provider abstraction and token calculation
- `sources/CompletionsOpenAI.swift` - OpenAI completions API
- `sources/LegacyOpenAI.swift` - Legacy OpenAI API
- `sources/Llama.swift` - Ollama/Llama integration
- `sources/DeepSeek.swift` - DeepSeek integration
- `sources/CompletionsAnthropic.swift` - Anthropic/Claude integration

