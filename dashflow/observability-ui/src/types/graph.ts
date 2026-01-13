// Graph visualization types

export type NodeType =
  | 'transform'
  | 'llm'
  | 'tool'
  | 'router'
  | 'aggregator'
  | 'validator'
  | 'human_in_loop'
  | 'checkpoint'
  | 'custom';

export type EdgeType = 'direct' | 'conditional' | 'parallel';

export type NodeStatus = 'pending' | 'active' | 'completed' | 'error';

export interface NodeMetadata {
  description?: string;
  node_type: NodeType;
  input_fields: string[];
  output_fields: string[];
  position?: [number, number];
  attributes: Record<string, string>;
}

export interface NodeSchema {
  name: string;
  description?: string;
  node_type: NodeType;
  input_fields: string[];
  output_fields: string[];
  position?: [number, number];
  attributes: Record<string, string>;
}

export interface EdgeSchema {
  from: string;
  to: string;
  edge_type: EdgeType;
  label?: string;
  conditional_targets?: string[];
}

export interface GraphSchema {
  name: string;
  version: string;
  description?: string;
  nodes: NodeSchema[];
  edges: EdgeSchema[];
  entry_point: string;
  state_type?: string;
  exported_at?: string;
  metadata: Record<string, string>;
}

export interface NodeExecution {
  node_name: string;
  status: NodeStatus;
  start_time?: number;
  end_time?: number;
  duration_ms?: number;
  input_state?: Record<string, unknown>;
  output_state?: Record<string, unknown>;
  error?: string;
}

export interface GraphExecution {
  graph_id: string;
  graph_name: string;
  thread_id: string;
  schema: GraphSchema;
  schema_id?: string; // Content-addressed schema version ID
  current_node?: string;
  node_executions: Record<string, NodeExecution>;
  state: Record<string, unknown>;
  status: 'running' | 'completed' | 'error';
  start_time: number;
  end_time?: number;
}

// Node type styling with accessibility labels (M-472)
// ariaLabel provides screen reader text since emojis are read inconsistently
export interface NodeTypeStyle {
  icon: string;
  color: string;
  bgColor: string;
  ariaLabel: string; // M-472: Screen reader accessible label for the node type icon
}

// GN-01: Dark theme node backgrounds for proper contrast on dark canvas
export const NODE_TYPE_STYLES: Record<NodeType, NodeTypeStyle> = {
  transform: { icon: '⚙️', color: '#9CA3AF', bgColor: '#1f2937', ariaLabel: 'Transform node' },
  llm: { icon: '🤖', color: '#A78BFA', bgColor: '#2e1065', ariaLabel: 'LLM node' },
  tool: { icon: '🔧', color: '#60A5FA', bgColor: '#1e3a5f', ariaLabel: 'Tool node' },
  router: { icon: '🔀', color: '#F87171', bgColor: '#450a0a', ariaLabel: 'Router node' },
  aggregator: { icon: '📊', color: '#34D399', bgColor: '#064e3b', ariaLabel: 'Aggregator node' },
  validator: { icon: '✓', color: '#4ADE80', bgColor: '#14532d', ariaLabel: 'Validator node' },
  human_in_loop: { icon: '👤', color: '#FB923C', bgColor: '#431407', ariaLabel: 'Human-in-the-loop node' },
  checkpoint: { icon: '💾', color: '#22D3EE', bgColor: '#164e63', ariaLabel: 'Checkpoint node' },
  custom: { icon: '📦', color: '#9CA3AF', bgColor: '#1f2937', ariaLabel: 'Custom node' },
};

export const NODE_STATUS_STYLES: Record<NodeStatus, { borderColor: string; pulseColor?: string }> = {
  pending: { borderColor: '#4B5563' }, // GN-09: Dark gray that contrasts with dark node backgrounds
  active: { borderColor: '#3B82F6', pulseColor: '#60A5FA' },
  completed: { borderColor: '#22C55E' },
  error: { borderColor: '#EF4444' },
};
