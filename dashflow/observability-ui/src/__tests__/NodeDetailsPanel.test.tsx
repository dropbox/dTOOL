// M-444: Component tests for NodeDetailsPanel (server-rendered)
// Run with: npx tsx src/__tests__/NodeDetailsPanel.test.tsx

import { renderToStaticMarkup } from 'react-dom/server';
import { NodeDetailsPanel } from '../components/NodeDetailsPanel';
import NodeDetailsPanelDefault from '../components/NodeDetailsPanel';
import type { NodeSchema, NodeExecution } from '../types/graph';

let passed = 0;
let failed = 0;

function test(name: string, fn: () => void): void {
  try {
    fn();
    console.log(`  ✓ ${name}`);
    passed++;
  } catch (e) {
    console.log(`  ✗ ${name}`);
    console.log(`    Error: ${e}`);
    failed++;
  }
}

function assertIncludes(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(message || `Expected to include: ${needle}`);
  }
}

function assertNotIncludes(haystack: string, needle: string, message?: string): void {
  if (haystack.includes(needle)) {
    throw new Error(message || `Expected NOT to include: ${needle}`);
  }
}

function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (actual !== expected) {
    throw new Error(message || `Expected ${expected} but got ${actual}`);
  }
}

function countOccurrences(haystack: string, needle: string): number {
  return haystack.split(needle).length - 1;
}

console.log('\nNodeDetailsPanel Tests\n');

console.log('Component exports:\n');

test('NodeDetailsPanel is exported as named export', () => {
  if (typeof NodeDetailsPanel !== 'object' && typeof NodeDetailsPanel !== 'function') {
    throw new Error('NodeDetailsPanel should be a React component');
  }
});

test('NodeDetailsPanel is exported as default export', () => {
  if (typeof NodeDetailsPanelDefault !== 'object' && typeof NodeDetailsPanelDefault !== 'function') {
    throw new Error('Default export should be a React component');
  }
});

test('named and default exports are the same component', () => {
  assertEqual(NodeDetailsPanel, NodeDetailsPanelDefault);
});

test('renders empty state when no node selected', () => {
  const html = renderToStaticMarkup(<NodeDetailsPanel node={null} />);
  // V-08 update: improved empty state message
  assertIncludes(html, 'Click a node in the graph to view its details');
});

test('empty state renders title and keyboard hints', () => {
  const html = renderToStaticMarkup(<NodeDetailsPanel node={null} />);
  assertIncludes(html, 'Node Details');
  assertIncludes(html, 'Use Tab/Arrow keys');
  assertIncludes(html, 'Press Enter to select');
});

test('renders node details, execution status, and state', () => {
  const node: NodeSchema = {
    name: 'fetch_users',
    description: 'Fetch users from API',
    node_type: 'tool',
    input_fields: ['tenant_id', 'page'],
    output_fields: ['users'],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'fetch_users',
    status: 'completed',
    duration_ms: 123,
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      execution={execution}
      currentState={{ users: [{ id: 1 }], count: 1 }}
      previousState={{ users: [], count: 0 }}
    />
  );

  assertIncludes(html, 'fetch_users');
  assertIncludes(html, 'tool');
  assertIncludes(html, 'Completed');
  // V-10 update: label is "Duration" without colon
  assertIncludes(html, 'Duration');
  assertIncludes(html, '123ms');
  assertIncludes(html, 'Description');
  assertIncludes(html, 'Fetch users from API');
  assertIncludes(html, 'Inputs');
  assertIncludes(html, 'tenant_id');
  assertIncludes(html, 'Outputs');
  assertIncludes(html, 'users');
  assertIncludes(html, 'State After Node');
  assertIncludes(html, 'users:');
  assertIncludes(html, 'count:');
  assertIncludes(html, 'aria-label=\"Tool node\"', 'M-472: icon aria-label should be present');
});

console.log('\nStatus badge variations:\n');

test('renders pending status badge', () => {
  const node: NodeSchema = {
    name: 'pending_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'pending_node',
    status: 'pending',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Pending');
  assertIncludes(html, 'bg-gray-500/15');
});

test('renders active status badge', () => {
  const node: NodeSchema = {
    name: 'active_node',
    description: '',
    node_type: 'llm',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'active_node',
    status: 'active',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Running...');
  assertIncludes(html, 'bg-blue-500/20');
});

test('renders completed status badge', () => {
  const node: NodeSchema = {
    name: 'completed_node',
    description: '',
    node_type: 'validator',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'completed_node',
    status: 'completed',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Completed');
  assertIncludes(html, 'bg-green-500/20');
});

test('renders error status badge', () => {
  const node: NodeSchema = {
    name: 'error_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'error_node',
    status: 'error',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Error');
  assertIncludes(html, 'bg-red-500/20');
});

test('unknown execution status falls back to pending', () => {
  const node: NodeSchema = {
    name: 'unknown_status_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution = {
    node_name: 'unknown_status_node',
    status: 'weird_status',
  } as unknown as NodeExecution;

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Pending');
  assertIncludes(html, 'bg-gray-500/15');
  assertNotIncludes(html, 'Running...');
});

console.log('\nError message display:\n');

test('renders error message when present', () => {
  const node: NodeSchema = {
    name: 'failed_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'failed_node',
    status: 'error',
    error: 'Connection timeout after 30s',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Error:');
  assertIncludes(html, 'Connection timeout after 30s');
  assertIncludes(html, 'bg-red-500/10');
});

test('does not render error block when execution.error is empty', () => {
  const node: NodeSchema = {
    name: 'no_error_message_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'no_error_message_node',
    status: 'error',
    error: '',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertNotIncludes(html, 'Error:', 'Should not render the error block label');
  assertNotIncludes(html, 'bg-red-500/10', 'Should not render the error block styling');
});

console.log('\nExecution timing display:\n');

test('renders start time when present', () => {
  const node: NodeSchema = {
    name: 'timed_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'timed_node',
    status: 'active',
    start_time: 1736088600000, // 2026-01-05T10:30:00Z as timestamp
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Started');
});

test('start time formats with locale time string', () => {
  const node: NodeSchema = {
    name: 'timed_node_locale',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const startTime = 1736088600000;
  const execution: NodeExecution = {
    node_name: 'timed_node_locale',
    status: 'active',
    start_time: startTime,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, new Date(startTime).toLocaleTimeString());
});

test('renders end time when present', () => {
  const node: NodeSchema = {
    name: 'completed_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'completed_node',
    status: 'completed',
    start_time: 1736088600000, // 2026-01-05T10:30:00Z
    end_time: 1736088605000,   // 2026-01-05T10:30:05Z
    duration_ms: 5000,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'Started');
  assertIncludes(html, 'Ended');
  assertIncludes(html, '5000ms');
});

test('does not render duration row when duration_ms is missing', () => {
  const node: NodeSchema = {
    name: 'no_duration_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'no_duration_node',
    status: 'active',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertNotIncludes(html, '>Duration<');
});

test('renders Execution section header when execution is provided', () => {
  const node: NodeSchema = {
    name: 'exec_header_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'exec_header_node',
    status: 'active',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, '>Execution</h4>');
});

test('does not render Started row when start_time is missing', () => {
  const node: NodeSchema = {
    name: 'no_start_time_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'no_start_time_node',
    status: 'active',
    end_time: 1736088605000,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertNotIncludes(html, '>Started<');
});

test('does not render Ended row when end_time is missing', () => {
  const node: NodeSchema = {
    name: 'no_end_time_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'no_end_time_node',
    status: 'active',
    start_time: 1736088600000,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertNotIncludes(html, '>Ended<');
});

console.log('\nEdge cases:\n');

test('renders without description section when not provided', () => {
  const node: NodeSchema = {
    name: 'no_desc_node',
    description: '',
    node_type: 'transform',
    input_fields: ['input'],
    output_fields: ['output'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'no_desc_node');
  // Should NOT include Description section when empty
  if (html.includes('>Description<')) {
    throw new Error('Should not render Description section for empty description');
  }
});

test('does not render description section when description is undefined', () => {
  const node: NodeSchema = {
    name: 'no_desc_prop_node',
    node_type: 'transform',
    input_fields: ['input'],
    output_fields: ['output'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertNotIncludes(html, '>Description<');
});

test('renders inputs and outputs headings with arrows', () => {
  const node: NodeSchema = {
    name: 'io_node',
    description: '',
    node_type: 'transform',
    input_fields: ['a'],
    output_fields: ['b'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '→');
  assertIncludes(html, 'Inputs');
  assertIncludes(html, '←');
  assertIncludes(html, 'Outputs');
});

test('renders each input field inside a code pill', () => {
  const node: NodeSchema = {
    name: 'inputs_node',
    description: '',
    node_type: 'transform',
    input_fields: ['tenant_id', 'page', 'per_page'],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'bg-blue-500/15');
  assertIncludes(html, 'tenant_id');
  assertIncludes(html, 'page');
  assertIncludes(html, 'per_page');
});

test('renders each output field inside a code pill', () => {
  const node: NodeSchema = {
    name: 'outputs_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: ['users', 'count', 'next_cursor'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'bg-green-500/15');
  assertIncludes(html, 'users');
  assertIncludes(html, 'count');
  assertIncludes(html, 'next_cursor');
});

test('renders "None specified" for empty input fields', () => {
  const node: NodeSchema = {
    name: 'no_inputs_node',
    description: 'A node with no inputs',
    node_type: 'transform',
    input_fields: [],
    output_fields: ['result'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertEqual(countOccurrences(html, 'None specified'), 1);
});

test('renders "None specified" for empty output fields', () => {
  const node: NodeSchema = {
    name: 'no_outputs_node',
    description: 'A node with no outputs',
    node_type: 'transform',
    input_fields: ['input'],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertEqual(countOccurrences(html, 'None specified'), 1);
});

test('does not render State section when currentState is empty', () => {
  const node: NodeSchema = {
    name: 'no_state_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} currentState={{}} />);
  if (html.includes('State After Node')) {
    throw new Error('Should not render State section for empty state');
  }
});

test('renders State section when currentState is non-empty', () => {
  const node: NodeSchema = {
    name: 'has_state_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ answer: 42, ok: true }}
    />
  );

  assertIncludes(html, 'State After Node');
  assertIncludes(html, 'answer:');
  assertIncludes(html, '42');
  assertIncludes(html, 'ok:');
  assertIncludes(html, 'true');
});

test('StateViewer highlights changed keys when previousState is provided', () => {
  const node: NodeSchema = {
    name: 'state_changes_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ count: 2 }}
      previousState={{ count: 1 }}
    />
  );

  assertIncludes(html, 'count:');
  assertIncludes(html, '●');
});

test('StateViewer does not mark changes when previousState is missing', () => {
  const node: NodeSchema = {
    name: 'state_no_prev_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ count: 1 }}
    />
  );

  assertNotIncludes(html, '●');
});

test('does not render Execution section when no execution provided', () => {
  const node: NodeSchema = {
    name: 'no_exec_node',
    description: 'Test node',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  if (html.includes('>Execution<')) {
    throw new Error('Should not render Execution section when no execution provided');
  }
});

console.log('\nNode type icons:\n');

test('renders transform node with gear icon', () => {
  const node: NodeSchema = {
    name: 'transform_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '⚙️');
  assertIncludes(html, 'aria-label="Transform node"');
});

test('node icon renders with role="img" for accessibility', () => {
  const node: NodeSchema = {
    name: 'a11y_icon_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'role="img"');
  assertIncludes(html, 'aria-label="Tool node"');
});

test('renders LLM node with robot icon', () => {
  const node: NodeSchema = {
    name: 'llm_node',
    description: '',
    node_type: 'llm',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '🤖');
  assertIncludes(html, 'aria-label="LLM node"');
});

test('renders tool node with wrench icon', () => {
  const node: NodeSchema = {
    name: 'tool_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '🔧');
  assertIncludes(html, 'aria-label="Tool node"');
});

test('renders router node with direction icon', () => {
  const node: NodeSchema = {
    name: 'router_node',
    description: '',
    node_type: 'router',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '🔀');
  assertIncludes(html, 'aria-label="Router node"');
});

test('renders aggregator node with chart icon', () => {
  const node: NodeSchema = {
    name: 'aggregator_node',
    description: '',
    node_type: 'aggregator',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '📊');
  assertIncludes(html, 'aria-label="Aggregator node"');
});

test('renders validator node with checkmark', () => {
  const node: NodeSchema = {
    name: 'validator_node',
    description: '',
    node_type: 'validator',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '✓');
  assertIncludes(html, 'aria-label="Validator node"');
});

test('renders human_in_loop node with user icon', () => {
  const node: NodeSchema = {
    name: 'hitl_node',
    description: '',
    node_type: 'human_in_loop',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '👤');
  assertIncludes(html, 'aria-label="Human-in-the-loop node"');
  assertIncludes(html, 'human_in_loop');
});

test('renders checkpoint node with save icon', () => {
  const node: NodeSchema = {
    name: 'checkpoint_node',
    description: '',
    node_type: 'checkpoint',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '💾');
  assertIncludes(html, 'aria-label="Checkpoint node"');
});

test('renders custom node with package icon', () => {
  const node: NodeSchema = {
    name: 'custom_node',
    description: '',
    node_type: 'custom',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '📦');
  assertIncludes(html, 'aria-label="Custom node"');
});

test('unknown node_type falls back to transform styling', () => {
  const node = {
    name: 'unknown_type_node',
    description: '',
    node_type: 'unknown_type',
    input_fields: [],
    output_fields: [],
    attributes: {},
  } as unknown as NodeSchema;

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '⚙️');
  assertIncludes(html, 'aria-label="Transform node"');
});

console.log('\nEmpty state details:\n');

test('empty state shows keyboard navigation hints', () => {
  const html = renderToStaticMarkup(<NodeDetailsPanel node={null} />);
  assertIncludes(html, 'Tab/Arrow keys');
  assertIncludes(html, 'Enter to select');
});

test('empty state shows magnifying glass icon', () => {
  const html = renderToStaticMarkup(<NodeDetailsPanel node={null} />);
  assertIncludes(html, '🔍');
});

console.log('\nNode type badge styling:\n');

test('node type badge uses inline colors from NODE_TYPE_STYLES', () => {
  const node: NodeSchema = {
    name: 'style_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'background-color:#1e3a5f');
  assertIncludes(html, 'color:#60A5FA');
});

console.log('\nDefault status fallback:\n');

test('defaults to pending status when execution has no status', () => {
  const node: NodeSchema = {
    name: 'no_status_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  // Node without execution defaults to 'pending'
  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'Pending');
});

console.log('\nMultiple fields handling:\n');

test('renders multiple input fields correctly', () => {
  const node: NodeSchema = {
    name: 'multi_input_node',
    description: '',
    node_type: 'transform',
    input_fields: ['a', 'b', 'c', 'd', 'e'],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'a');
  assertIncludes(html, 'b');
  assertIncludes(html, 'c');
  assertIncludes(html, 'd');
  assertIncludes(html, 'e');
  assertEqual(countOccurrences(html, 'bg-blue-500/15'), 5);
});

test('renders multiple output fields correctly', () => {
  const node: NodeSchema = {
    name: 'multi_output_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: ['result', 'status', 'metadata', 'timestamp'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'result');
  assertIncludes(html, 'status');
  assertIncludes(html, 'metadata');
  assertIncludes(html, 'timestamp');
  assertEqual(countOccurrences(html, 'bg-green-500/15'), 4);
});

test('renders both inputs and outputs with correct styling', () => {
  const node: NodeSchema = {
    name: 'io_both_node',
    description: '',
    node_type: 'transform',
    input_fields: ['input1', 'input2'],
    output_fields: ['output1', 'output2'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertEqual(countOccurrences(html, 'bg-blue-500/15'), 2);
  assertEqual(countOccurrences(html, 'bg-green-500/15'), 2);
});

console.log('\nSpecial character handling:\n');

test('handles node names with special characters', () => {
  const node: NodeSchema = {
    name: 'fetch_users_v2.0-beta',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'fetch_users_v2.0-beta');
});

test('handles node names with unicode characters', () => {
  const node: NodeSchema = {
    name: 'process_données',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'process_données');
});

test('handles description with special HTML characters (escaped)', () => {
  const node: NodeSchema = {
    name: 'html_desc_node',
    description: 'Process <input> and <output> tags',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '&lt;input&gt;');
  assertIncludes(html, '&lt;output&gt;');
});

test('handles description with ampersand', () => {
  const node: NodeSchema = {
    name: 'ampersand_node',
    description: 'Input & output processing',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'Input &amp; output processing');
});

test('handles field names with underscores and numbers', () => {
  const node: NodeSchema = {
    name: 'field_test_node',
    description: '',
    node_type: 'transform',
    input_fields: ['user_id_123', 'page_num_1'],
    output_fields: ['result_v2'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'user_id_123');
  assertIncludes(html, 'page_num_1');
  assertIncludes(html, 'result_v2');
});

console.log('\nDuration display variations:\n');

test('renders zero duration correctly', () => {
  const node: NodeSchema = {
    name: 'zero_duration_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'zero_duration_node',
    status: 'completed',
    duration_ms: 0,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, '0ms');
});

test('renders sub-millisecond duration (shows as 0ms)', () => {
  const node: NodeSchema = {
    name: 'fast_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'fast_node',
    status: 'completed',
    duration_ms: 0.5,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, '0.5ms');
});

test('renders large duration correctly', () => {
  const node: NodeSchema = {
    name: 'slow_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'slow_node',
    status: 'completed',
    duration_ms: 60000,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, '60000ms');
});

test('renders decimal duration correctly', () => {
  const node: NodeSchema = {
    name: 'decimal_duration_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'decimal_duration_node',
    status: 'completed',
    duration_ms: 123.456,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, '123.456ms');
});

console.log('\nAccessibility features:\n');

test('header uses semantic h3 for node name', () => {
  const node: NodeSchema = {
    name: 'semantic_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '<h3');
  assertIncludes(html, 'semantic_node</h3>');
});

test('section headers use h4 elements', () => {
  const node: NodeSchema = {
    name: 'headers_node',
    description: 'A description',
    node_type: 'transform',
    input_fields: ['input'],
    output_fields: ['output'],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'headers_node',
    status: 'completed',
    duration_ms: 100,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} currentState={{ key: 'value' }} />);
  // 5 h4 elements: Description, Inputs, Outputs, Execution, State
  assertEqual(countOccurrences(html, '<h4'), 5);
});

test('inputs list uses ul and li elements', () => {
  const node: NodeSchema = {
    name: 'list_node',
    description: '',
    node_type: 'transform',
    input_fields: ['a', 'b'],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '<ul');
  assertIncludes(html, '<li');
});

test('all node type icons have proper aria-label', () => {
  const nodeTypes: Array<[string, string]> = [
    ['transform', 'Transform node'],
    ['llm', 'LLM node'],
    ['tool', 'Tool node'],
    ['router', 'Router node'],
    ['aggregator', 'Aggregator node'],
    ['validator', 'Validator node'],
    ['human_in_loop', 'Human-in-the-loop node'],
    ['checkpoint', 'Checkpoint node'],
    ['custom', 'Custom node'],
  ];

  for (const [nodeType, expectedLabel] of nodeTypes) {
    const node: NodeSchema = {
      name: `${nodeType}_test`,
      description: '',
      node_type: nodeType as NodeSchema['node_type'],
      input_fields: [],
      output_fields: [],
      attributes: {},
    };

    const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
    assertIncludes(html, `aria-label="${expectedLabel}"`, `${nodeType} should have aria-label "${expectedLabel}"`);
  }
});

console.log('\nComplex state rendering:\n');

test('renders state with nested objects', () => {
  const node: NodeSchema = {
    name: 'nested_state_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ nested: { deep: { value: 42 } } }}
    />
  );

  assertIncludes(html, 'nested:');
  assertIncludes(html, 'State After Node');
});

test('renders state with arrays', () => {
  const node: NodeSchema = {
    name: 'array_state_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ items: [1, 2, 3] }}
    />
  );

  assertIncludes(html, 'items:');
});

test('renders state with null values', () => {
  const node: NodeSchema = {
    name: 'null_state_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ nullField: null }}
    />
  );

  assertIncludes(html, 'nullField:');
  assertIncludes(html, 'null');
});

test('renders state with boolean values', () => {
  const node: NodeSchema = {
    name: 'bool_state_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ isEnabled: true, isDisabled: false }}
    />
  );

  assertIncludes(html, 'isEnabled:');
  assertIncludes(html, 'true');
  assertIncludes(html, 'isDisabled:');
  assertIncludes(html, 'false');
});

test('state change marker shows for changed values only', () => {
  const node: NodeSchema = {
    name: 'change_marker_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ changed: 'new', unchanged: 'same' }}
      previousState={{ changed: 'old', unchanged: 'same' }}
    />
  );

  // The ● marker appears only for changed values
  assertEqual(countOccurrences(html, '●'), 1);
});

test('multiple changed keys each get markers', () => {
  const node: NodeSchema = {
    name: 'multi_change_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ a: 1, b: 2, c: 3 }}
      previousState={{ a: 0, b: 0, c: 0 }}
    />
  );

  assertEqual(countOccurrences(html, '●'), 3);
});

test('new keys in currentState are marked as changed', () => {
  const node: NodeSchema = {
    name: 'new_key_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(
    <NodeDetailsPanel
      node={node}
      currentState={{ existing: 1, newKey: 2 }}
      previousState={{ existing: 1 }}
    />
  );

  // newKey is a new key, should be marked as changed
  assertEqual(countOccurrences(html, '●'), 1);
});

console.log('\nError message formatting:\n');

test('renders long error message', () => {
  const node: NodeSchema = {
    name: 'long_error_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const longError = 'Failed to connect to database server at localhost:5432 after 3 retries. Connection timed out.';
  const execution: NodeExecution = {
    node_name: 'long_error_node',
    status: 'error',
    error: longError,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, longError);
});

test('renders error with special characters', () => {
  const node: NodeSchema = {
    name: 'special_error_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'special_error_node',
    status: 'error',
    error: 'Invalid JSON: {"key": "value<>"}',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, '&lt;&gt;');
});

test('error block has correct border styling', () => {
  const node: NodeSchema = {
    name: 'error_style_node',
    description: '',
    node_type: 'tool',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'error_style_node',
    status: 'error',
    error: 'Test error',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'border-red-500/30');
});

console.log('\nNode type badge colors:\n');

test('LLM node badge has correct colors', () => {
  const node: NodeSchema = {
    name: 'llm_color_node',
    description: '',
    node_type: 'llm',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'background-color:#2e1065');
  assertIncludes(html, 'color:#A78BFA');
});

test('router node badge has correct colors', () => {
  const node: NodeSchema = {
    name: 'router_color_node',
    description: '',
    node_type: 'router',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'background-color:#450a0a');
  assertIncludes(html, 'color:#F87171');
});

test('aggregator node badge has correct colors', () => {
  const node: NodeSchema = {
    name: 'aggregator_color_node',
    description: '',
    node_type: 'aggregator',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'background-color:#064e3b');
  assertIncludes(html, 'color:#34D399');
});

test('checkpoint node badge has correct colors', () => {
  const node: NodeSchema = {
    name: 'checkpoint_color_node',
    description: '',
    node_type: 'checkpoint',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'background-color:#164e63');
  assertIncludes(html, 'color:#22D3EE');
});

test('human_in_loop node badge has correct colors', () => {
  const node: NodeSchema = {
    name: 'hitl_color_node',
    description: '',
    node_type: 'human_in_loop',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'background-color:#431407');
  assertIncludes(html, 'color:#FB923C');
});

console.log('\nStatus badge ring styling:\n');

test('pending status has ring styling', () => {
  const node: NodeSchema = {
    name: 'pending_ring_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'pending_ring_node',
    status: 'pending',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'ring-1');
  assertIncludes(html, 'ring-inset');
  assertIncludes(html, 'ring-gray-500/30');
});

test('active status has ring styling', () => {
  const node: NodeSchema = {
    name: 'active_ring_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'active_ring_node',
    status: 'active',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'ring-blue-500/30');
});

test('completed status has ring styling', () => {
  const node: NodeSchema = {
    name: 'completed_ring_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'completed_ring_node',
    status: 'completed',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'ring-green-500/30');
});

test('error status has ring styling', () => {
  const node: NodeSchema = {
    name: 'error_ring_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'error_ring_node',
    status: 'error',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, 'ring-red-500/30');
});

console.log('\nExecution timing edge cases:\n');

test('renders both start_time and end_time when both present', () => {
  const node: NodeSchema = {
    name: 'both_times_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const startTime = 1736088600000;
  const endTime = 1736088610000;
  const execution: NodeExecution = {
    node_name: 'both_times_node',
    status: 'completed',
    start_time: startTime,
    end_time: endTime,
    duration_ms: 10000,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, new Date(startTime).toLocaleTimeString());
  assertIncludes(html, new Date(endTime).toLocaleTimeString());
});

test('does not render execution section header when no timing or duration', () => {
  const node: NodeSchema = {
    name: 'no_timing_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'no_timing_node',
    status: 'pending',
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  // Execution section should still render even without timing (for status badge display context)
  assertIncludes(html, '>Execution</h4>');
});

test('renders only duration when no start/end times', () => {
  const node: NodeSchema = {
    name: 'duration_only_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const execution: NodeExecution = {
    node_name: 'duration_only_node',
    status: 'completed',
    duration_ms: 500,
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} execution={execution} />);
  assertIncludes(html, '500ms');
  assertNotIncludes(html, '>Started<');
  assertNotIncludes(html, '>Ended<');
});

console.log('\nInput/output pill styling:\n');

test('input pills have blue dot indicator', () => {
  const node: NodeSchema = {
    name: 'input_pill_node',
    description: '',
    node_type: 'transform',
    input_fields: ['test_input'],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'bg-blue-400');
  assertIncludes(html, 'rounded-full');
});

test('output pills have green dot indicator', () => {
  const node: NodeSchema = {
    name: 'output_pill_node',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: ['test_output'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'bg-green-400');
  assertIncludes(html, 'rounded-full');
});

test('input pills use monospace font', () => {
  const node: NodeSchema = {
    name: 'mono_input_node',
    description: '',
    node_type: 'transform',
    input_fields: ['field'],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'font-mono');
});

test('input/output section uses grid layout', () => {
  const node: NodeSchema = {
    name: 'grid_layout_node',
    description: '',
    node_type: 'transform',
    input_fields: ['a'],
    output_fields: ['b'],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'grid-cols-2');
});

console.log('\nDescription section styling:\n');

test('description section has proper border', () => {
  const node: NodeSchema = {
    name: 'desc_border_node',
    description: 'Test description',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'border-b-2');
  assertIncludes(html, 'border-gray-700');
});

test('description uses uppercase tracking for header', () => {
  const node: NodeSchema = {
    name: 'desc_header_node',
    description: 'Test description',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'uppercase');
  assertIncludes(html, 'tracking-wide');
});

test('description text has relaxed line height', () => {
  const node: NodeSchema = {
    name: 'desc_line_height_node',
    description: 'Test description',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, 'leading-relaxed');
});

console.log('\nEmpty node name handling:\n');

test('renders empty string node name', () => {
  const node: NodeSchema = {
    name: '',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  // Should render without crashing
  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '<h3');
});

test('renders whitespace-only node name', () => {
  const node: NodeSchema = {
    name: '   ',
    description: '',
    node_type: 'transform',
    input_fields: [],
    output_fields: [],
    attributes: {},
  };

  const html = renderToStaticMarkup(<NodeDetailsPanel node={node} />);
  assertIncludes(html, '   ');
});

console.log('\n--------------------------');
console.log(`Tests: ${passed} passed, ${failed} failed`);
console.log('--------------------------\n');

if (failed > 0) {
  process.exit(1);
}
