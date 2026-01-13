import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import {
  LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer,
  BarChart, Bar, AreaChart, Area, PieChart, Pie, Cell
} from 'recharts';
import { GraphCanvas } from './components/GraphCanvas';
import { NodeDetailsPanel } from './components/NodeDetailsPanel';
import { ExecutionTimeline, TimelineEvent } from './components/ExecutionTimeline';
import { StateDiffViewer } from './components/StateDiffViewer';
import { TimelineSlider } from './components/TimelineSlider';
import { MermaidView } from './components/MermaidView';
import { SchemaHistoryPanel, SchemaObservation } from './components/SchemaHistoryPanel';
import { ErrorBoundary } from './components/ErrorBoundary';
// I-13: Import Tooltip component for metric explanations (renamed to avoid recharts collision)
import { Tooltip as MetricTooltip } from './components/Tooltip';
import { GraphSchema, NodeExecution } from './types/graph';
import { getEventTypeName, EventType, EXPECTED_SCHEMA_VERSION, MAX_DECOMPRESSED_SIZE } from './proto/dashstream';
// M-998: Use Web Worker for decode/decompress to prevent main thread freezes
import { getDecodeWorkerPool, DecodeWorkerPool } from './workers/DecodeWorkerPool';
// useRunStateStore is now the single source of truth for graph state
// M-123: parseConfigFromUrl reads URL params for runtime config overrides
import { useRunStateStore, parseConfigFromUrl } from './hooks/useRunStateStore';
import type { GroupingMode } from './utils/grouping';
import { formatUptime, formatTimestamp, formatKafkaStatus } from './utils/timeFormat';
import { getStringAttribute } from './utils/attributes';
import { colors, shadows } from './styles/tokens';

// Helper to construct WebSocket URL with correct protocol (ws:// or wss://)
function getWebSocketUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws`;
}

// Debug logging gated behind localStorage flag
// Set localStorage.setItem('dashflow.debug', 'true') to enable verbose logging
const DEBUG_ENABLED = typeof window !== 'undefined' &&
  window.localStorage?.getItem('dashflow.debug') === 'true';

function debugLog(...args: unknown[]): void {
  if (DEBUG_ENABLED) {
    console.log(...args);
  }
}

function debugWarn(...args: unknown[]): void {
  if (DEBUG_ENABLED) {
    console.warn(...args);
  }
}

// Error logging is always enabled (important for debugging issues)
function logError(...args: unknown[]): void {
  console.error(...args);
}

// Types for the dashboard
// M-693: sequence is string to prevent precision loss for values > 2^53
interface DashStreamEvent {
  timestamp: number;
  type: string;
  thread_id?: string;
  quality?: number;
  model?: string;
  sequence?: string;
}

// M-1099: ReplayBuffer metrics snapshot from /health response
interface ReplayBufferMetricsSnapshot {
  redis_enabled: boolean;
  memory_hits: number;
  redis_hits: number;
  redis_misses: number;
  redis_write_dropped: number;
  redis_write_failures: number;
  // M-1022: Operational metrics for predicting cursor_stale/eviction
  memory_buffer_size: number;      // Current messages in memory buffer
  max_memory_size: number;         // Configured max memory buffer capacity
  redis_message_ttl_secs: number;  // Configured Redis TTL (retention)
  redis_max_sequences: number;     // Max sequences retained per thread in Redis
}

interface HealthResponse {
  status: string;
  metrics: {
    kafka_messages_received: number;
    kafka_errors: number;
    infrastructure_errors: number;
    connected_clients: number;
    uptime_seconds: number;
    last_kafka_message_ago_seconds: number | null;
    dropped_messages: number;
    decode_errors: number;
    // M-1100: Old data decode errors (pre-session, expected during catch-up)
    old_data_decode_errors?: number;
    // M-1099: Windowed metrics (last 120s) for operational truth
    dropped_messages_last_120s?: number;
    last_drop_ago_seconds?: number | null;
    decode_errors_last_120s?: number;
    messages_last_120s?: number;
    last_infrastructure_error_ago_seconds?: number | null;
    // M-1072: Send failure counters
    send_failed?: number;
    send_timeout?: number;
  };
  // M-1099: Replay buffer metrics for operational visibility
  replay_buffer?: ReplayBufferMetricsSnapshot;
  kafka_status: string;
  websocket_status: string;
  alert?: string;
  circuit_breaker?: {
    state: string;
    degraded_duration_seconds?: number;
    time_until_restart_seconds?: number;
  };
}

interface VersionInfo {
  git_sha: string;
  build_date: string;
  schema_version: number;
  component: string;
  resume_namespace?: string;
  kafka_topic?: string;
  kafka_group_id?: string;
  // M-1019: Server's max payload config. Compare against UI's MAX_DECOMPRESSED_SIZE
  // to detect config drift (server accepts larger payloads than UI can decode).
  max_payload_bytes?: number;
  // M-1020: Server's decode error policy ("skip" or "pause").
  decode_error_policy?: string;
}

interface ThroughputDataPoint {
  time: string;
  messages: number;
  errors: number;
  // M-789/M-1066: Monotonic timestamp (performance.now()) for accurate rate computation
  // Note: Date.now() is NOT monotonic - can be affected by NTP adjustments
  tMs: number;
}

interface LatencyDataPoint {
  time: string;
  latency: number;
}

// Status colors - using design tokens for consistency
const STATUS_COLORS: Record<string, string> = {
  healthy: colors.connection.healthy,
  degraded: colors.connection.degraded,
  reconnecting: colors.connection.reconnecting,
  waiting: colors.connection.waiting,
  will_restart_soon: colors.connection.unavailable,
  stale: colors.connection.waiting,
  unavailable: colors.connection.unavailable,
};

const CHART_COLORS = [
  colors.chart.purple,
  colors.chart.green,
  colors.chart.yellow,
  colors.chart.orange,
  colors.chart.teal,
];

// M-803/M-804: Treat health as stale if no successful sample arrives within this window.
const HEALTH_STALE_MS = 15000;
// V-12: Metric card sparklines use last N samples for trend context.
const METRIC_SPARKLINE_POINTS = 60;

const DASHSTREAM_LAST_OFFSETS_STORAGE_KEY = 'dashstream_lastOffsetsByPartition_v1';
// M-681: Storage key for per-thread sequence cursors
const DASHSTREAM_LAST_SEQUENCES_STORAGE_KEY = 'dashstream_lastSequencesByThread_v1';

// M-678: Eviction limits to prevent unbounded localStorage growth
const MAX_STORED_PARTITIONS = 100;
const MAX_STORED_THREADS = 500;

// M-678: LRU eviction helper - keeps most recently updated entries
// M-727: Protects important keys (like partition "0") from eviction
// M-1058: Uses actual update timestamps for recency (not offset magnitude)
function evictOldestEntries<T extends string>(
  entries: Record<string, T>,
  maxEntries: number,
  _compareFn: (a: T, b: T) => number, // Legacy param, kept for signature compat
  protectedKeys: string[] = [],
  updatedAt?: Record<string, number> // M-1058: timestamp map for true LRU
): Record<string, T> {
  const keys = Object.keys(entries);
  if (keys.length <= maxEntries) return entries;

  // M-727: Separate protected keys from evictable keys
  const protectedSet = new Set(protectedKeys);
  const protectedEntries: [string, T][] = [];
  const evictableKeys: string[] = [];

  for (const key of keys) {
    if (protectedSet.has(key) && key in entries) {
      protectedEntries.push([key, entries[key]]);
    } else {
      evictableKeys.push(key);
    }
  }

  // M-1058: Sort evictable by update timestamp (most recent first)
  // If no timestamps provided, fall back to keeping keys with highest values (legacy behavior)
  const sortedEvictable = evictableKeys.sort((a, b) => {
    if (updatedAt) {
      // Most recent first (higher timestamp = more recent)
      return (updatedAt[b] ?? 0) - (updatedAt[a] ?? 0);
    }
    // Legacy fallback: highest offset/sequence first
    const aBig = BigInt(entries[a]);
    const bBig = BigInt(entries[b]);
    return aBig < bBig ? 1 : aBig > bBig ? -1 : 0;
  });

  // Build result: protected entries first, then fill with most recent evictable
  const result: Record<string, T> = {};
  for (const [key, val] of protectedEntries) {
    result[key] = val;
  }

  const remainingSlots = maxEntries - protectedEntries.length;
  for (let i = 0; i < remainingSlots && i < sortedEvictable.length; i++) {
    result[sortedEvictable[i]] = entries[sortedEvictable[i]];
  }
  return result;
}

// M-774: Binary decode/apply is serialized via a single promise chain. If any awaited step hangs,
// the chain wedges forever. Add bounded timeouts so we can force a reconnect and recover.
const BINARY_PROCESSING_STEP_TIMEOUT_MS = 30_000;

// M-1007: Hard cap on pending binary messages. If the backlog grows beyond this, the UI is falling
// behind and will likely never catch up without reconnecting. Force reconnect + resync.
// Prevents unbounded memory growth from queued closures/buffers.
const MAX_PENDING_BINARY_MESSAGES = 500;

function makeTimeoutError(label: string, ms: number): Error {
  const err = new Error(`${label} timed out after ${ms}ms`);
  err.name = 'TimeoutError';
  return err;
}

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  let timeoutId: number | null = null;
  const timeoutPromise = new Promise<T>((_, reject) => {
    timeoutId = window.setTimeout(() => reject(makeTimeoutError(label, ms)), ms);
  });

  return Promise.race([promise, timeoutPromise]).finally(() => {
    if (timeoutId !== null) {
      window.clearTimeout(timeoutId);
    }
  }) as Promise<T>;
}

function isTimeoutError(err: unknown): boolean {
  return err instanceof Error && err.name === 'TimeoutError';
}

function App() {
  // WebSocket state
  const [events, setEvents] = useState<DashStreamEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [wsError, setWsError] = useState<string | null>(null);
  // C-01: Track reconnection attempt for user feedback
  const [wsRetryAttempt, setWsRetryAttempt] = useState<{ attempt: number; maxRetries: number } | null>(null);
  // M-693: sequences are strings to prevent precision loss
  const lastSequencesByThreadRef = useRef<Record<string, string>>({});
  // M-707: Kafka offsets are strings to prevent precision loss for values > 2^53
  const lastOffsetsByPartitionRef = useRef<Record<string, string>>({});
  // M-1058: Track update timestamps for proper LRU eviction (recency, not magnitude)
  const offsetUpdatedAtRef = useRef<Record<string, number>>({});
  const seqUpdatedAtRef = useRef<Record<string, number>>({});
  const lastOffsetsPersistedAtRef = useRef<number>(0);
  // M-681: Track when thread sequences were last persisted
  const lastSequencesPersistedAtRef = useRef<number>(0);
  // M-707: offset is string to preserve precision for values > 2^53
  const pendingKafkaCursorRef = useRef<{ partition: number; offset: string } | null>(null);
  const binaryProcessingChainRef = useRef<Promise<void>>(Promise.resolve());
  const wsEpochRef = useRef<number>(0);
  const wsRef = useRef<WebSocket | null>(null);
  // M-720/M-723: If cursor↔binary pairing breaks, abort processing and reconnect.
  const wsProtocolErrorRef = useRef<boolean>(false);
  // M-774: If binary processing times out, force a reconnect (prevents promise chain wedge).
  const wsBinaryProcessingTimeoutRef = useRef<boolean>(false);
  // M-1007: If pending binary messages exceed MAX_PENDING_BINARY_MESSAGES, we are falling behind.
  // Force reconnect to prevent unbounded memory growth and let the server resend from last committed offset.
  const fallingBehindTriggeredRef = useRef<boolean>(false);
  // M-976: Track schema version mismatch to warn once and avoid spamming logs.
  // When true, cursor commits are gated to prevent advancing past potentially misinterpreted messages.
  const schemaVersionMismatchWarnedRef = useRef<boolean>(false);
  const schemaVersionMismatchActiveRef = useRef<boolean>(false);
  // M-997: Also surface mismatch state in UI and stop applying messages when active.
  const [schemaVersionMismatchInfo, setSchemaVersionMismatchInfo] = useState<{
    messageSchemaVersion: number;
    expectedSchemaVersion: number;
  } | null>(null);
  // M-451: Track reconnect timeout so we can cancel it on cleanup
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // M-459: Track retry count for exponential backoff
  const wsRetryCountRef = useRef(0);
  // M-998: Use Web Worker for decode/decompress to prevent main thread freezes
  const workerPoolRef = useRef<DecodeWorkerPool | null>(null);
  const workerPoolInitPromiseRef = useRef<Promise<void> | null>(null);
  const [, setDecoderReady] = useState(false);

  // M-705: Client-side apply lag metrics tracking
  // Tracks the time between message receipt and successful apply
  // M-1018: Added windowed metrics to surface recent spikes/regressions
  const applyLagMetricsRef = useRef({
    pendingCount: 0,       // Messages currently queued for processing
    totalApplied: 0,       // Total messages successfully applied (lifetime)
    totalLatencyMs: 0,     // Cumulative apply latency in ms (lifetime)
    lastReportTime: 0,     // Time of last metrics report
    maxLatencyMs: 0,       // Maximum apply latency seen (lifetime)
    // M-1018: Windowed metrics - 60 second sliding window
    windowMs: 60000,       // Window duration: 60 seconds
    recentSamples: [] as Array<{ timestamp: number; latencyMs: number }>,
  });

  // Health and metrics state
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthLastOkAt, setHealthLastOkAt] = useState<number | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [healthStale, setHealthStale] = useState(false);
  const [version, setVersion] = useState<VersionInfo | null>(null);
  const [versionLoaded, setVersionLoaded] = useState(false);
  const [resumeNamespace, setResumeNamespace] = useState<string | null>(null);
  // M-1019: Warning when server max_payload_bytes > UI MAX_DECOMPRESSED_SIZE
  const [configDriftWarning, setConfigDriftWarning] = useState<string | null>(null);
  const [throughputData, setThroughputData] = useState<ThroughputDataPoint[]>([]);
  const [latencyData, setLatencyData] = useState<LatencyDataPoint[]>([]);
  const [gapIndicators, setGapIndicators] = useState<{ time: string; count: number }[]>([]);

  // M-792: In-flight guard for health polling to prevent overlapping requests
  const healthFetchInFlightRef = useRef(false);
  const healthStaleTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // M-999: Client-side apply lag metrics for UI display
  // Shows pending count, avg latency, max latency without requiring devtools
  // M-1018: Extended with windowed metrics (60s window) to surface recent spikes
  const [applyLagInfo, setApplyLagInfo] = useState<{
    pendingCount: number;
    avgLatencyMs: number;       // Lifetime average (for reference)
    maxLatencyMs: number;       // Lifetime max (for reference)
    totalApplied: number;
    windowedAvgMs: number;      // M-1018: 60s window average (primary display)
    windowedMaxMs: number;      // M-1018: 60s window max
    windowedCount: number;      // M-1018: Samples in window
  } | null>(null);

  // Derived metrics
  const [errorRate, setErrorRate] = useState<number | null>(null);
  const [messagesPerSecond, setMessagesPerSecond] = useState<number | null>(null);
  // V-12/V-13: Metric history for sparklines + trend indicators
  const [metricHistory, setMetricHistory] = useState<Record<string, number[]>>({});

  // Tab state
  const [activeTab, setActiveTab] = useState<'overview' | 'events' | 'metrics' | 'graph'>('overview');

  // Graph visualization state - demo/fallback values (viewModel is primary source)
  const [graphSchema, setGraphSchema] = useState<GraphSchema | null>(null);
  const [nodeExecutions, setNodeExecutions] = useState<Record<string, NodeExecution>>({});
  const [selectedNode, setSelectedNode] = useState<string | undefined>();
  const [graphState, setGraphState] = useState<Record<string, unknown>>({});

  // Live execution tracking
  const [executionStartTime, setExecutionStartTime] = useState<number | null>(null);
  const [elapsedTime, setElapsedTime] = useState<string>('00:00.0');
  const [isGraphLive, setIsGraphLive] = useState(false);
  const lastGraphEventTimeRef = useRef<number>(0);

  // Timeline tracking
  const [timelineEvents, setTimelineEvents] = useState<TimelineEvent[]>([]);
  // Time-travel: selected timeline event index for graph state animation
  const [timelineSelectedIndex, setTimelineSelectedIndex] = useState<number | undefined>(undefined);

  // View toggle for Graph tab (Canvas vs Mermaid vs History)
  const [graphViewMode, setGraphViewMode] = useState<'canvas' | 'mermaid' | 'history'>('canvas');

  // M-388: Grouping controls for graph viewer (Canvas + Mermaid exports)
  const [groupingMode, setGroupingMode] = useState<GroupingMode>('none');
  const [groupingAttributeKey, setGroupingAttributeKey] = useState('group');

  // Schema observations for history tracking
  const [schemaObservations, setSchemaObservations] = useState<Map<string, SchemaObservation>>(new Map());

  // M-440: Track demo mode for clear visual indication
  const [isDemoMode, setIsDemoMode] = useState(false);

  // TYP-09: Export dropdown state
  const [showExportDropdown, setShowExportDropdown] = useState(false);

  // Expected schema pin for mismatch detection
  // Now persisted server-side via /api/expected-schema
  // M-113: Support per-graph baselines via graph_name in API path
  const [expectedSchemaId, setExpectedSchemaId] = useState<string | undefined>();
  const [expectedSchemaGraphName, setExpectedSchemaGraphName] = useState<string | undefined>();
  const [_expectedSchemaLoading, setExpectedSchemaLoading] = useState(true);

  // M-113: Helper to build expected-schema API endpoint with graph name
  // Falls back to "default" when no graph name is available
  const getExpectedSchemaEndpoint = useCallback((graphName?: string): string => {
    // Sanitize graph name for URL path (remove spaces, special chars)
    const safeName = graphName?.replace(/[^a-zA-Z0-9_-]/g, '_') || 'default';
    return `/api/expected-schema/${encodeURIComponent(safeName)}`;
  }, []);

  // Load expected schema from server on mount (uses "default" initially)
  useEffect(() => {
    const loadExpectedSchema = async () => {
      try {
        const response = await fetch(getExpectedSchemaEndpoint('default'));
        if (response.ok) {
          const data = await response.json();
          setExpectedSchemaId(data.schema_id);
          setExpectedSchemaGraphName('default');
        }
        // If 404, no expected schema is set - that's fine
      } catch (err) {
        debugWarn('[App] Failed to load expected schema from server:', err);
        // Fallback to localStorage for backwards compatibility
        try {
          const stored = window.localStorage.getItem('dashflow.expectedSchemaId');
          if (stored) setExpectedSchemaId(stored);
        } catch {
          // ignore
        }
      } finally {
        setExpectedSchemaLoading(false);
      }
    };
    loadExpectedSchema();
  }, [getExpectedSchemaEndpoint]);

  // M-123: Parse URL params for runtime config overrides (memoized since URL doesn't change)
  // Example: ?maxRuns=100&maxEvents=50K&maxCheckpointSize=20M
  const urlConfig = useMemo(() => parseConfigFromUrl(), []);

  // Run state store for time-travel debugging
  const {
    processMessage: processRunStateMessage,
    getRuns,
    getRunsSorted,
    getRunStore,
    getStateAt,
    cursor: runCursor,
    setCursor: setRunCursor,
    setLiveMode: setRunLiveMode,
    isLive: isRunLive,
    getViewModel,
    // M-115/M-116: Diagnostics for quarantined messages and corruption
    getQuarantined,
    clearQuarantine,
    // M-450: Hash verification version for UI re-renders on corruption detection
    hashVerificationVersion,
    // M-711: Mark runs needing resync due to gap/stale cursor
    markActiveRunsNeedResync,
    // M-744: Clear all runs for cursor_reset recovery
    clearAllRuns,
  } = useRunStateStore(urlConfig);

  // Get current view model for MermaidView
  // M-450: Include hashVerificationVersion to re-render when async corruption detection completes
  const viewModel = useMemo(() => getViewModel(), [getViewModel, runCursor, isRunLive, hashVerificationVersion]);

  // Get list of runs for TimelineSlider
  const runs = useMemo(() => getRuns(), [getRuns, runCursor]);

  // M-113: Reload expected schema when graph name changes
  // This ensures we use per-graph baselines when switching between graphs
  const currentGraphName = viewModel?.schema?.name;
  useEffect(() => {
    if (!currentGraphName) return;

    // Skip if we already have the expected schema for this graph
    if (expectedSchemaGraphName === currentGraphName) return;

    const loadGraphExpectedSchema = async () => {
      try {
        const endpoint = getExpectedSchemaEndpoint(currentGraphName);
        const response = await fetch(endpoint);
        if (response.ok) {
          const data = await response.json();
          setExpectedSchemaId(data.schema_id);
          setExpectedSchemaGraphName(currentGraphName);
          debugLog('[App] Loaded expected schema for graph:', currentGraphName);
        } else if (response.status === 404) {
          // No expected schema for this graph - clear it if we had one from a different graph
          if (expectedSchemaGraphName && expectedSchemaGraphName !== currentGraphName) {
            debugLog('[App] No expected schema for graph:', currentGraphName);
            // Don't clear - keep showing the previous expected schema until user sets a new one
          }
        }
      } catch (err) {
        debugWarn('[App] Failed to load expected schema for graph:', currentGraphName, err);
      }
    };

    loadGraphExpectedSchema();
  }, [currentGraphName, expectedSchemaGraphName, getExpectedSchemaEndpoint]);

  // Consolidated graph state pipeline - useRunStateStore is now the single source
  // Schema observation tracking and live status are handled via useEffects watching viewModel
  const processRunStateMessageRef = useRef(processRunStateMessage);
  useEffect(() => {
    processRunStateMessageRef.current = processRunStateMessage;
  }, [processRunStateMessage]);

  // Track schema observations when viewModel updates
  // M-457: Limit map size to prevent unbounded growth
  // M-759: Limit threadIds per schema to prevent unbounded memory
  const MAX_SCHEMA_OBSERVATIONS = 100;
  const MAX_THREADS_PER_SCHEMA = 50;
  useEffect(() => {
    if (!viewModel?.schemaId || !viewModel.schema) return;

    const schemaId = viewModel.schemaId;
    const threadId = viewModel.cursor.threadId;
    const now = Date.now();

    setSchemaObservations(prev => {
      const updated = new Map(prev);
      const existing = updated.get(schemaId);
      if (existing) {
        existing.lastSeen = now;
        if (!existing.threadIds.includes(threadId)) {
          existing.threadIds.push(threadId);
          existing.runCount++;
          // M-759: Evict oldest threadIds when over limit (FIFO)
          if (existing.threadIds.length > MAX_THREADS_PER_SCHEMA) {
            existing.threadIds = existing.threadIds.slice(-MAX_THREADS_PER_SCHEMA);
          }
        }
      } else {
        // Get graph name from run store if available
        const store = getRunStore(threadId);
        updated.set(schemaId, {
          schemaId,
          graphName: store?.graphName || viewModel.schema?.name || 'unknown',
          schema: viewModel.schema!,
          firstSeen: now,
          lastSeen: now,
          runCount: 1,
          threadIds: [threadId],
        });

        // M-457: Evict oldest entries when over limit
        if (updated.size > MAX_SCHEMA_OBSERVATIONS) {
          // Sort by lastSeen and remove oldest
          const entries = Array.from(updated.entries());
          entries.sort((a, b) => a[1].lastSeen - b[1].lastSeen);
          const toRemove = entries.slice(0, updated.size - MAX_SCHEMA_OBSERVATIONS);
          for (const [key] of toRemove) {
            updated.delete(key);
          }
        }
      }
      return updated;
    });

    // Track live status
    lastGraphEventTimeRef.current = now;
    setIsGraphLive(true);
    if (!executionStartTime && viewModel.currentNode) {
      setExecutionStartTime(now);
    }
  }, [viewModel?.schemaId, viewModel?.cursor?.seq, getRunStore, executionStartTime]);

  const executionStartTimeRef = useRef<number | null>(executionStartTime);
  useEffect(() => {
    executionStartTimeRef.current = executionStartTime;
  }, [executionStartTime]);

  // Expected schema controls (server-side persistence)
  // M-113: Use graph name for per-graph baselines
  const handleSetExpectedSchema = useCallback(async () => {
    const currentSchemaId = viewModel?.schemaId;
    const currentGraphName = viewModel?.schema?.name;
    if (!currentSchemaId) return;

    // Optimistically update UI
    setExpectedSchemaId(currentSchemaId);
    setExpectedSchemaGraphName(currentGraphName);

    const endpoint = getExpectedSchemaEndpoint(currentGraphName);
    try {
      const response = await fetch(endpoint, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ schema_id: currentSchemaId }),
      });
      if (!response.ok) {
        logError('[App] Failed to save expected schema to server:', response.statusText);
        // Fallback to localStorage
        window.localStorage.setItem('dashflow.expectedSchemaId', currentSchemaId);
      }
    } catch (err) {
      logError('[App] Failed to save expected schema to server:', err);
      // Fallback to localStorage
      try {
        window.localStorage.setItem('dashflow.expectedSchemaId', currentSchemaId);
      } catch {
        // ignore
      }
    }
  }, [viewModel, getExpectedSchemaEndpoint]);

  const handleClearExpectedSchema = useCallback(async () => {
    const currentGraphName = viewModel?.schema?.name || expectedSchemaGraphName;
    setExpectedSchemaId(undefined);
    setExpectedSchemaGraphName(undefined);

    const endpoint = getExpectedSchemaEndpoint(currentGraphName);
    try {
      const response = await fetch(endpoint, {
        method: 'DELETE',
      });
      if (!response.ok && response.status !== 404) {
        logError('[App] Failed to clear expected schema on server:', response.statusText);
      }
    } catch (err) {
      logError('[App] Failed to clear expected schema on server:', err);
    }
    // Also clear localStorage for backwards compatibility
    try {
      window.localStorage.removeItem('dashflow.expectedSchemaId');
    } catch {
      // ignore
    }
  }, [viewModel, expectedSchemaGraphName, getExpectedSchemaEndpoint]);

  // TYP-09: Export graph schema in different formats
  const exportToJson = useCallback((): string => {
    const schema = viewModel?.schema || graphSchema;
    if (!schema) return '{}';
    return JSON.stringify(schema, null, 2);
  }, [viewModel, graphSchema]);

  const exportToDot = useCallback((): string => {
    const schema = viewModel?.schema || graphSchema;
    if (!schema) return 'digraph G {}';

    // Escape special characters for DOT format (quotes and backslashes)
    const escapeDot = (s: string): string => s.replace(/\\/g, '\\\\').replace(/"/g, '\\"');

    const lines: string[] = [
      `digraph "${escapeDot(schema.name)}" {`,
      '  rankdir=TB;',
      '  node [shape=box, style=rounded];',
      '',
    ];

    // Add nodes with their types as labels
    for (const node of schema.nodes) {
      const desc = node.description ? node.description.slice(0, 30) + (node.description.length > 30 ? '...' : '') : '';
      const label = desc
        ? `${escapeDot(node.name)}\\n[${node.node_type}]\\n${escapeDot(desc)}`
        : `${escapeDot(node.name)}\\n[${node.node_type}]`;
      lines.push(`  "${escapeDot(node.name)}" [label="${label}"];`);
    }

    lines.push('');

    // Add edges
    for (const edge of schema.edges) {
      const style = edge.edge_type === 'conditional' ? ', style=dashed'
        : edge.edge_type === 'parallel' ? ', style=bold'
        : '';
      const label = edge.label ? `, label="${escapeDot(edge.label)}"` : '';

      if (edge.conditional_targets && edge.conditional_targets.length > 0) {
        for (const target of edge.conditional_targets) {
          lines.push(`  "${escapeDot(edge.from)}" -> "${escapeDot(target)}"${style}${label};`);
        }
      } else {
        lines.push(`  "${escapeDot(edge.from)}" -> "${escapeDot(edge.to)}"${style}${label};`);
      }
    }

    lines.push('}');
    return lines.join('\n');
  }, [viewModel, graphSchema]);

  const exportToMermaid = useCallback((): string => {
    const schema = viewModel?.schema || graphSchema;
    if (!schema) return 'graph TD';

    // Sanitize node ID for Mermaid (alphanumeric + underscore only)
    const sanitizeId = (s: string): string => s.replace(/[^a-zA-Z0-9_]/g, '_');
    // Escape quotes in labels
    const escapeLabel = (s: string): string => s.replace(/"/g, '#quot;');

    const lines: string[] = [
      'graph TD',
      `  %% ${schema.name} v${schema.version}`,
      '',
    ];

    // Add nodes with their types
    for (const node of schema.nodes) {
      const nodeId = sanitizeId(node.name);
      const label = `${escapeLabel(node.name)}<br/>[${node.node_type}]`;
      lines.push(`  ${nodeId}["${label}"]`);
    }

    lines.push('');

    // Add edges
    for (const edge of schema.edges) {
      const fromId = sanitizeId(edge.from);
      const edgeLabel = edge.label ? escapeLabel(edge.label) : '';
      const arrow = edge.edge_type === 'conditional' ? `-.->|${edgeLabel}|`
        : edge.edge_type === 'parallel' ? '==>'
        : '-->';

      if (edge.conditional_targets && edge.conditional_targets.length > 0) {
        for (const target of edge.conditional_targets) {
          lines.push(`  ${fromId} ${arrow} ${sanitizeId(target)}`);
        }
      } else {
        lines.push(`  ${fromId} ${arrow} ${sanitizeId(edge.to)}`);
      }
    }

    return lines.join('\n');
  }, [viewModel, graphSchema]);

  const handleExport = useCallback((format: 'json' | 'dot' | 'mermaid') => {
    const schema = viewModel?.schema || graphSchema;
    const graphName = schema?.name || 'graph';

    let content: string;
    let filename: string;
    let mimeType: string;

    switch (format) {
      case 'json':
        content = exportToJson();
        filename = `${graphName}.json`;
        mimeType = 'application/json';
        break;
      case 'dot':
        content = exportToDot();
        filename = `${graphName}.dot`;
        mimeType = 'text/plain';
        break;
      case 'mermaid':
        content = exportToMermaid();
        filename = `${graphName}.mermaid`;
        mimeType = 'text/plain';
        break;
    }

    // Create and trigger download
    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    setShowExportDropdown(false);
  }, [viewModel, graphSchema, exportToJson, exportToDot, exportToMermaid]);

  // Derive current graph view inputs from time-travel store when available
  const nodeExecutionsFromViewModel = useMemo((): Record<string, NodeExecution> | null => {
    if (!viewModel?.nodeStates) return null;
    const executions: Record<string, NodeExecution> = {};
    for (const [nodeName, nodeState] of viewModel.nodeStates.entries()) {
      executions[nodeName] = {
        node_name: nodeName,
        status: nodeState.status,
        start_time: nodeState.startTime,
        end_time: nodeState.endTime,
        duration_ms: nodeState.durationMs,
        error: nodeState.error,
      };
    }
    return executions;
  }, [viewModel]);

  // Time-travel: compute node executions from timeline events up to selected index
  // This enables scrubbing through the timeline to see graph state at each point
  const nodeExecutionsFromTimeline = useMemo((): Record<string, NodeExecution> | null => {
    if (timelineSelectedIndex === undefined || timelineEvents.length === 0) return null;

    const executions: Record<string, NodeExecution> = {};
    const startTimes: Record<string, number> = {};

    // Process events up to and including the selected index
    for (let i = 0; i <= timelineSelectedIndex && i < timelineEvents.length; i++) {
      const event = timelineEvents[i];
      const nodeId = event.node_id;
      if (!nodeId) continue;

      if (event.event_type === 'NodeStart') {
        startTimes[nodeId] = event.timestamp;
        executions[nodeId] = {
          node_name: nodeId,
          status: 'active',
          start_time: event.timestamp,
        };
      } else if (event.event_type === 'NodeEnd') {
        const startTime = startTimes[nodeId];
        executions[nodeId] = {
          node_name: nodeId,
          status: 'completed',
          start_time: startTime,
          end_time: event.timestamp,
          duration_ms: startTime ? event.timestamp - startTime : undefined,
        };
      } else if (event.event_type === 'NodeError') {
        executions[nodeId] = {
          ...executions[nodeId],
          node_name: nodeId,
          status: 'error',
          error: event.details,
        };
      }
    }

    return executions;
  }, [timelineSelectedIndex, timelineEvents]);

  // Unified state access - prefer useRunStateStore (viewModel) when available
  // Time-travel scrubbing (nodeExecutionsFromTimeline) takes highest priority
  // This pattern consolidates the two pipelines by preferring the authoritative source
  const effectiveSchema = viewModel?.schema || graphSchema;
  const effectiveNodeExecutions = nodeExecutionsFromTimeline || nodeExecutionsFromViewModel || nodeExecutions;
  const effectiveCurrentNode = viewModel?.currentNode;
  const effectiveGraphState = viewModel?.state || graphState;

  const previousGraphState = useMemo(() => {
    if (!runCursor) return {};
    const store = getRunStore(runCursor.threadId);
    if (!store || store.events.length === 0) return {};

    const idx = store.events.findIndex(e => e.seq === runCursor.seq);
    if (idx <= 0) return {};

    const prevSeq = store.events[idx - 1].seq;
    return getStateAt(runCursor.threadId, prevSeq);
  }, [runCursor, getRunStore, getStateAt]);

  // Compute node-specific state at node completion time
  // Shows state as it was when the selected node finished, not global state
  // Also works in live mode by finding the most recent run
  const selectedNodeState = useMemo(() => {
    if (!selectedNode) return null;

    // Determine which thread/run to use
    let threadId: string | undefined = runCursor?.threadId;

    // If no cursor (live mode), try to find the thread from recent runs
    if (!threadId) {
      // M-753: Use getRunsSorted() which returns runs sorted by startTime descending (most recent first)
      // getRuns() returns unsorted Map keys, which may not be chronological
      const allRunsSorted = getRunsSorted();
      if (allRunsSorted.length === 0) return null;
      // First element is most recent
      threadId = allRunsSorted[0].threadId;
    }

    if (!threadId) return null;

    const store = getRunStore(threadId);
    if (!store) return null;

    const nodeState = store.nodeStates.get(selectedNode);
    if (!nodeState?.endSeq) {
      // Node hasn't finished yet or no endSeq tracked, use current state
      return null;
    }

    return getStateAt(threadId, nodeState.endSeq);
  }, [selectedNode, runCursor, getRunStore, getStateAt, getRunsSorted]);

  // Compute previous state (before node started) for diff highlighting
  // Also works in live mode
  const selectedNodePreviousState = useMemo(() => {
    if (!selectedNode) return null;

    // Determine which thread/run to use (same logic as selectedNodeState)
    let threadId: string | undefined = runCursor?.threadId;
    if (!threadId) {
      // M-753: Use getRunsSorted() which returns runs sorted by startTime descending (most recent first)
      const allRunsSorted = getRunsSorted();
      if (allRunsSorted.length === 0) return null;
      // First element is most recent
      threadId = allRunsSorted[0].threadId;
    }

    if (!threadId) return null;

    const store = getRunStore(threadId);
    if (!store) return null;

    const nodeState = store.nodeStates.get(selectedNode);
    if (!nodeState?.startSeq) return null;

    // Get state just before the node started (startSeq - 1)
    // M-693: startSeq is now string, use BigInt arithmetic
    const startSeqBigInt = BigInt(nodeState.startSeq);
    if (startSeqBigInt <= BigInt(0)) return {};
    const prevSeq = (startSeqBigInt - BigInt(1)).toString();

    return getStateAt(threadId, prevSeq);
  }, [selectedNode, runCursor, getRunStore, getStateAt, getRunsSorted]);

  // Update elapsed time counter every 100ms when live
  useEffect(() => {
    if (!isGraphLive || !executionStartTime) return;

    const interval = setInterval(() => {
      const elapsed = Date.now() - executionStartTime;
      const seconds = Math.floor(elapsed / 1000);
      const tenths = Math.floor((elapsed % 1000) / 100);
      const minutes = Math.floor(seconds / 60);
      const secs = seconds % 60;
      setElapsedTime(`${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}.${tenths}`);

      // Check if still live (no events in 2 seconds)
      if (Date.now() - lastGraphEventTimeRef.current > 2000) {
        setIsGraphLive(false);
      }
    }, 100);

    return () => clearInterval(interval);
  }, [isGraphLive, executionStartTime]);

  // M-998: Initialize Web Worker pool for decode/decompress
  // M-2488: Don't terminate singleton in React cleanup - singleton should persist across
  // component lifecycle. React Strict Mode unmounts/remounts, and terminating the singleton
  // during cleanup causes the second mount to fail. The worker pool is a singleton that
  // should live for the lifetime of the page, not the component.
  useEffect(() => {
    const workerPool = getDecodeWorkerPool();
    workerPoolRef.current = workerPool;
    const initPromise = workerPool.init();
    workerPoolInitPromiseRef.current = initPromise;
    initPromise.then(() => {
      setDecoderReady(true);
      debugLog('[App] Decode worker pool initialized');
    }).catch((error) => {
      logError('[App] Failed to initialize decode worker pool:', error);
    });

    // M-2488: No cleanup - singleton worker pool should persist for page lifetime
    // Terminating in cleanup breaks React Strict Mode (mount/unmount/remount)
  }, []);

  // Initialize demo graph data as fallback (used when no live data is available)
  useEffect(() => {
    // P0 BUG FIX: Check multiple indicators of real data
    // viewModel may be null even when real data flows through WebSocket events
    // The 'events' array (raw DashStreamEvent[]) is only populated by real WebSocket data,
    // never by demo data setup, making it a reliable discriminator
    // Also check timelineEvents which is populated from binary protobuf decode path
    const hasRealData = viewModel || (events && events.length > 0) || (timelineEvents && timelineEvents.length > 0);

    if (hasRealData) {
      // M-440: Exit demo mode when live data becomes available
      if (isDemoMode) setIsDemoMode(false);
      return;
    }

    // M-440: Enter demo mode - set flag for clear visual indication
    setIsDemoMode(true);

    // Demo graph schema - ReAct coding agent (like codex-dashflow)
    const demoSchema: GraphSchema = {
      name: 'codex-agent (demo)',
      version: '1.0.0',
      description: 'Demo graph - ReAct coding agent with tools',
      entry_point: 'agent',
      nodes: [
        {
          name: 'agent',
          description: 'ReAct agent that decides which tool to use based on the task',
          node_type: 'llm',
          input_fields: ['messages', 'task'],
          output_fields: ['tool_call', 'response'],
          attributes: { source: 'src/agent/mod.rs:90' }
        },
        {
          name: 'read_file',
          description: 'Reads file contents from the filesystem',
          node_type: 'tool',
          input_fields: ['path'],
          output_fields: ['content', 'line_count'],
          attributes: { source: 'src/agent/tools.rs:45' }
        },
        {
          name: 'write_file',
          description: 'Writes or creates files on the filesystem',
          node_type: 'tool',
          input_fields: ['path', 'content'],
          output_fields: ['success', 'bytes_written'],
          attributes: { source: 'src/agent/tools.rs:120' }
        },
        {
          name: 'edit_file',
          description: 'Makes targeted edits to existing files',
          node_type: 'tool',
          input_fields: ['path', 'old_text', 'new_text'],
          output_fields: ['success', 'changes'],
          attributes: { source: 'src/agent/tools.rs:180' }
        },
        {
          name: 'shell_exec',
          description: 'Executes shell commands (cargo, git, npm)',
          node_type: 'tool',
          input_fields: ['command', 'timeout_secs'],
          output_fields: ['stdout', 'stderr', 'exit_code'],
          attributes: { source: 'src/agent/tools.rs:250' }
        },
        {
          name: 'list_files',
          description: 'Lists directory contents recursively',
          node_type: 'tool',
          input_fields: ['path', 'recursive'],
          output_fields: ['files', 'count'],
          attributes: { source: 'src/agent/tools.rs:310' }
        },
        {
          name: 'validator',
          description: 'Validates agent output and checks for errors',
          node_type: 'validator',
          input_fields: ['response'],
          output_fields: ['is_valid', 'errors'],
          attributes: {}
        },
        {
          name: 'router',
          description: 'Routes to appropriate tool based on agent decision',
          node_type: 'router',
          input_fields: ['tool_call'],
          output_fields: ['next_node'],
          attributes: {}
        }
      ],
      edges: [
        { from: 'agent', to: 'router', edge_type: 'direct' },
        { from: 'router', to: 'read_file', edge_type: 'conditional', label: 'read_file' },
        { from: 'router', to: 'write_file', edge_type: 'conditional', label: 'write_file' },
        { from: 'router', to: 'edit_file', edge_type: 'conditional', label: 'edit_file' },
        { from: 'router', to: 'shell_exec', edge_type: 'conditional', label: 'shell_exec' },
        { from: 'router', to: 'list_files', edge_type: 'conditional', label: 'list_files' },
        { from: 'router', to: 'validator', edge_type: 'conditional', label: 'done' },
        { from: 'read_file', to: 'agent', edge_type: 'direct' },
        { from: 'write_file', to: 'agent', edge_type: 'direct' },
        { from: 'edit_file', to: 'agent', edge_type: 'direct' },
        { from: 'shell_exec', to: 'agent', edge_type: 'direct' },
        { from: 'list_files', to: 'agent', edge_type: 'direct' }
      ],
      metadata: {}
    };

    setGraphSchema(demoSchema);

    // Demo execution state - simulate a codex agent run with multiple tool calls
    setNodeExecutions({
      'agent': { node_name: 'agent', status: 'completed', start_time: Date.now() - 8000, end_time: Date.now() - 7500, duration_ms: 500 },
      'router': { node_name: 'router', status: 'completed', start_time: Date.now() - 7500, end_time: Date.now() - 7400, duration_ms: 100 },
      'list_files': { node_name: 'list_files', status: 'completed', start_time: Date.now() - 7400, end_time: Date.now() - 7100, duration_ms: 300 },
      'read_file': { node_name: 'read_file', status: 'completed', start_time: Date.now() - 6000, end_time: Date.now() - 5700, duration_ms: 300 },
      'edit_file': { node_name: 'edit_file', status: 'completed', start_time: Date.now() - 4500, end_time: Date.now() - 4200, duration_ms: 300 },
      'shell_exec': { node_name: 'shell_exec', status: 'completed', start_time: Date.now() - 3000, end_time: Date.now() - 1500, duration_ms: 1500 },
      'write_file': { node_name: 'write_file', status: 'pending', start_time: undefined, end_time: undefined, duration_ms: undefined },
      'validator': { node_name: 'validator', status: 'completed', start_time: Date.now() - 500, end_time: Date.now() - 200, duration_ms: 300 }
    });

    // Demo state - coding task
    setGraphState({
      task: 'Fix the bug in src/parser.rs',
      messages: [
        { role: 'user', content: 'Fix the bug in src/parser.rs' },
        { role: 'assistant', content: 'I\'ll examine the parser file and fix the issue.' }
      ],
      tool_results: {
        list_files: { files: ['src/main.rs', 'src/parser.rs', 'src/lib.rs'], count: 3 },
        read_file: { content: 'fn parse(input: &str) -> Result<()> { ... }', line_count: 42 },
        edit_file: { success: true, changes: 1 },
        shell_exec: { stdout: 'test result: ok. 15 passed; 0 failed', exit_code: 0 }
      },
      status: 'completed'
    });

    // Demo timeline events - codex agent execution with tool loop
    const demoStartTime = Date.now() - 8000;
    setTimelineEvents([
      { timestamp: demoStartTime, elapsed_ms: 0, event_type: 'GraphStart', details: 'codex-agent started' },
      { timestamp: demoStartTime + 100, elapsed_ms: 100, event_type: 'NodeStart', node_id: 'agent' },
      { timestamp: demoStartTime + 500, elapsed_ms: 500, event_type: 'NodeEnd', node_id: 'agent', details: '400ms' },
      { timestamp: demoStartTime + 600, elapsed_ms: 600, event_type: 'NodeStart', node_id: 'router' },
      { timestamp: demoStartTime + 700, elapsed_ms: 700, event_type: 'NodeEnd', node_id: 'router', details: '100ms → list_files' },
      { timestamp: demoStartTime + 800, elapsed_ms: 800, event_type: 'NodeStart', node_id: 'list_files' },
      { timestamp: demoStartTime + 1100, elapsed_ms: 1100, event_type: 'NodeEnd', node_id: 'list_files', details: '300ms' },
      { timestamp: demoStartTime + 1200, elapsed_ms: 1200, event_type: 'NodeStart', node_id: 'agent' },
      { timestamp: demoStartTime + 1700, elapsed_ms: 1700, event_type: 'NodeEnd', node_id: 'agent', details: '500ms' },
      { timestamp: demoStartTime + 1800, elapsed_ms: 1800, event_type: 'NodeStart', node_id: 'router' },
      { timestamp: demoStartTime + 1900, elapsed_ms: 1900, event_type: 'NodeEnd', node_id: 'router', details: '100ms → read_file' },
      { timestamp: demoStartTime + 2000, elapsed_ms: 2000, event_type: 'NodeStart', node_id: 'read_file' },
      { timestamp: demoStartTime + 2300, elapsed_ms: 2300, event_type: 'NodeEnd', node_id: 'read_file', details: '300ms' },
      { timestamp: demoStartTime + 2400, elapsed_ms: 2400, event_type: 'NodeStart', node_id: 'agent' },
      { timestamp: demoStartTime + 2900, elapsed_ms: 2900, event_type: 'NodeEnd', node_id: 'agent', details: '500ms' },
      { timestamp: demoStartTime + 3000, elapsed_ms: 3000, event_type: 'NodeStart', node_id: 'router' },
      { timestamp: demoStartTime + 3100, elapsed_ms: 3100, event_type: 'NodeEnd', node_id: 'router', details: '100ms → edit_file' },
      { timestamp: demoStartTime + 3200, elapsed_ms: 3200, event_type: 'NodeStart', node_id: 'edit_file' },
      { timestamp: demoStartTime + 3500, elapsed_ms: 3500, event_type: 'NodeEnd', node_id: 'edit_file', details: '300ms' },
      { timestamp: demoStartTime + 3600, elapsed_ms: 3600, event_type: 'NodeStart', node_id: 'agent' },
      { timestamp: demoStartTime + 4100, elapsed_ms: 4100, event_type: 'NodeEnd', node_id: 'agent', details: '500ms' },
      { timestamp: demoStartTime + 4200, elapsed_ms: 4200, event_type: 'NodeStart', node_id: 'router' },
      { timestamp: demoStartTime + 4300, elapsed_ms: 4300, event_type: 'NodeEnd', node_id: 'router', details: '100ms → shell_exec' },
      { timestamp: demoStartTime + 4400, elapsed_ms: 4400, event_type: 'NodeStart', node_id: 'shell_exec' },
      { timestamp: demoStartTime + 5900, elapsed_ms: 5900, event_type: 'NodeEnd', node_id: 'shell_exec', details: '1500ms (cargo test)' },
      { timestamp: demoStartTime + 6000, elapsed_ms: 6000, event_type: 'NodeStart', node_id: 'agent' },
      { timestamp: demoStartTime + 6500, elapsed_ms: 6500, event_type: 'NodeEnd', node_id: 'agent', details: '500ms' },
      { timestamp: demoStartTime + 6600, elapsed_ms: 6600, event_type: 'NodeStart', node_id: 'router' },
      { timestamp: demoStartTime + 6700, elapsed_ms: 6700, event_type: 'NodeEnd', node_id: 'router', details: '100ms → done' },
      { timestamp: demoStartTime + 6800, elapsed_ms: 6800, event_type: 'NodeStart', node_id: 'validator' },
      { timestamp: demoStartTime + 7100, elapsed_ms: 7100, event_type: 'NodeEnd', node_id: 'validator', details: '300ms' },
      { timestamp: demoStartTime + 7200, elapsed_ms: 7200, event_type: 'GraphEnd', details: 'completed in 7.2s' },
    ]);
    setExecutionStartTime(demoStartTime);
    // P0 BUG FIX: Added events.length and timelineEvents.length to detect real WebSocket data
    // timelineEvents is populated from binary protobuf path (the primary data flow)
  }, [viewModel, events.length, timelineEvents.length, isDemoMode]);

  // Fetch health data periodically
  // M-792: Added timeout and in-flight guard to prevent overlapping requests and hangs
  const fetchHealth = useCallback(async () => {
    // M-792: Skip if previous request is still in flight
    if (healthFetchInFlightRef.current) {
      debugLog('[App] Skipping health fetch - previous request still in flight');
      return;
    }
    healthFetchInFlightRef.current = true;

    // M-792: Use AbortController for timeout (4s, less than 5s poll interval)
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 4000);

    try {
      const response = await fetch('/health', { signal: controller.signal });
      // M-795: Check response.ok before parsing JSON
      if (!response.ok) {
        const errorMsg = `HTTP ${response.status}`;
        console.warn(`[App] Health check failed with status ${response.status}`);
        setHealthStale(true);
        setHealthError(errorMsg);
        setErrorRate(null);
        setMessagesPerSecond(null);
        return;
      }
      const data: HealthResponse = await response.json();
      setHealth(data);
      setHealthLastOkAt(Date.now());
      setHealthError(null);
      setHealthStale(false);
      setErrorRate(null);

      if (healthStaleTimeoutRef.current) {
        clearTimeout(healthStaleTimeoutRef.current);
      }
      healthStaleTimeoutRef.current = setTimeout(() => {
        setHealthStale(true);
        setErrorRate(null);
        setMessagesPerSecond(null);
      }, HEALTH_STALE_MS);

      // Calculate error rate
      if (data.metrics.kafka_messages_received > 0) {
        const rate = (data.metrics.kafka_errors / data.metrics.kafka_messages_received) * 100;
        setErrorRate(Math.round(rate * 100) / 100);
      }

      // Update throughput data
      // M-789/M-1066: Include monotonic timestamp for accurate rate calculation
      // Use performance.now() which is monotonic (unaffected by NTP clock adjustments)
      // T-06: Use consistent 24h timestamp format
      const now = formatTimestamp(new Date());
      const tMs = performance.now();
      setThroughputData(prev => {
        const newData = [...prev, {
          time: now,
          messages: data.metrics.kafka_messages_received,
          errors: data.metrics.kafka_errors,
          tMs
        }];
        return newData.slice(-METRIC_SPARKLINE_POINTS); // V-12: Keep last 60 data points for sparklines
      });

      // V-12: Record key metrics for card sparklines (best-effort; skip missing values)
      setMetricHistory(prev => {
        const next = { ...prev };
        const append = (label: string, value: number | null | undefined) => {
          if (value == null || !Number.isFinite(value)) return;
          const current = next[label] ?? [];
          next[label] = [...current, value].slice(-METRIC_SPARKLINE_POINTS);
        };

        append('Dropped (2m)', data.metrics.dropped_messages_last_120s ?? 0);
        append('Decode Errors (2m)', data.metrics.decode_errors_last_120s ?? 0);
        append('Send Failures', (data.metrics.send_failed ?? 0) + (data.metrics.send_timeout ?? 0));
        append('Replay Buffer', data.replay_buffer?.memory_buffer_size);
        return next;
      });
    } catch (e) {
      // M-792: Distinguish timeout/abort from other errors
      if (e instanceof Error && e.name === 'AbortError') {
        console.warn('[App] Health check timed out after 4s');
        setHealthStale(true);
        setHealthError('timeout');
      } else {
        logError('Failed to fetch health:', e);
        setHealthStale(true);
        setHealthError(e instanceof Error ? e.message : String(e));
      }
      setErrorRate(null);
      setMessagesPerSecond(null);
    } finally {
      clearTimeout(timeoutId);
      healthFetchInFlightRef.current = false;
    }
  }, []);

  // V-12: Track client-side apply lag metrics for sparklines.
  useEffect(() => {
    if (!applyLagInfo) return;
    setMetricHistory(prev => {
      const next = { ...prev };
      const append = (label: string, value: number | null | undefined) => {
        if (value == null || !Number.isFinite(value)) return;
        const current = next[label] ?? [];
        next[label] = [...current, value].slice(-METRIC_SPARKLINE_POINTS);
      };
      append('Apply Lag (60s)', applyLagInfo.windowedAvgMs);
      append('Apply Queue', applyLagInfo.pendingCount);
      return next;
    });
  }, [applyLagInfo]);

  // Fetch version info
  // M-795: Check response.ok before parsing JSON to avoid throwing on error bodies
  const fetchVersion = useCallback(async () => {
    try {
      const response = await fetch('/version');
      if (!response.ok) {
        console.warn(`[App] Version fetch failed with status ${response.status}`);
        return;
      }
      const data: VersionInfo = await response.json();
      setVersion(data);
      setResumeNamespace(data.resume_namespace ?? null);

      // M-1019: Check for config drift between server and UI payload limits.
      // If server accepts larger payloads than UI can decode, warn operators.
      if (data.max_payload_bytes !== undefined && data.max_payload_bytes > MAX_DECOMPRESSED_SIZE) {
        const warning = `Config drift: Server max payload (${(data.max_payload_bytes / 1024 / 1024).toFixed(1)}MB) > UI limit (${(MAX_DECOMPRESSED_SIZE / 1024 / 1024).toFixed(1)}MB). Large messages may fail to decode.`;
        console.warn(`[App] ${warning}`);
        setConfigDriftWarning(warning);
      } else {
        setConfigDriftWarning(null);
      }

      // M-1020: Log decode error policy for operator awareness
      if (data.decode_error_policy) {
        console.info(`[App] Server decode_error_policy: ${data.decode_error_policy}`);
      }
    } catch (e) {
      logError('Failed to fetch version:', e);
    } finally {
      setVersionLoaded(true);
    }
  }, []);

  // WebSocket connection
  useEffect(() => {
    if (!versionLoaded) return;

    // M-691: Namespace persisted resume cursors to prevent cross-topic/cluster collisions.
    const offsetsStorageKey = resumeNamespace
      ? `${DASHSTREAM_LAST_OFFSETS_STORAGE_KEY}:${resumeNamespace}`
      : DASHSTREAM_LAST_OFFSETS_STORAGE_KEY;

    // M-674: Restore last known Kafka partition offsets so resume can catch up runs started while UI was offline.
    // M-707: Offsets are stored as strings to preserve precision for values > 2^53
    // M-763: Validate that restored strings are numeric to prevent BigInt() throws
    const isNumericString = (s: string): boolean => /^\d+$/.test(s);
    try {
      let raw = localStorage.getItem(offsetsStorageKey);
      // Backwards-compat: If we're now namespaced but only the legacy key exists, migrate it.
      if (!raw && resumeNamespace) {
        raw = localStorage.getItem(DASHSTREAM_LAST_OFFSETS_STORAGE_KEY);
        if (raw) {
          try {
            localStorage.setItem(offsetsStorageKey, raw);
          } catch {
            // ignore
          }
        }
      }
      if (raw) {
        const parsed = JSON.parse(raw) as unknown;
        if (parsed && typeof parsed === 'object') {
          // M-707: Convert legacy number offsets to strings for precision
          const offsets: Record<string, string> = {};
          for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
            if (typeof value === 'string') {
              // M-763: Validate numeric format before accepting
              if (isNumericString(value)) {
                offsets[key] = value;
              } else {
                console.warn(`[App] Dropping invalid offset for partition ${key}: "${value}" is not a valid numeric string`);
              }
            } else if (typeof value === 'number') {
              // Legacy format: convert number to string
              offsets[key] = value.toString();
            }
          }
          lastOffsetsByPartitionRef.current = offsets;
        }
      }
    } catch {
      // Ignore corrupt localStorage state; resume will fall back to per-thread cursors.
    }

    // M-681: Restore per-thread sequence cursors for fallback replay
    // M-763: Validate restored sequences are numeric strings
    const sequencesStorageKey = resumeNamespace
      ? `${DASHSTREAM_LAST_SEQUENCES_STORAGE_KEY}:${resumeNamespace}`
      : DASHSTREAM_LAST_SEQUENCES_STORAGE_KEY;
    try {
      let raw = localStorage.getItem(sequencesStorageKey);
      // Backwards-compat: migrate legacy key if exists
      if (!raw && resumeNamespace) {
        raw = localStorage.getItem(DASHSTREAM_LAST_SEQUENCES_STORAGE_KEY);
        if (raw) {
          try {
            localStorage.setItem(sequencesStorageKey, raw);
          } catch {
            // ignore
          }
        }
      }
      if (raw) {
        const parsed = JSON.parse(raw) as unknown;
        if (parsed && typeof parsed === 'object') {
          const sequences: Record<string, string> = {};
          for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
            if (typeof value === 'string') {
              // M-763: Validate numeric format before accepting
              if (isNumericString(value)) {
                sequences[key] = value;
              } else {
                console.warn(`[App] Dropping invalid sequence for thread ${key}: "${value}" is not a valid numeric string`);
              }
            } else if (typeof value === 'number') {
              sequences[key] = value.toString();
            }
          }
          lastSequencesByThreadRef.current = sequences;
        }
      }
    } catch {
      // Ignore corrupt localStorage; sequences will start fresh
    }

    const connectWebSocket = () => {
      // Use getWebSocketUrl() for correct protocol (ws:// or wss://)
      const ws = new WebSocket(getWebSocketUrl());
      wsRef.current = ws;
      wsEpochRef.current += 1;
      const wsEpoch = wsEpochRef.current;
      pendingKafkaCursorRef.current = null;
      binaryProcessingChainRef.current = Promise.resolve();
      wsProtocolErrorRef.current = false;
      wsBinaryProcessingTimeoutRef.current = false;
      fallingBehindTriggeredRef.current = false; // M-1007: Reset falling behind state on reconnect
      // M-1005: Reset schema mismatch state on reconnect. If the server is rolled back
      // to a compatible schema version, the UI should resume normal operation.
      schemaVersionMismatchWarnedRef.current = false;
      schemaVersionMismatchActiveRef.current = false;
      setSchemaVersionMismatchInfo(null);

      ws.onopen = () => {
        debugLog('WebSocket connected');
        setConnected(true);
        setWsError(null);
        // M-459: Reset retry count on successful connection
        wsRetryCountRef.current = 0;
        // C-01: Clear reconnection feedback on successful connect
        setWsRetryAttempt(null);
        // M-734/M-758: Reset apply lag metrics on reconnect to prevent stale averages
        // M-1018: Also reset windowed metrics buffer
        applyLagMetricsRef.current = {
          pendingCount: 0,
          totalApplied: 0,
          totalLatencyMs: 0,
          lastReportTime: Date.now(),
          maxLatencyMs: 0,
          windowMs: 60000,
          recentSamples: [],
        };

        // M-674 + M-676: Always send resume with partition mode enabled.
        // Even if we have no offsets, sending empty `lastOffsetsByPartition` tells the server
        // to use partition mode and discover all known partitions for first-connect clients.
        const lastOffsetsByPartition = lastOffsetsByPartitionRef.current;
        const lastSequencesByThread = lastSequencesByThreadRef.current;
        const hasSequences = Object.keys(lastSequencesByThread).length > 0;

	        // Legacy compatibility: older websocket-server expects scalar lastSequence.
	        // This is best-effort and not globally correct, but helps during rolling upgrades.
	        // M-693: sequences are strings; convert to BigInt for comparison, Number for legacy output
	        // M-763: Wrap BigInt in try/catch as defensive fallback (validation should prevent bad strings)
	        let maxSeqStr = '0';
	        let lastSequence: number | undefined;
	        if (hasSequences) {
	          try {
	            maxSeqStr = Object.values(lastSequencesByThread).reduce(
	              (max, v) => {
                try {
                  return BigInt(v) > BigInt(max) ? v : max;
                } catch {
                  console.warn(`[App] Invalid sequence value "${v}" skipped during max computation`);
                  return max;
                }
              },
	              '0'
	            );
	            const maxSeqBigInt = BigInt(maxSeqStr);

	            // M-805: Never send an unsafe numeric legacy lastSequence (precision loss).
	            const maxSafe = BigInt(Number.MAX_SAFE_INTEGER);
	            if (maxSeqBigInt > maxSafe) {
	              console.warn(
	                `[App] Legacy lastSequence ${maxSeqStr} exceeds MAX_SAFE_INTEGER (${Number.MAX_SAFE_INTEGER}). ` +
	                `Omitting legacy lastSequence to avoid incorrect resume on old servers. ` +
	                `Server should use lastSequencesByThread (string) for accurate replay.`
	              );
	              lastSequence = undefined;
	            } else {
	              lastSequence = Number(maxSeqBigInt);
	            }
	          } catch (e) {
	            // M-763: On any BigInt failure, fall back to '0'/0 and clear invalid sequences
	            console.warn('[App] BigInt conversion failed during resume, falling back to 0:', e);
	            maxSeqStr = '0';
	            lastSequence = undefined;
	          }
	        }

        // M-703: Determine resume strategy based on whether we have stored offsets.
        // - "latest": first connect, start from current position (no historical replay)
        // - "cursor": subsequent connects, resume from last known position
        const isFirstConnect = Object.keys(lastOffsetsByPartition).length === 0 && !hasSequences;
        const resumeFrom = isFirstConnect ? 'latest' : 'cursor';

	        ws.send(JSON.stringify({
	          type: 'resume',
	          // M-676: Always send lastOffsetsByPartition to enable partition discovery mode
	          lastOffsetsByPartition,
	          // M-703: Explicit resume strategy
	          from: resumeFrom,
	          ...(hasSequences
	            ? {
	                lastSequencesByThread,
	                ...(lastSequence !== undefined ? { lastSequence } : {}),
	              }
	            : {}),
	        }));
	      };

      ws.onclose = () => {
        debugLog('WebSocket disconnected');
        setConnected(false);
        pendingKafkaCursorRef.current = null;
        // M-459: Exponential backoff with jitter (1s, 2s, 4s, 8s... up to 30s)
        // Jitter helps prevent thundering herd when server restarts
        const baseDelay = 1000;
        const maxDelay = 30000;
        const maxRetries = 10; // Give up after ~2 minutes of retries
        const attempt = wsRetryCountRef.current;

        if (attempt >= maxRetries) {
          logError(`WebSocket: Max retries (${maxRetries}) reached. Stopping reconnection.`);
          setWsError('Connection failed after multiple retries. Refresh page to retry.');
          return;
        }

        const exponentialDelay = Math.min(baseDelay * Math.pow(2, attempt), maxDelay);
        const jitter = Math.random() * exponentialDelay * 0.3; // ±30% jitter
        const delay = Math.round(exponentialDelay + jitter);

        wsRetryCountRef.current = attempt + 1;
        debugLog(`WebSocket reconnecting in ${delay}ms (attempt ${attempt + 1}/${maxRetries})`);
        // C-01: Update reconnection feedback state for UI display
        setWsRetryAttempt({ attempt: attempt + 1, maxRetries });

        // M-451: Store reconnect timeout in ref so it can be canceled on cleanup
        reconnectTimeoutRef.current = setTimeout(connectWebSocket, delay);
      };

      ws.onerror = () => {
        logError('WebSocket error');
        setWsError('WebSocket connection failed');
        setConnected(false);
      };

      ws.onmessage = (event) => {
        try {
          // Ignore any straggler messages from prior WebSocket instances/epochs.
          if (wsEpoch !== wsEpochRef.current) return;

          // Handle text messages (JSON control messages)
          if (typeof event.data === 'string') {
            const data = JSON.parse(event.data);

            if (data.type === 'gap') {
              // Gap indicator - messages were missed
              // M-711: Gap signals potential data loss - mark runs as needing resync
              const gapCount = data.count ?? 1;
              setGapIndicators(prev => [...prev, {
                time: formatTimestamp(new Date()), // T-06: consistent 24h format
                count: gapCount
              }].slice(-10));
              // M-711: Mark active runs as corrupted/needing resync
              markActiveRunsNeedResync(
                `Message gap detected (${gapCount} messages missed). ` +
                `State may be incomplete until a full snapshot/checkpoint arrives.`
              );
            } else if (data.type === 'pong') {
              // Health check response
            } else if (data.type === 'cursor') {
              // M-674: Cursor metadata so we can resume by Kafka partition+offset.
              //
              // IMPORTANT: Do not persist offsets on receipt of the cursor frame. The UI must only
              // "commit" a cursor after the corresponding binary message has been successfully
              // decoded and applied; otherwise a reload mid-replay can permanently skip messages.
              const partition = data.partition;
              // M-690: Server sends offset as string to avoid JS precision loss for values > 2^53
              // M-707: Keep offset as string throughout to preserve precision
              const offsetRaw = data.offset;
              const offset: string | null =
                typeof offsetRaw === 'string' ? offsetRaw :
                typeof offsetRaw === 'number' ? offsetRaw.toString() : null;
              // Validate: partition is number, offset is valid non-negative string
              if (typeof partition === 'number' && offset !== null && /^\d+$/.test(offset)) {
                if (pendingKafkaCursorRef.current) {
                  // M-723: A cursor must pair 1:1 with the next binary message.
                  // Receiving a second cursor while one is pending would desync pairing and can
                  // corrupt persisted resume offsets. Abort and reconnect.
                  if (!wsProtocolErrorRef.current) {
                    wsProtocolErrorRef.current = true;
                    debugWarn('[App] Protocol error: received cursor while previous cursor still pending; forcing reconnect');
                    markActiveRunsNeedResync(
                      'Protocol error: cursor/binary pairing broke (cursor arrived while previous cursor still pending). ' +
                      'Reconnecting to prevent offset corruption.'
                    );
                    // Abort in-flight binary processing for this epoch before closing.
                    wsEpochRef.current += 1;
                    try {
                      wsRef.current?.close(1002, 'cursor_pairing_desync');
                    } catch {
                      // ignore
                    }
                  }
                  return;
                }
                pendingKafkaCursorRef.current = { partition, offset };
              }
            } else if (data.type === 'replay_complete') {
              // M-676: Server confirms replay is complete.
              //
              // M-686: This handler is INTENTIONALLY informational-only and does not verify
              // catch-up completeness. The design rationale:
              // 1. Messages are processed as they arrive (cursor commit on each binary message)
              // 2. Gap detection happens during replay via gap/cursor_stale messages (M-711)
              // 3. Corruption flags are set if state is incomplete (needsResync)
              // 4. The UI already tracks its own progress via cursor commits
              //
              // replay_complete is useful for:
              // - Debugging (logging how many messages were replayed)
              // - Detecting replay truncation (capped field) for user warnings
              // - Operators monitoring replay performance
              //
              // It is NOT used for: state consistency verification (that's handled elsewhere)
              const totalReplayed = data.totalReplayed ?? 0;
              const capped = data.capped ?? false; // M-692: Server signals if replay was truncated
              if (capped) {
                // M-692: Warn user that replay was truncated - state may be incomplete
                console.warn(`[App] ⚠️ Replay was capped at ${totalReplayed} messages (safety limit). Earlier state may be missing.`);
                debugLog(`Replay CAPPED: ${totalReplayed} messages (not all history available)`);
              } else {
                debugLog(`Replay complete: ${totalReplayed} messages replayed`);
              }
            } else if (data.type === 'cursor_stale') {
              // M-679: Server detected that client's cursor is older than retained data
              // This means some historical data was likely evicted (TTL expiration)
              // M-711: This is a resync trigger - state cannot be trusted
              const partition = data.partition ?? '?';
              const requested = data.requested ?? '?';
              const oldest = data.oldest ?? '?';
              console.warn(
                `[App] ⚠️ Stale cursor for partition ${partition}: ` +
                `requested offset ${requested}, but oldest retained is ${oldest}. ` +
                `Some historical state may be missing.`
              );
              debugLog(`Cursor STALE: partition=${partition} requested=${requested} oldest=${oldest}`);
              // M-711: Mark active runs as corrupted/needing resync
              markActiveRunsNeedResync(
                `Stale cursor for partition ${partition} (requested: ${requested}, oldest available: ${oldest}). ` +
                `Messages were evicted before replay could catch up. State may be incomplete.`
              );
            } else if (data.type === 'disconnect') {
              // M-682: Server disconnecting us due to backpressure (slow client)
              // This is expected when the client cannot keep up with message throughput.
              // The client should reconnect using the resume protocol to catch up.
              const reason = data.reason ?? 'unknown';
              const cumulativeLag = data.cumulative_lag ?? 0;
              const threshold = data.threshold ?? 0;
              console.warn(
                `[App] 🛑 Server disconnecting client: reason=${reason}, ` +
                `cumulative_lag=${cumulativeLag}, threshold=${threshold}. ` +
                `${data.message ?? 'Will attempt auto-reconnect.'}`
              );
              debugLog(`DISCONNECT: reason=${reason} lag=${cumulativeLag}/${threshold}`);
              // Mark runs as needing resync since messages were dropped
              markActiveRunsNeedResync(
                `Server disconnected slow client (${cumulativeLag} messages dropped). ` +
                `Reconnection will use resume protocol to recover missed data.`
              );
              // Note: The WebSocket close event will trigger auto-reconnect via existing logic
            } else if (data.type === 'cursor_reset_complete') {
              // M-706: Server confirms cursor reset and provides latest offsets
              // Client can use these offsets for a clean resume
              const offsets = data.latestOffsetsByPartition ?? {};
              const partitionCount = Object.keys(offsets).length;
              const bufferCleared = data.bufferCleared ?? false;
              console.info(
                `[App] Cursor reset complete: ${partitionCount} partition(s) available` +
                `${bufferCleared ? ', server buffer cleared' : ''}. ` +
                `${data.message ?? ''}`
              );
              debugLog(`CURSOR_RESET: ${partitionCount} partitions with latest offsets`);

              // M-744: Clear all UI run state for a clean slate recovery
              // This ensures corrupt/stale state doesn't persist after a reset
              clearAllRuns();

              // M-744: Clear all prior offsets and set to server's latest
              // This ensures we don't mix old offsets with new ones
              lastOffsetsByPartitionRef.current = {};
              for (const [partition, offset] of Object.entries(offsets)) {
                if (typeof offset === 'string' && /^\d+$/.test(offset)) {
                  lastOffsetsByPartitionRef.current[partition] = offset;
                }
              }

              // Clear per-thread sequences too (now invalid after reset)
              lastSequencesByThreadRef.current = {};

              // Persist to localStorage
              try {
                localStorage.setItem(
                  offsetsStorageKey,
                  JSON.stringify(lastOffsetsByPartitionRef.current)
                );
                // Also clear thread sequences from localStorage
                localStorage.removeItem(sequencesStorageKey);
              } catch {
                // Ignore localStorage failures
              }
            } else {
              // Legacy JSON event
              // M-468: Only slice when length exceeds threshold to avoid unnecessary array allocation
              setEvents(prev => {
                const newEvents = [data, ...prev];
                return newEvents.length > 100 ? newEvents.slice(0, 100) : newEvents;
              });
            }
          } else if (event.data instanceof Blob || event.data instanceof ArrayBuffer) {
            // Binary protobuf data - decode with protobufjs
            // M-779: Handle both Blob and ArrayBuffer frames. WebSocket binaryType can be
            // "blob" (default) or "arraybuffer", and some servers may send either format.
            const blobOrBuffer = event.data;

            // M-707: offset is string to preserve precision for values > 2^53
            const commitKafkaCursor = (cursor: { partition: number; offset: string }) => {
              const { partition, offset } = cursor;
              // Validate offset is a non-negative numeric string
              if (!/^\d+$/.test(offset)) return;
              const key = String(partition);
              const prev = lastOffsetsByPartitionRef.current[key];
              // M-707: Use BigInt for comparison to handle values > 2^53
              if (prev !== undefined) {
                const prevBig = BigInt(prev);
                const currBig = BigInt(offset);
                // M-785: Backward offsets indicate data loss (topic recreation, compaction, corruption).
                // Unlike forward gaps (which might just be timing), backward offsets mean messages
                // we thought we processed are now gone or different. Mark runs as needing resync
                // to acknowledge potential state inconsistency.
                if (currBig < prevBig) {
                  console.warn(
                    `[App] Backward Kafka offset detected for partition ${partition}: ` +
                    `${prev} → ${offset}. This may indicate topic recreation or data loss. ` +
                    `Marking active runs as needing resync.`
                  );
                  markActiveRunsNeedResync(
                    `Backward Kafka offset on partition ${partition} (${prev} → ${offset}). ` +
                    'This indicates potential data loss. Consider cursor_reset if state is inconsistent.'
                  );
                }
              }
              // Always update to the new offset (forward or backward)
              if (prev === undefined || BigInt(offset) !== BigInt(prev)) {
                lastOffsetsByPartitionRef.current[key] = offset;
                // M-1058: Track update timestamp for true LRU eviction
                offsetUpdatedAtRef.current[key] = Date.now();
              }

              const now = Date.now();
              if (now - lastOffsetsPersistedAtRef.current > 1000) {
                lastOffsetsPersistedAtRef.current = now;
                try {
                  // M-678: Apply eviction to prevent unbounded localStorage growth
                  // M-727: Protect partition "0" from eviction (often most important in Kafka)
                  // M-1058: Pass update timestamps for true LRU eviction
                  const toStore = evictOldestEntries(
                    lastOffsetsByPartitionRef.current,
                    MAX_STORED_PARTITIONS,
                    (a, b) => {
                      const aBig = BigInt(a);
                      const bBig = BigInt(b);
                      return aBig < bBig ? -1 : aBig > bBig ? 1 : 0;
                    },
                    ['0'], // Protect partition 0 from eviction
                    offsetUpdatedAtRef.current // M-1058: timestamp map
                  );
                  localStorage.setItem(
                    offsetsStorageKey,
                    JSON.stringify(toStore)
                  );
                } catch {
                  // Ignore localStorage failures; resume will still work within the tab session.
                }
              }
            };

            // M-998: Use Web Worker pool for decode/decompress
            const ensureWorkerPoolReady = async (): Promise<DecodeWorkerPool | null> => {
              const current = workerPoolRef.current;
              if (current && current.isInitialized()) return current;

              let promise = workerPoolInitPromiseRef.current;
              if (!promise) {
                const pool = getDecodeWorkerPool();
                workerPoolRef.current = pool;
                promise = pool.init();
                workerPoolInitPromiseRef.current = promise;
              }

              try {
                await promise;
                return workerPoolRef.current;
              } catch (e) {
                logError('[App] Failed to initialize decode worker pool (during message):', e);
                return null;
              }
            };

            // M-674 follow-up: Binary decode/apply must be serialized and cursor commits must be
            // tied to successful decode+apply (not cursor receipt).
            const cursor = pendingKafkaCursorRef.current;
            pendingKafkaCursorRef.current = null;
            if (!cursor) {
              // M-720: Applying a binary message without its cursor can permanently corrupt resume:
              // later offsets may be committed past this message, making it unreplayable after reload/crash.
              // Abort and reconnect so the server can replay from the last committed cursor.
              if (!wsProtocolErrorRef.current) {
                wsProtocolErrorRef.current = true;
                debugWarn('[App] Protocol error: binary message arrived without a pending cursor; forcing reconnect');
                markActiveRunsNeedResync(
                  'Protocol error: received a binary message without a paired cursor. ' +
                  'Reconnecting to avoid corrupting persisted resume offsets.'
                );
                wsEpochRef.current += 1;
                try {
                  wsRef.current?.close(1002, 'missing_cursor');
                } catch {
                  // ignore
                }
              }
              return;
            }

            // M-1007: Check if we are falling behind. If pendingCount exceeds the cap,
            // force reconnect to prevent unbounded memory growth. The server will replay
            // from the last committed offset after reconnect.
            const currentPending = applyLagMetricsRef.current.pendingCount;
            if (currentPending >= MAX_PENDING_BINARY_MESSAGES && !fallingBehindTriggeredRef.current) {
              fallingBehindTriggeredRef.current = true;
              console.warn(
                `[App] Falling behind: ${currentPending} messages pending (max ${MAX_PENDING_BINARY_MESSAGES}). ` +
                'Forcing reconnect to prevent unbounded memory growth.'
              );
              markActiveRunsNeedResync(
                `Client falling behind: ${currentPending} messages queued for processing. ` +
                'Reconnecting to resume from last committed offset.'
              );
              wsEpochRef.current += 1;
              try {
                wsRef.current?.close(1002, 'falling_behind');
              } catch {
                // ignore
              }
              return;
            }

            // M-705: Track message receipt time for apply lag metrics
            const receiptTime = performance.now();
            // M-1017: Capture metrics object reference at increment time. This ensures that
            // all decrement operations in the async chain operate on the SAME metrics object
            // that received the increment. Without this, a reconnect during processing would
            // replace applyLagMetricsRef.current and cause in-flight tasks to decrement the
            // NEW object (which never saw the increment), producing negative pendingCount
            // and unreliable backlog cap behavior.
            const metricsForThisBatch = applyLagMetricsRef.current;
            metricsForThisBatch.pendingCount++;

            binaryProcessingChainRef.current = binaryProcessingChainRef.current
              .then(async () => {
                if (wsEpoch !== wsEpochRef.current) {
                  metricsForThisBatch.pendingCount--;
                  return;
                }

                // M-998: Use Web Worker pool for decode/decompress
                const workerPool = await withTimeout(
                  ensureWorkerPoolReady(),
                  BINARY_PROCESSING_STEP_TIMEOUT_MS,
                  'Decode worker pool initialization'
                );
                if (!workerPool || !workerPool.isInitialized()) {
                  // M-739: Decrement pendingCount on worker init failure to prevent leak
                  // M-1017: Use captured metrics object to match the increment
                  metricsForThisBatch.pendingCount--;
                  return;
                }

                // M-779: Handle both Blob and ArrayBuffer - skip conversion if already ArrayBuffer
                const buffer = blobOrBuffer instanceof ArrayBuffer
                  ? blobOrBuffer
                  : await withTimeout(
                      blobOrBuffer.arrayBuffer(),
                      BINARY_PROCESSING_STEP_TIMEOUT_MS,
                      'Binary message blob.arrayBuffer()'
                    );

                // M-998: Decode in Web Worker to prevent main thread freezes.
                // Worker handles decompression + protobuf decode off the main thread.
                // If decode times out, the worker is terminated and recreated.
                const decoded = await workerPool.decode(buffer, BINARY_PROCESSING_STEP_TIMEOUT_MS);
                if (!decoded) {
                  // M-975: Decode failure is a protocol/data error. We must NOT continue streaming
                  // because later cursors could be committed past this unapplied message, causing
                  // a permanent skip after reload/crash. Force reconnect to replay from last committed cursor.
                  // M-739: Decrement pendingCount on decode failure to prevent leak
                  // M-1017: Use captured metrics object to match the increment
                  metricsForThisBatch.pendingCount--;
                  if (!wsProtocolErrorRef.current) {
                    wsProtocolErrorRef.current = true;
                    debugWarn('[App] Protobuf decode returned null; forcing reconnect to prevent cursor skip');
                    markActiveRunsNeedResync(
                      'Protocol error: binary message failed to decode. ' +
                      'Reconnecting to avoid committing cursors past an unapplied message.'
                    );
                    wsEpochRef.current += 1;
                    try {
                      wsRef.current?.close(1002, 'decode_failure');
                    } catch {
                      // ignore
                    }
                  }
                  return;
                }

                // M-976: Check for schema version mismatch and warn/gate cursor commits.
                // If the message's schemaVersion > EXPECTED_SCHEMA_VERSION, the UI may misinterpret fields.
                if (decoded.schemaVersionMismatch) {
                  const isFirstMismatch = !schemaVersionMismatchActiveRef.current;
                  schemaVersionMismatchActiveRef.current = true;
                  if (isFirstMismatch && typeof decoded.schemaVersion === 'number') {
                    setSchemaVersionMismatchInfo({
                      messageSchemaVersion: decoded.schemaVersion,
                      expectedSchemaVersion: EXPECTED_SCHEMA_VERSION,
                    });
                  }
                  if (!schemaVersionMismatchWarnedRef.current) {
                    schemaVersionMismatchWarnedRef.current = true;
                    console.warn(
                      `[App] Schema version mismatch: message has v${decoded.schemaVersion}, ` +
                      `UI expects v${EXPECTED_SCHEMA_VERSION}. ` +
                      'The UI may misinterpret fields. Update the UI (run proto:gen) to match the server. ' +
                      'Cursor commits are gated until this is resolved.'
                    );
                  }
                }

                // M-997: Under schema mismatch, do not apply decoded messages. The UI can remain
                // "connected" but must not mutate graph state or timelines with incompatible payloads.
                if (schemaVersionMismatchActiveRef.current) {
                  // M-1017: Use captured metrics object to match the increment
                  metricsForThisBatch.pendingCount--;
                  return;
                }

                // M-1031: Re-check epoch immediately before state mutation. The async decode work
                // above (workerPool init, arrayBuffer, decode) can take significant time. If the
                // websocket epoch changed during that work, this message is from a stale connection
                // and must NOT mutate state or commit cursors - doing so would contaminate the new
                // epoch's state and/or advance persisted cursors past messages that were never
                // applied in the current epoch.
                if (wsEpoch !== wsEpochRef.current) {
                  metricsForThisBatch.pendingCount--;
                  console.debug(
                    `[App] Epoch changed during decode (${wsEpoch} → ${wsEpochRef.current}); ` +
                    'discarding stale message to prevent cross-epoch state contamination'
                  );
                  return;
                }

                // Single state pipeline - useRunStateStore is the authoritative source
                // M-685: Attach Kafka cursor to decoded message so EventBatch inner events can inherit it
                decoded.partition = cursor.partition;
                decoded.offset = cursor.offset;
                processRunStateMessageRef.current(decoded);

                // Create event for the events list
                const now = Date.now();
                let eventTypeName: string = decoded.type;
                let nodeId: string | undefined;

	                // Extract more details from event messages
	                // Also extract schema_id for out-of-schema highlighting
	                let eventSchemaId: string | undefined;
	                if (decoded.type === 'event' && decoded.message.event) {
                  const eventType = decoded.message.event.eventType as EventType;
                  eventTypeName = getEventTypeName(eventType);
	                  nodeId = decoded.message.event.nodeId || undefined;
	                  // Extract schema_id from event attributes if present
	                  const attrs = decoded.message.event.attributes as Record<string, unknown> | undefined;
	                  if (attrs) {
	                    eventSchemaId = getStringAttribute(attrs, 'schema_id');
	                  }
	                }

                // M-974: Treat timestamp=0 as valid and reject NaN/non-finite timestamps.
                const decodedTimestamp =
                  typeof decoded.timestamp === 'number' && Number.isFinite(decoded.timestamp)
                    ? decoded.timestamp
                    : undefined;

                const dashEvent: DashStreamEvent = {
                  timestamp: decodedTimestamp ?? now,
                  type: eventTypeName,
                  thread_id: decoded.threadId,
                  sequence: decoded.sequence
                };
                setEvents(prev => {
                  const newEvents = [dashEvent, ...prev];
                  return newEvents.length > 100 ? newEvents.slice(0, 100) : newEvents;
                });

                // M-693: sequences are strings; use BigInt for comparison
                // M-802: Accept seq=0 and validate before BigInt()
                // M-982: EventBatch can contain multiple threadIds; persist max sequence per thread.
                let sequencesUpdated = false;
                const sequencesToConsider: Record<string, string> | undefined =
                  decoded.type === 'event_batch' && decoded.sequencesByThread
                    ? decoded.sequencesByThread
                    : decoded.threadId && decoded.sequence
                      ? { [decoded.threadId]: decoded.sequence }
                      : undefined;

                if (sequencesToConsider) {
                  for (const [threadId, seqStr] of Object.entries(sequencesToConsider)) {
                    if (!threadId || !seqStr || !isNumericString(seqStr)) continue;
                    const seqBig = BigInt(seqStr);
                    const prev = lastSequencesByThreadRef.current[threadId];
                    const prevBig = prev && isNumericString(prev) ? BigInt(prev) : null;
                    if (prevBig === null || seqBig > prevBig) {
                      lastSequencesByThreadRef.current[threadId] = seqStr;
                      // M-1058: Track update timestamp for true LRU eviction
                      seqUpdatedAtRef.current[threadId] = Date.now();
                      sequencesUpdated = true;
                    }
                  }
                }

                if (sequencesUpdated) {
                  // M-681: Persist per-thread sequences to localStorage (throttled)
                  const now2 = Date.now();
                  if (now2 - lastSequencesPersistedAtRef.current > 1000) {
                    lastSequencesPersistedAtRef.current = now2;
                    try {
                      // M-678: Apply eviction to prevent unbounded localStorage growth
                      // M-1058: Pass update timestamps for true LRU eviction
                      const toStore = evictOldestEntries(
                        lastSequencesByThreadRef.current,
                        MAX_STORED_THREADS,
                        (a, b) => {
                          const aBig = BigInt(a);
                          const bBig = BigInt(b);
                          return aBig < bBig ? -1 : aBig > bBig ? 1 : 0;
                        },
                        [], // No protected keys for threads
                        seqUpdatedAtRef.current // M-1058: timestamp map
                      );
                      localStorage.setItem(
                        sequencesStorageKey,
                        JSON.stringify(toStore)
                      );
                    } catch {
                      // Ignore localStorage failures
                    }
                  }
                }

                if (decodedTimestamp !== undefined) {
                  const latency = now - decodedTimestamp;
                  const timeStr = formatTimestamp(new Date()); // T-06: consistent 24h format
                  // M-791: Don't clamp negative latency to 0. Negative values indicate
                  // clock skew (producer's clock is ahead of consumer's). Exposing this
                  // helps operators diagnose NTP/clock sync issues rather than hiding them.
                  setLatencyData(prev => [...prev, {
                    time: timeStr,
                    latency: latency
                  }].slice(-30));
                }

                setTimelineEvents(prev => {
                  const startTimeRef = executionStartTimeRef.current || now;
                  const elapsed_ms = now - startTimeRef;

                  const timelineEvent: TimelineEvent = {
                    // M-974: Treat timestamp=0 as valid and reject NaN/non-finite timestamps.
                    timestamp: decodedTimestamp ?? now,
                    elapsed_ms,
                    event_type: eventTypeName,
                    node_id: nodeId,
                    schema_id: eventSchemaId,
                  };

                  const newTimeline = [...prev, timelineEvent];
                  return newTimeline.length > 100 ? newTimeline.slice(-100) : newTimeline;
                });

                // M-976: Gate cursor commits when schema version mismatch is active.
                // This prevents advancing past messages that may have been misinterpreted.
                // M-1031: Also re-check epoch immediately before cursor commit. Even though we
                // checked epoch before state mutation (above), we re-check here as a defense-in-depth
                // measure. If the epoch changed during the time between state apply and cursor commit
                // (unlikely but possible if React state updates yielded), we must not advance
                // persisted cursors for a stale epoch - that would cause the new epoch to skip
                // messages that were never applied in the current connection lifecycle.
                if (cursor && !schemaVersionMismatchActiveRef.current && wsEpoch === wsEpochRef.current) {
                  commitKafkaCursor(cursor);
                }

                // M-705: Record apply lag metrics
                // M-1017: Use captured metricsForThisBatch to avoid race where epoch changes
                // mid-processing and applyLagMetricsRef.current points to the new epoch's metrics
                const applyTime = performance.now();
                const latencyMs = applyTime - receiptTime;
                metricsForThisBatch.pendingCount--;
                metricsForThisBatch.totalApplied++;
                metricsForThisBatch.totalLatencyMs += latencyMs;
                if (latencyMs > metricsForThisBatch.maxLatencyMs) {
                  metricsForThisBatch.maxLatencyMs = latencyMs;
                }
                // M-1018/M-1066: Add sample to windowed buffer and prune old samples
                // Use performance.now() for monotonic timestamps (unaffected by clock adjustments)
                const sampleTimestamp = performance.now();
                metricsForThisBatch.recentSamples.push({ timestamp: sampleTimestamp, latencyMs });
                const windowCutoff = sampleTimestamp - metricsForThisBatch.windowMs;
                // Prune samples older than window (shift from front)
                while (metricsForThisBatch.recentSamples.length > 0 &&
                       metricsForThisBatch.recentSamples[0].timestamp < windowCutoff) {
                  metricsForThisBatch.recentSamples.shift();
                }

                // M-705: Periodic metrics reporting (every 10 seconds)
                // M-999: Also update UI state so apply lag is visible in health panel
                // M-1017: Only update UI if still in current epoch to avoid showing stale metrics
                // M-1030: Update even when totalApplied==0 to show pending count under wedge conditions
                const now3 = Date.now();
                if (now3 - metricsForThisBatch.lastReportTime > 10000) {
                  metricsForThisBatch.lastReportTime = now3;
                  // M-1030: Safe division - show 0 avg latency when no applies yet
                  const avgLatencyMs = metricsForThisBatch.totalApplied > 0
                    ? metricsForThisBatch.totalLatencyMs / metricsForThisBatch.totalApplied
                    : 0;
                  // M-1018: Calculate windowed metrics from recentSamples
                  const windowedSamples = metricsForThisBatch.recentSamples;
                  let windowedAvgMs = 0;
                  let windowedMaxMs = 0;
                  if (windowedSamples.length > 0) {
                    let windowSum = 0;
                    for (const s of windowedSamples) {
                      windowSum += s.latencyMs;
                      if (s.latencyMs > windowedMaxMs) {
                        windowedMaxMs = s.latencyMs;
                      }
                    }
                    windowedAvgMs = windowSum / windowedSamples.length;
                  }
                  // M-1018: Enhanced logging with both lifetime and windowed metrics
                  console.info(
                    `[ApplyLag] pending=${metricsForThisBatch.pendingCount} ` +
                    `applied=${metricsForThisBatch.totalApplied} ` +
                    `avgLatency=${avgLatencyMs.toFixed(1)}ms ` +
                    `maxLatency=${metricsForThisBatch.maxLatencyMs.toFixed(1)}ms ` +
                    `| 60s: avg=${windowedAvgMs.toFixed(1)}ms max=${windowedMaxMs.toFixed(1)}ms n=${windowedSamples.length}`
                  );
                  // M-999: Update UI state for health panel display
                  // M-1017: Only update if still in current epoch
                  // M-1018: Include windowed metrics for spike-sensitive display
                  if (wsEpoch === wsEpochRef.current) {
                    setApplyLagInfo({
                      pendingCount: metricsForThisBatch.pendingCount,
                      avgLatencyMs,
                      maxLatencyMs: metricsForThisBatch.maxLatencyMs,
                      totalApplied: metricsForThisBatch.totalApplied,
                      windowedAvgMs,
                      windowedMaxMs,
                      windowedCount: windowedSamples.length,
                    });
                  }
                }
              })
              .catch((err) => {
                // M-1016: ANY error in the decode/apply pipeline is fatal to resume correctness.
                // If we continue streaming after an error, later cursors can be committed past
                // the unapplied message, causing permanent message skips after reload/crash.
                // Force reconnect for ALL errors, not just timeouts.
                const isTimeout = isTimeoutError(err);
                const errorType = isTimeout ? 'timeout' : 'processing_error';
                const closeCode = isTimeout ? 1011 : 1002;

                logError(`[App] Binary processing ${errorType}:`, err);

                if (isTimeout) {
                  debugWarn('[App] Binary processing timeout; forcing reconnect');
                  markActiveRunsNeedResync(
                    'Binary processing timed out. Reconnecting to avoid wedging the decode/apply pipeline.'
                  );
                } else {
                  // M-1016: Non-timeout errors (worker exceptions, apply failures) are equally
                  // dangerous. The message was not applied but if we continue, later cursors
                  // could be committed and the message would be skipped permanently.
                  debugWarn('[App] Binary processing error (non-timeout); forcing reconnect to prevent cursor skip');
                  markActiveRunsNeedResync(
                    'Binary processing error. Reconnecting to avoid committing cursors past an unapplied message.'
                  );
                }

                // M-705: Decrement pending on error to keep count accurate
                // M-1017: Use captured metrics object to match the increment
                metricsForThisBatch.pendingCount--;

                // Force reconnect (only once per epoch to avoid redundant closes)
                if (!wsProtocolErrorRef.current) {
                  wsProtocolErrorRef.current = true;
                  wsEpochRef.current += 1;
                  try {
                    wsRef.current?.close(closeCode, errorType);
                  } catch {
                    // ignore
                  }
                }
              });
          }
        } catch (err) {
          logError('Failed to parse message:', err);
        }
      };

      return ws;
    };

    connectWebSocket();

    // M-451: Cleanup must close wsRef.current, not the captured ws variable.
    // When WebSocket reconnects on disconnect, it creates new instances stored in wsRef.current.
    // Closing only 'ws' would orphan reconnected instances (resource leak).
    // Also cancel any pending reconnect timeout to prevent zombie connections.
    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
        reconnectTimeoutRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [resumeNamespace, versionLoaded]);

  // M-745: Callback to send cursor_reset request to server
  // This allows users to trigger a clean reset when state is corrupted
  const sendCursorReset = useCallback(() => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
      console.warn('[App] Cannot send cursor_reset: WebSocket not connected');
      return false;
    }
    try {
      wsRef.current.send(JSON.stringify({ type: 'cursor_reset' }));
      console.info('[App] Sent cursor_reset request to server');
      return true;
    } catch (err) {
      console.error('[App] Failed to send cursor_reset:', err);
      return false;
    }
  }, []);

  // Periodic health checks
  useEffect(() => {
    fetchHealth();
    fetchVersion();
    const interval = setInterval(fetchHealth, 5000);
    return () => {
      clearInterval(interval);
      if (healthStaleTimeoutRef.current) {
        clearTimeout(healthStaleTimeoutRef.current);
        healthStaleTimeoutRef.current = null;
      }
    };
  }, [fetchHealth, fetchVersion]);

  // M-1030: Periodic apply-lag UI update (independent of message apply)
  // This ensures the health panel shows pending count even when:
  // - No messages have been applied yet (totalApplied==0)
  // - The decode/apply pipeline is wedged or slow
  // - Messages are accumulating faster than they're being processed
  // M-1018: Also prunes old samples and calculates windowed metrics
  useEffect(() => {
    const interval = setInterval(() => {
      const metrics = applyLagMetricsRef.current;
      // Only update if we have activity (pendingCount > 0 or totalApplied > 0)
      // This avoids showing stale "0 pending, 0 applied" when idle
      if (metrics.pendingCount > 0 || metrics.totalApplied > 0) {
        const avgLatencyMs = metrics.totalApplied > 0
          ? metrics.totalLatencyMs / metrics.totalApplied
          : 0;
        // M-1018: Prune old samples and calculate windowed metrics
        const now = Date.now();
        const windowCutoff = now - metrics.windowMs;
        while (metrics.recentSamples.length > 0 &&
               metrics.recentSamples[0].timestamp < windowCutoff) {
          metrics.recentSamples.shift();
        }
        let windowedAvgMs = 0;
        let windowedMaxMs = 0;
        if (metrics.recentSamples.length > 0) {
          let windowSum = 0;
          for (const s of metrics.recentSamples) {
            windowSum += s.latencyMs;
            if (s.latencyMs > windowedMaxMs) {
              windowedMaxMs = s.latencyMs;
            }
          }
          windowedAvgMs = windowSum / metrics.recentSamples.length;
        }
        setApplyLagInfo({
          pendingCount: metrics.pendingCount,
          avgLatencyMs,
          maxLatencyMs: metrics.maxLatencyMs,
          totalApplied: metrics.totalApplied,
          windowedAvgMs,
          windowedMaxMs,
          windowedCount: metrics.recentSamples.length,
        });
      }
    }, 5000); // Update every 5 seconds
    return () => clearInterval(interval);
  }, []);

  // Calculate messages per second
  // M-789: Use actual time delta instead of assuming fixed 5s intervals; handle counter resets
  useEffect(() => {
    if (healthStale) {
      setMessagesPerSecond(null);
      return;
    }
    if (throughputData.length >= 2) {
      const latest = throughputData[throughputData.length - 1];
      const previous = throughputData[throughputData.length - 2];
      const msgDiff = latest.messages - previous.messages;
      const dtMs = latest.tMs - previous.tMs;

      // M-789: Counter reset detection - if messages decreased, server likely restarted
      if (msgDiff < 0) {
        console.warn('[App] Counter reset detected (messages decreased), clearing throughput history');
        setThroughputData([latest]); // Keep only current point after reset
        setMessagesPerSecond(0);
        return;
      }

      // M-789: Guard against zero/negative time delta (shouldn't happen but be defensive)
      if (dtMs <= 0) {
        console.warn('[App] Invalid time delta for rate calculation:', dtMs);
        return;
      }

      // M-789: Compute rate using actual wall-clock time
      const dtSec = dtMs / 1000;
      setMessagesPerSecond(Math.max(0, msgDiff / dtSec));
    }
  }, [throughputData, healthStale]);

  // Render status badge
  // V-14: Enhanced status badge with pulse animation for warning/error states
  // T-04: Added tooltips to explain what each status means
  const STATUS_EXPLANATIONS: Record<string, string> = {
    healthy: 'System is operating normally with no issues detected',
    degraded: 'System is experiencing issues but still operational. Performance may be affected.',
    reconnecting: 'Attempting to restore connection to the server',
    waiting: 'Waiting for initial data from the server',
    will_restart_soon: 'Critical errors detected. System will automatically restart to recover.',
    stale: 'Data has not been updated recently. Server may be unresponsive.',
    unavailable: 'Cannot reach the server. Check network connection.',
  };

  const renderStatusBadge = (status: string) => {
    const color = STATUS_COLORS[status] || colors.connection.waiting;
    // V-14: Determine pulse class based on status severity
    const isWarning = status === 'degraded' || status === 'reconnecting';
    const isError = status === 'will_restart_soon' || status === 'unavailable';
    const pulseClass = isError ? 'status-badge-error' : isWarning ? 'status-badge-warning' : '';
    // T-04: Get tooltip explanation for status
    const explanation = STATUS_EXPLANATIONS[status] || `Current status: ${status}`;

    return (
      <span
        className={pulseClass}
        title={explanation}
        style={{
          display: 'inline-block',
          padding: '4px 12px',
          borderRadius: '16px',
          backgroundColor: color,
          color: colors.text.white,
          fontSize: '12px',
          fontWeight: 'bold',
          textTransform: 'uppercase',
          cursor: 'help',
        }}
      >
        {status.replace(/_/g, ' ')}
      </span>
    );
  };

  // Render circuit breaker status
  // C-05: State explanations for circuit breaker
  const CIRCUIT_BREAKER_EXPLANATIONS: Record<string, string> = {
    healthy: 'System is operating normally.',
    degraded: 'Repeated errors detected. Circuit breaker has limited retries to prevent cascading failures.',
    will_restart_soon: 'Too many errors accumulated. Server will restart automatically to recover.',
    half_open: 'Testing if the system has recovered. Limited requests are being allowed through.',
    open: 'Circuit breaker is open. All requests are being rejected to protect the system.',
  };

  const renderCircuitBreaker = () => {
    if (!health || healthStale || !health.circuit_breaker) return null;

    const cb = health.circuit_breaker;
    const isHealthy = cb.state === 'healthy';
    const willRestart = cb.state === 'will_restart_soon';
    // C-05: Get explanation for current state
    const explanation = CIRCUIT_BREAKER_EXPLANATIONS[cb.state] || `Current state: ${cb.state}`;

    return (
      <div style={{
        padding: '12px',
        borderRadius: '8px',
        backgroundColor: isHealthy ? colors.statusBg.successMaterial : willRestart ? colors.statusBg.errorMaterial : colors.statusBg.warningMaterial,
        border: `1px solid ${isHealthy ? colors.connection.healthy : willRestart ? colors.connection.unavailable : colors.connection.degraded}`
      }}>
        <div style={{ fontWeight: 'bold', marginBottom: '4px' }}>
          Circuit Breaker: {renderStatusBadge(cb.state)}
        </div>
        {/* C-05: Show explanation of what the current state means */}
        {!isHealthy && (
          <div style={{ fontSize: '12px', color: colors.text.light, marginBottom: '4px', fontStyle: 'italic' }}>
            {explanation}
          </div>
        )}
        {/* I-05: Handle undefined/null gracefully with fallback */}
        {cb.degraded_duration_seconds != null && (
          <div style={{ fontSize: '12px', color: colors.text.muted }}>
            Degraded for: {formatUptime(cb.degraded_duration_seconds)}
          </div>
        )}
        {cb.degraded_duration_seconds == null && cb.state !== 'healthy' && (
          <div style={{ fontSize: '12px', color: colors.text.faint }}>
            Degraded for: N/A
          </div>
        )}
        {/* B-05: Use formatUptime for consistent duration display */}
        {cb.time_until_restart_seconds !== undefined && cb.time_until_restart_seconds > 0 && (
          <div style={{ fontSize: '12px', color: colors.connection.unavailable }}>
            Auto-restart in: {formatUptime(cb.time_until_restart_seconds)}
          </div>
        )}
      </div>
    );
  };

  // I-04: Enhanced metric card with visual importance hierarchy
  // Critical metrics (error/warning colors) get a colored left border and slightly larger text
  // I-13: Added tooltip parameter for metric explanations
  // V-11: Icon mapping for metric cards - distinct icon for each metric type
  const METRIC_ICONS: Record<string, string> = {
    'Messages Received': '📨',
    'Messages/sec': '⚡',
    'Error Rate': '❌',
    'Dropped (2m)': '📉',
    'Decode Errors (2m)': '🔓',
    'Send Failures': '📤',
    'Uptime': '⏱️',
    'Apply Lag (60s)': '⏳',
    'Apply Queue': '📋',
    'Replay Buffer': '🔄',
  };

  // V-13: Define whether higher values indicate improvement for each metric.
  const METRIC_HIGHER_IS_BETTER: Partial<Record<string, boolean>> = {
    'Messages Received': true,
    'Messages/sec': true,
    'Error Rate': false,
    'Dropped (2m)': false,
    'Decode Errors (2m)': false,
    'Send Failures': false,
    'Uptime': true,
    'Apply Lag (60s)': false,
    'Apply Queue': false,
    // Replay Buffer is context-dependent; treat as neutral (no trend coloring).
  };

  // V-12/V-13: Derived series for sparklines + trends.
  const messagesReceivedSeries = useMemo(
    () => throughputData.map(p => p.messages).slice(-METRIC_SPARKLINE_POINTS),
    [throughputData]
  );

  const messagesPerSecondSeries = useMemo(() => {
    if (throughputData.length < 2) return [];
    const rates: number[] = [];
    for (let i = 1; i < throughputData.length; i++) {
      const latest = throughputData[i];
      const previous = throughputData[i - 1];
      const msgDiff = latest.messages - previous.messages;
      const dtMs = latest.tMs - previous.tMs;
      if (msgDiff < 0 || dtMs <= 0) continue;
      rates.push(msgDiff / (dtMs / 1000));
    }
    return rates.slice(-METRIC_SPARKLINE_POINTS);
  }, [throughputData]);

  const lifetimeErrorRateSeries = useMemo(
    () => throughputData
      .map(p => (p.messages > 0 ? (p.errors / p.messages) * 100 : 0))
      .slice(-METRIC_SPARKLINE_POINTS),
    [throughputData]
  );

  const computeTrend = (values: number[] | null | undefined, higherIsBetter: boolean | null) => {
    if (!values || values.length < 2) return null;
    const latest = values[values.length - 1];
    const previous = values[values.length - 2];
    if (!Number.isFinite(latest) || !Number.isFinite(previous)) return null;

    const delta = latest - previous;
    const absPrevious = Math.abs(previous);
    const pct = absPrevious > 0 ? (delta / absPrevious) * 100 : null;
    const stable =
      pct != null
        ? Math.abs(pct) < 0.5
        : Math.abs(delta) < 0.01;

    const direction = stable ? 'flat' : delta > 0 ? 'up' : 'down';
    const arrow = direction === 'up' ? '↑' : direction === 'down' ? '↓' : '→';

    // Replay Buffer and any other neutral metrics: show gray indicator only.
    if (higherIsBetter == null || direction === 'flat') {
      return {
        arrow,
        color: colors.text.muted,
        pctText: pct != null ? `${pct >= 0 ? '+' : ''}${pct.toFixed(0)}%` : null,
      };
    }

    const improving = (delta > 0 && higherIsBetter) || (delta < 0 && !higherIsBetter);
    const worsening = (delta > 0 && !higherIsBetter) || (delta < 0 && higherIsBetter);

    return {
      arrow,
      color: improving ? colors.connection.healthy : worsening ? colors.connection.unavailable : colors.text.muted,
      pctText: pct != null ? `${pct >= 0 ? '+' : ''}${pct.toFixed(0)}%` : null,
    };
  };

  const renderSparkline = (values: number[], stroke: string) => {
    if (values.length < 2) return null;

    const width = 120;
    const height = 18;
    const pad = 2;

    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (const v of values) {
      if (!Number.isFinite(v)) continue;
      if (v < min) min = v;
      if (v > max) max = v;
    }
    if (!Number.isFinite(min) || !Number.isFinite(max)) return null;

    const span = max - min;
    const innerW = width - pad * 2;
    const innerH = height - pad * 2;
    const stepX = innerW / (values.length - 1);

    const yFor = (v: number) => {
      if (span === 0) return pad + innerH * 0.5;
      const t = (v - min) / span;
      return pad + innerH * (1 - t);
    };

    let d = '';
    for (let i = 0; i < values.length; i++) {
      const x = pad + i * stepX;
      const y = yFor(values[i]);
      d += i === 0 ? `M ${x} ${y}` : ` L ${x} ${y}`;
    }

    return (
      <svg
        width="100%"
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        aria-hidden="true"
        style={{ display: 'block' }}
      >
        <path d={d} fill="none" stroke={stroke} strokeWidth={1.5} strokeLinecap="round" />
      </svg>
    );
  };

  const renderMetricCard = (
    label: string,
    value: string | number,
    subtext?: string,
    color?: string,
    loading: boolean = false,
    tooltip?: string
  ) => {
    const isWarning = color === colors.connection.degraded;
    const isError = color === colors.connection.unavailable;
    const isSuccess = color === colors.connection.healthy;
    const isCritical = isWarning || isError;
    // V-11: Get icon for this metric type
    const icon = METRIC_ICONS[label];
    // V-12/V-13: Compute sparkline series and trend indicator based on label.
    const sparklineValues =
      label === 'Messages Received'
        ? messagesReceivedSeries
        : label === 'Messages/sec'
          ? messagesPerSecondSeries
          : label === 'Error Rate'
            ? lifetimeErrorRateSeries
            : (metricHistory[label] ?? null);
    const higherIsBetter = METRIC_HIGHER_IS_BETTER[label] ?? null;
    const trend = computeTrend(sparklineValues, higherIsBetter);

    return (
      <div style={{
        flex: '1 1 150px',
        padding: '16px',
        backgroundColor: colors.bg.card,
        borderRadius: '8px',
        textAlign: 'center',
        minWidth: '150px',
        borderLeft: isCritical ? `4px solid ${color}` : isSuccess ? `4px solid ${colors.connection.healthy}` : `4px solid ${colors.border.primary}`,
        transition: 'transform 0.2s, box-shadow 0.2s',
        position: 'relative' as const,
      }}>
        {/* V-11: Icon positioned at top-left of card */}
        {icon && (
          <div style={{
            position: 'absolute',
            top: '8px',
            left: '8px',
            fontSize: '14px',
            opacity: 0.7,
          }} aria-hidden="true">
            {icon}
          </div>
        )}
        <div style={{
          fontSize: isCritical ? '28px' : '24px',
          fontWeight: 'bold',
          color: color || colors.text.white,
        }}>
          {loading ? (
            <span
              className="inline-block animate-pulse bg-gray-700/60 rounded"
              style={{ width: '70%', height: isCritical ? '28px' : '24px' }}
              aria-label="Loading metric"
            />
          ) : (
            <span style={{ display: 'inline-flex', alignItems: 'baseline', justifyContent: 'center', gap: '8px' }}>
              <span>{value}</span>
              {trend && (
                <span style={{ fontSize: '12px', color: trend.color, fontWeight: 600 }}>
                  {trend.arrow}{trend.pctText ? ` ${trend.pctText}` : ''}
                </span>
              )}
            </span>
          )}
        </div>
        {/* V-12: Tiny sparkline for trend context */}
        {!loading && sparklineValues && sparklineValues.length >= 2 && (
          <div style={{ marginTop: '6px', marginBottom: '2px', height: '18px', opacity: 0.9 }}>
            {renderSparkline(sparklineValues, trend?.color ?? colors.text.faint)}
          </div>
        )}
        {/* T-02: Added title attribute for label truncation indication */}
        <div
          style={{
            fontSize: isCritical ? '13px' : '12px',
            color: isCritical ? colors.text.light : colors.text.muted,
            marginTop: '4px',
            fontWeight: isCritical ? 500 : 400,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '4px',
          }}
          title={tooltip || label}
        >
          {label}
          {/* I-13: Help icon with tooltip for metric explanation */}
          {tooltip && (
            <MetricTooltip content={tooltip} position="top">
              <span
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: '14px',
                  height: '14px',
                  borderRadius: '50%',
                  backgroundColor: colors.alpha.white10,
                  color: colors.text.faint,
                  fontSize: '10px',
                  cursor: 'help',
                }}
                aria-label={`Help: ${tooltip}`}
              >
                ?
              </span>
            </MetricTooltip>
          )}
        </div>
        {/* T-02: Added title attribute for subtext truncation indication */}
        {subtext && (
          <div style={{ fontSize: '10px', color: colors.text.faint, marginTop: '2px' }} title={subtext}>
            {loading ? (
              <span
                className="inline-block animate-pulse bg-gray-700/40 rounded"
                style={{ width: '55%', height: '10px' }}
                aria-label="Loading metric detail"
              />
            ) : (
              subtext
            )}
          </div>
        )}
      </div>
    );
  };

  // M-115/M-116: Diagnostics panel for quarantined messages and corrupted runs
  const renderDiagnosticsPanel = () => {
    const quarantined = getQuarantined();
    const sortedRuns = getRunsSorted();
    const corruptedRuns = sortedRuns.filter(r => r.corrupted);

    // Only show panel if there are diagnostics to display
    if (quarantined.length === 0 && corruptedRuns.length === 0) {
      return null;
    }

    return (
      <div style={{
        padding: '16px',
        backgroundColor: colors.bg.card,
        borderRadius: '8px',
        marginTop: '20px',
        border: `1px solid ${colors.statusBg.warningBorder}`,
      }}>
        <h3 style={{ margin: '0 0 16px 0', color: colors.accent.amber, display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ fontSize: '18px' }}>⚠</span>
          Diagnostics
        </h3>

        {/* M-115: Quarantined Messages */}
        {quarantined.length > 0 && (
          <div style={{ marginBottom: corruptedRuns.length > 0 ? '16px' : 0 }}>
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: '8px',
            }}>
              <h4 style={{ margin: 0, fontSize: '14px', color: colors.connection.degraded }}>
                Quarantined Messages ({quarantined.length})
              </h4>
              <button
                type="button"
                onClick={() => clearQuarantine()}
                style={{
                  backgroundColor: 'transparent',
                  border: `1px solid ${colors.text.faint}`,
                  borderRadius: '4px',
                  color: colors.text.muted,
                  padding: '4px 8px',
                  cursor: 'pointer',
                  fontSize: '11px',
                }}
              >
                Clear
              </button>
            </div>
            <p style={{ fontSize: '12px', color: colors.text.muted, margin: '0 0 8px 0' }}>
              Messages without <code style={{ backgroundColor: colors.border.primary, padding: '2px 4px', borderRadius: '2px' }}>thread_id</code> are quarantined (unbound telemetry).
            </p>
            <div style={{
              maxHeight: '150px',
              overflowY: 'auto',
              backgroundColor: colors.bg.overlay,
              borderRadius: '4px',
              padding: '8px',
            }}>
              {quarantined.slice(0, 10).map((q, i) => (
                <div key={i} style={{
                  padding: '6px 8px',
                  marginBottom: i < Math.min(quarantined.length, 10) - 1 ? '4px' : 0,
                  backgroundColor: colors.bg.card,
                  borderLeft: `2px solid ${colors.connection.degraded}`,
                  borderRadius: '2px',
                  fontSize: '11px',
                }}>
                  <div style={{ color: colors.text.muted }}>
                    {formatTimestamp(q.timestamp)} (seq: {q.seq})
                  </div>
                  <div style={{ color: colors.text.lighter }}>
                    Type: <span style={{ color: colors.text.code }}>{q.type}</span>
                  </div>
                  <div style={{ color: colors.text.faint, fontSize: '10px' }}>
                    {q.reason}
                  </div>
                </div>
              ))}
              {quarantined.length > 10 && (
                <div style={{ textAlign: 'center', color: colors.text.faint, fontSize: '11px', marginTop: '8px' }}>
                  ... and {quarantined.length - 10} more
                </div>
              )}
            </div>
          </div>
        )}

        {/* M-116: Corrupted Runs with Debug Details */}
        {corruptedRuns.length > 0 && (
          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
              <h4 style={{ margin: 0, fontSize: '14px', color: colors.connection.unavailable }}>
                Corrupted Runs ({corruptedRuns.length})
              </h4>
              {/* M-745: Reset Cursor button for recovery */}
              <button
                type="button"
                onClick={() => {
                  if (sendCursorReset()) {
                    // Show confirmation (button will be disabled until cursor_reset_complete arrives and clears state)
                  }
                }}
                disabled={!connected}
                style={{
                  padding: '4px 12px',
                  fontSize: '12px',
                  backgroundColor: connected ? colors.status.error : colors.text.faint,
                  color: colors.text.white,
                  border: 'none',
                  borderRadius: '4px',
                  cursor: connected ? 'pointer' : 'not-allowed',
                  fontWeight: 500,
                }}
                title={connected ? 'Reset cursor to latest position and clear UI state' : 'Not connected'}
              >
                Reset Cursor
              </button>
            </div>
            <p style={{ fontSize: '12px', color: colors.text.muted, margin: '0 0 8px 0' }}>
              Runs with state hash mismatches. This may indicate data corruption or hash algorithm drift.
              Click "Reset Cursor" to clear state and restart from latest position.
            </p>
            <div style={{
              maxHeight: '200px',
              overflowY: 'auto',
              backgroundColor: colors.bg.overlay,
              borderRadius: '4px',
              padding: '8px',
            }}>
              {corruptedRuns.map((run, i) => (
                <div key={run.threadId} style={{
                  padding: '8px',
                  marginBottom: i < corruptedRuns.length - 1 ? '8px' : 0,
                  backgroundColor: colors.bg.card,
                  borderLeft: `2px solid ${colors.connection.unavailable}`,
                  borderRadius: '2px',
                }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <span style={{ fontWeight: 500, color: colors.text.lighter }}>{run.graphName}</span>
                    <span style={{ fontSize: '11px', color: colors.text.muted }}>
                      {formatTimestamp(run.startTime)}
                    </span>
                  </div>
                  <div style={{ fontSize: '11px', color: colors.text.muted, marginTop: '4px' }}>
                    Thread: <code style={{ backgroundColor: colors.border.primary, padding: '1px 4px', borderRadius: '2px' }}>
                      {run.threadId.slice(0, 16)}...
                    </code>
                  </div>
                  {run.corruptionDetails && (
                    <div style={{
                      marginTop: '8px',
                      padding: '6px',
                      backgroundColor: colors.statusBg.error,
                      borderRadius: '4px',
                      fontSize: '10px',
                      fontFamily: 'monospace',
                    }}>
                      <div style={{ color: colors.connection.unavailable, marginBottom: '4px' }}>
                        First mismatch at seq {run.corruptionDetails.firstMismatchSeq} ({formatTimestamp(run.corruptionDetails.firstMismatchTime)})
                      </div>
                      <div style={{ color: colors.text.muted }}>
                        Expected: <span style={{ color: colors.accent.amber }}>{run.corruptionDetails.expectedHash.slice(0, 16)}...</span>
                      </div>
                      <div style={{ color: colors.text.muted }}>
                        Computed: <span style={{ color: colors.status.error }}>{run.corruptionDetails.computedHash.slice(0, 16)}...</span>
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    );
  };

  // Overview tab content
  const renderOverview = () => {
    // TC-06: Skeleton states while waiting for first health snapshot
    const healthLoading = connected && !health && !healthError;

    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
      {/* Status Row */}
      <div style={{ display: 'flex', gap: '20px', flexWrap: 'wrap' }}>
        {/* Connection Status */}
        <div style={{
          flex: '1 1 300px',
          padding: '16px',
          backgroundColor: colors.bg.card,
          borderRadius: '8px'
        }}>
          {/* T-03: Section header with icon for visual hierarchy */}
          <h3 style={{ margin: '0 0 12px 0', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span aria-hidden="true">📡</span> Connection Status
          </h3>
          {/* C-01: WebSocket status with reconnection feedback */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
            <span style={{
              width: '12px',
              height: '12px',
              borderRadius: '50%',
              backgroundColor: connected
                ? colors.connection.healthy
                : wsRetryAttempt
                  ? colors.connection.reconnecting
                  : colors.connection.unavailable
            }} />
            <span>
              WebSocket: {connected
                ? 'Connected'
                : wsRetryAttempt
                  ? `Reconnecting... (attempt ${wsRetryAttempt.attempt}/${wsRetryAttempt.maxRetries})`
                  : 'Disconnected'}
            </span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
            <span>Server Status: </span>
            {renderStatusBadge(
              health && !healthStale
                ? health.status
                : healthStale
                  ? 'stale'
                  : healthError
                    ? 'unavailable'
                    : 'waiting'
            )}
          </div>
          {health && !healthStale ? (
            <div style={{ fontSize: '12px', color: colors.text.muted }}>
              {/* T-08: Map internal Kafka states to user-friendly text */}
              Kafka: {formatKafkaStatus(health.kafka_status)}
            </div>
          ) : (
            <div style={{ fontSize: '12px', color: colors.text.muted }}>
              Health: {healthError ?? (healthStale ? 'stale' : 'waiting')}
              {healthLastOkAt != null ? ` (last OK ${Math.round((Date.now() - healthLastOkAt) / 1000)}s ago)` : ''}
            </div>
          )}
          {wsError && (
            <div style={{ color: colors.connection.unavailable, fontSize: '12px', marginTop: '8px' }}>
              {wsError}
            </div>
          )}
        </div>

        {/* Circuit Breaker */}
        <div style={{ flex: '1 1 300px' }}>
          {healthLoading ? (
            <div
              style={{
                padding: '12px',
                borderRadius: '8px',
                backgroundColor: colors.alpha.gray08,
                border: `1px solid ${colors.alpha.gray25}`,
              }}
              aria-busy="true"
            >
              <div className="animate-pulse bg-gray-700/60 rounded" style={{ height: '14px', width: '55%', marginBottom: '10px' }} />
              <div className="animate-pulse bg-gray-700/40 rounded" style={{ height: '10px', width: '70%', marginBottom: '6px' }} />
              <div className="animate-pulse bg-gray-700/40 rounded" style={{ height: '10px', width: '45%' }} />
            </div>
          ) : (
            renderCircuitBreaker()
          )}
        </div>
      </div>

      {/* D-09: Data freshness indicator - shows when data was last updated */}
      {healthLastOkAt != null && (
        <div style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'flex-end',
          gap: '6px',
          marginBottom: '8px',
          fontSize: '11px',
        }}>
          {(() => {
            const secondsAgo = Math.round((Date.now() - healthLastOkAt) / 1000);
            const color = secondsAgo > 60
              ? colors.connection.unavailable
              : secondsAgo > 30
                ? colors.connection.degraded
                : colors.connection.healthy;
            return (
              <>
                <span style={{
                  width: '6px',
                  height: '6px',
                  borderRadius: '50%',
                  backgroundColor: color,
                }} />
                <span style={{ color }}>
                  Last updated: {secondsAgo}s ago
                </span>
              </>
            );
          })()}
        </div>
      )}

      {/* Metrics Cards - I-13: Added tooltips to explain each metric */}
      {/* I-12: Replaced "—" placeholders with descriptive text */}
      <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
        {/* T-07: Added "since boot" context to cumulative counts */}
        {renderMetricCard(
          'Messages Received',
          health && !healthStale ? health.metrics.kafka_messages_received.toLocaleString() : 'Waiting...',
          healthStale ? 'Data is stale' : 'Since boot',
          undefined,
          healthLoading,
          'Total Kafka messages received since server started'
        )}
        {renderMetricCard(
          'Messages/sec',
          messagesPerSecond != null ? `${messagesPerSecond.toFixed(1)}/sec` : 'Calculating...',
          healthStale ? 'Data is stale' : 'Average rate',
          messagesPerSecond != null ? (messagesPerSecond > 0 ? colors.connection.healthy : colors.text.muted) : undefined,
          false,
          'Average message processing rate calculated over a rolling window'
        )}
        {/* D-07: Use toFixed(1) for consistent decimal display */}
        {renderMetricCard(
          'Error Rate',
          errorRate != null ? `${errorRate.toFixed(1)}%` : 'Calculating...',
          'Since boot',  // M-790: Clarify this is lifetime ratio, not current/rolling rate
          errorRate != null ? (errorRate > 1 ? colors.connection.unavailable : colors.connection.healthy) : undefined,
          false,
          'Percentage of messages that resulted in errors since server boot'
        )}
        {/* M-1099: Show windowed dropped messages (last 2m) as primary, lifetime as context */}
        {/* T-07: Added "since boot" context to total count */}
        {renderMetricCard(
          'Dropped (2m)',
          health && !healthStale ? (health.metrics.dropped_messages_last_120s ?? 0).toLocaleString() : 'Waiting...',
          healthStale ? 'Data is stale' : `${health?.metrics.dropped_messages.toLocaleString() ?? 0} total (since boot)`,
          health && !healthStale && (health.metrics.dropped_messages_last_120s ?? 0) > 10 ? colors.connection.degraded : undefined,
          healthLoading,
          'Messages dropped in the last 2 minutes due to backpressure or client disconnect'
        )}
        {/* M-1099: Show windowed decode error rate as primary metric */}
        {/* D-07: Use toFixed(1) for consistent percentage display */}
        {/* T-07: Added "since boot" context to total count */}
        {renderMetricCard(
          'Decode Errors (2m)',
          health && !healthStale
            ? (() => {
                const errorsLast120s = health.metrics.decode_errors_last_120s ?? 0;
                const msgsLast120s = health.metrics.messages_last_120s ?? 0;
                if (msgsLast120s === 0) return errorsLast120s.toString();
                const rate = ((errorsLast120s / msgsLast120s) * 100).toFixed(1);
                return `${errorsLast120s} (${rate}%)`;
              })()
            : 'Waiting...',
          healthStale ? 'Data is stale' : `${health?.metrics.decode_errors.toLocaleString() ?? 0} total (since boot)`,
          health && !healthStale
            ? ((health.metrics.decode_errors_last_120s ?? 0) > 0 ? colors.connection.unavailable : colors.connection.healthy)
            : undefined,
          healthLoading,
          'Messages that failed to parse or decode in the last 2 minutes (may indicate schema mismatch)'
        )}
        {/* M-1099: Show send failures/timeouts for operator visibility */}
        {/* T-07: Added "since boot" context to clarify cumulative counts */}
        {renderMetricCard(
          'Send Failures',
          health && !healthStale
            ? `${(health.metrics.send_failed ?? 0) + (health.metrics.send_timeout ?? 0)}`
            : 'Waiting...',
          healthStale
            ? 'Data is stale'
            : `failed: ${health?.metrics.send_failed ?? 0}, timeout: ${health?.metrics.send_timeout ?? 0} (since boot)`,
          health && !healthStale && ((health.metrics.send_failed ?? 0) + (health.metrics.send_timeout ?? 0)) > 0
            ? colors.connection.degraded
            : undefined,
          healthLoading,
          'WebSocket send failures: messages that could not be delivered to clients'
        )}
        {renderMetricCard(
          'Uptime',
          health && !healthStale ? formatUptime(health.metrics.uptime_seconds) : 'Waiting...',
          healthStale ? 'Data is stale' : undefined,
          undefined,
          healthLoading,
          'Time elapsed since the server process started'
        )}
        {/* M-999: Apply lag metrics - visible without devtools */}
        {/* M-1004: Fix threshold ordering - check highest severity first */}
        {/* M-1018: Show 60s windowed average as primary (responsive to spikes) */}
        {renderMetricCard(
          'Apply Lag (60s)',
          applyLagInfo ? `${applyLagInfo.windowedAvgMs.toFixed(0)}ms` : 'No data',
          applyLagInfo ? `max: ${applyLagInfo.windowedMaxMs.toFixed(0)}ms (n=${applyLagInfo.windowedCount})` : 'Waiting for messages',
          applyLagInfo
            ? (applyLagInfo.windowedAvgMs > 5000
                ? colors.connection.unavailable
                : applyLagInfo.windowedAvgMs > 1000
                  ? colors.connection.degraded
                  : colors.connection.healthy)
            : undefined,
          false,
          'Average delay between message receipt and UI update over the last 60 seconds. High values indicate processing bottlenecks.'
        )}
        {renderMetricCard(
          'Apply Queue',
          applyLagInfo ? applyLagInfo.pendingCount.toString() : 'No data',
          applyLagInfo ? `${applyLagInfo.totalApplied.toLocaleString()} applied` : 'Waiting for messages',
          applyLagInfo
            ? (applyLagInfo.pendingCount > 500
                ? colors.connection.unavailable
                : applyLagInfo.pendingCount > 100
                  ? colors.connection.degraded
                  : colors.connection.healthy)
            : undefined,
          false,
          'Messages waiting to be processed. High values indicate the UI is falling behind.'
        )}
        {/* M-1099: Replay buffer metrics for reconnection visibility */}
        {/* I-11: Handle undefined values gracefully to avoid "undefined/undefined" display */}
        {renderMetricCard(
          'Replay Buffer',
          health && !healthStale && health.replay_buffer
            ? (health.replay_buffer.memory_buffer_size != null && health.replay_buffer.max_memory_size != null
                ? `${health.replay_buffer.memory_buffer_size}/${health.replay_buffer.max_memory_size}`
                : 'Not configured')
            : 'Waiting...',
          healthStale
            ? 'Data is stale'
            : health?.replay_buffer
              ? `Redis: ${health.replay_buffer.redis_enabled ? 'on' : 'off'}, hits: ${(health.replay_buffer.memory_hits ?? 0) + (health.replay_buffer.redis_hits ?? 0)}`
              : 'No replay buffer data',
          undefined,
          healthLoading,
          'In-memory buffer for message replay on reconnection. Shows current/max capacity.'
        )}
      </div>

      {/* Alert Banner */}
      {health && !healthStale && health.alert && (
        <div style={{
          padding: '12px 16px',
          backgroundColor: colors.statusBg.error,
          border: `1px solid ${colors.connection.unavailable}`,
          borderRadius: '8px',
          color: colors.connection.unavailable
        }}>
          <strong>Alert:</strong> {health.alert}
        </div>
      )}

      {/* Throughput Chart - I-06: Increased axis label size for readability */}
      {/* V-15: Improved empty chart state with meaningful message */}
      <div style={{
        padding: '16px',
        backgroundColor: colors.bg.card,
        borderRadius: '8px'
      }}>
        {/* T-03: Section header with icon for visual hierarchy */}
        <h3 style={{ margin: '0 0 16px 0', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span aria-hidden="true">📈</span> Message Throughput
        </h3>
        {throughputData.length === 0 ? (
          <div
            style={{
              height: 220,
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: colors.bg.emptyState,
              borderRadius: '8px',
              border: `1px dashed ${colors.border.dashed}`,
            }}
          >
            <div
              className="animate-spin"
              style={{
                width: 24,
                height: 24,
                border: `2px solid ${colors.border.dashed}`,
                borderTopColor: colors.chart.purple,
                borderRadius: '50%',
                marginBottom: 12,
              }}
            />
            <div style={{ color: colors.text.muted, fontSize: '14px', fontWeight: 500 }}>
              No data yet
            </div>
            <div style={{ color: colors.text.faint, fontSize: '12px', marginTop: 4 }}>
              Waiting for first message...
            </div>
          </div>
        ) : (
          <ResponsiveContainer width="100%" height={220}>
            <AreaChart data={throughputData} margin={{ top: 10, right: 30, left: 10, bottom: 5 }}>
              <CartesianGrid strokeDasharray="3 3" stroke={colors.border.primary} />
              {/* B-07: Improved X-axis timestamp readability */}
              <XAxis
                dataKey="time"
                stroke={colors.text.tertiary}
                fontSize={11}
                tickLine={{ stroke: colors.border.hover }}
                tick={{ fill: colors.text.tertiary }}
                height={40}
                interval="preserveStartEnd"
                minTickGap={30}
              />
              <YAxis
                stroke={colors.text.tertiary}
                fontSize={12}
                tickLine={{ stroke: colors.border.hover }}
                tick={{ fill: colors.text.tertiary }}
                tickFormatter={(value) => `${value.toLocaleString()} msg`}
              />
              <Tooltip
                contentStyle={{ backgroundColor: colors.bg.card, border: `1px solid ${colors.border.primary}`, borderRadius: '6px' }}
                labelStyle={{ color: colors.text.light, fontWeight: 'bold' }}
              />
              <Area
                type="monotone"
                dataKey="messages"
                stroke={colors.chart.purple}
                fill={colors.chart.purple}
                fillOpacity={0.3}
                name="Total Messages"
                dot={{ r: 2, fill: colors.chart.purple }}
                activeDot={{ r: 4, fill: colors.chart.purple, stroke: colors.text.white }}
              />
            </AreaChart>
          </ResponsiveContainer>
        )}
      </div>

      {/* Version Info - V-19: Improved Build Info readability */}
      {version && (
        <div style={{
          padding: '12px 16px',
          backgroundColor: colors.bg.card,
          borderRadius: '8px',
          fontSize: '12px',
          color: colors.text.tertiary,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: '8px'
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '16px', flexWrap: 'wrap' }}>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <strong style={{ color: colors.text.light }}>Component:</strong>
              <span style={{ color: colors.text.link }}>{version.component}</span>
            </span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <strong style={{ color: colors.text.light }}>SHA:</strong>
              <code style={{
                backgroundColor: colors.alpha.white05,
                padding: '2px 6px',
                borderRadius: '4px',
                fontFamily: 'monospace',
                fontSize: '11px',
                color: colors.text.code,
                cursor: 'pointer'
              }}
              onClick={() => {
                navigator.clipboard.writeText(version.git_sha);
              }}
              title="Click to copy SHA"
              >
                {version.git_sha.substring(0, 7)}
              </code>
            </span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <strong style={{ color: colors.text.light }}>Schema:</strong>
              <span>v{version.schema_version}</span>
            </span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <strong style={{ color: colors.text.light }}>Built:</strong>
              <span style={{ color: colors.text.muted }}>{version.build_date}</span>
            </span>
          </div>
        </div>
      )}

      {/* M-115/M-116: Diagnostics Panel */}
      {renderDiagnosticsPanel()}
      </div>
    );
  };

  // Events tab content
  const renderEvents = () => (
    <div style={{
      backgroundColor: colors.bg.card,
      borderRadius: '8px',
      padding: '16px',
      maxHeight: '600px',
      overflowY: 'auto'
    }}>
      {/* T-03: Section header with icon for visual hierarchy */}
      <h3 style={{ margin: '0 0 16px 0', display: 'flex', alignItems: 'center', gap: '8px' }}>
        <span aria-hidden="true">📜</span> Live Event Stream ({events.length} events)
      </h3>

      {/* Gap Indicators */}
      {gapIndicators.length > 0 && (
        <div style={{
          marginBottom: '16px',
          padding: '8px 12px',
          backgroundColor: colors.statusBg.warning,
          borderRadius: '4px',
          fontSize: '12px',
          color: colors.connection.degraded
        }}>
          Recent gaps: {gapIndicators.map((g, i) => (
            <span key={i} style={{ marginLeft: '8px' }}>
              {g.time}: {g.count} msgs
            </span>
          ))}
        </div>
      )}

      {events.length === 0 ? (
        <div style={{ padding: '20px', textAlign: 'center', color: colors.text.faint }}>
          {connected ? 'Waiting for events...' : 'Not connected to WebSocket server'}
        </div>
      ) : (
        events.map((event, i) => (
          <div
            key={i}
            style={{
              padding: '10px',
              marginBottom: '5px',
              backgroundColor: colors.bg.overlay,
              borderLeft: `3px solid ${colors.connection.reconnecting}`,
              borderRadius: '2px'
            }}
          >
            <div style={{ fontSize: '12px', color: colors.text.faint }}>
              {formatTimestamp(event.timestamp)}
              {event.sequence !== undefined && (
                <span style={{ marginLeft: '12px', color: colors.text.muted }}>
                  seq: {event.sequence}
                </span>
              )}
            </div>
            <div style={{ marginTop: '5px' }}>
              <span style={{ fontWeight: 'bold' }}>Type:</span> {event.type}
              {event.thread_id && (
                <span style={{ marginLeft: '15px' }}>
                  <span style={{ fontWeight: 'bold' }}>Thread:</span> {event.thread_id}
                </span>
              )}
              {event.quality !== undefined && (
                <span style={{ marginLeft: '15px' }}>
                  <span style={{ fontWeight: 'bold' }}>Quality:</span> {event.quality.toFixed(2)}
                </span>
              )}
              {event.model && (
                <span style={{ marginLeft: '15px' }}>
                  <span style={{ fontWeight: 'bold' }}>Model:</span> {event.model}
                </span>
              )}
            </div>
          </div>
        ))
      )}
    </div>
  );

  // Metrics tab content
  const renderMetrics = () => (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
      {/* Error Distribution */}
      <div style={{
        padding: '16px',
        backgroundColor: colors.bg.card,
        borderRadius: '8px'
      }}>
        {/* T-03: Section header with icon for visual hierarchy */}
        <h3 style={{ margin: '0 0 16px 0', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span aria-hidden="true">⚠️</span> Error Distribution
        </h3>
        {health && !healthStale ? (
          <div style={{ display: 'flex', gap: '20px' }}>
            <ResponsiveContainer width="50%" height={200}>
              <PieChart>
                {/* M-1100: Removed "Kafka Errors" because kafka_errors == decode_errors (both
                    incremented on decode failure). Showing both double-counts decode errors.
                    Now showing: Decode Errors (new data), Old Data Errors, Infrastructure */}
                <Pie
                  data={[
                    { name: 'Decode Errors', value: health.metrics.decode_errors },
                    { name: 'Old Data Errors', value: health.metrics.old_data_decode_errors ?? 0 },
                    { name: 'Infrastructure', value: health.metrics.infrastructure_errors },
                  ]}
                  cx="50%"
                  cy="50%"
                  innerRadius={40}
                  outerRadius={80}
                  paddingAngle={5}
                  dataKey="value"
                >
                  {CHART_COLORS.map((color, index) => (
                    <Cell key={`cell-${index}`} fill={color} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{ backgroundColor: colors.bg.card, border: `1px solid ${colors.border.primary}` }}
                />
              </PieChart>
            </ResponsiveContainer>
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'center' }}>
              {/* M-1100: Decode errors = problematic (new data corruption) */}
              <div style={{ marginBottom: '8px' }}>
                <span style={{ display: 'inline-block', width: '12px', height: '12px', backgroundColor: CHART_COLORS[0], marginRight: '8px' }} />
                Decode Errors: {health.metrics.decode_errors}
              </div>
              {/* M-1100: Old data errors = expected during catch-up (pre-session data) */}
              <div style={{ marginBottom: '8px' }}>
                <span style={{ display: 'inline-block', width: '12px', height: '12px', backgroundColor: CHART_COLORS[1], marginRight: '8px' }} />
                Old Data Errors: {health.metrics.old_data_decode_errors ?? 0}
              </div>
              <div style={{ marginBottom: '8px' }}>
                <span style={{ display: 'inline-block', width: '12px', height: '12px', backgroundColor: CHART_COLORS[2], marginRight: '8px' }} />
                Infrastructure: {health.metrics.infrastructure_errors}
              </div>
            </div>
          </div>
        ) : (
          <div style={{ fontSize: '12px', color: colors.text.muted }}>
            Health: {healthError ?? (healthStale ? 'stale' : 'unavailable')}
            {healthLastOkAt != null ? ` (last OK ${Math.round((Date.now() - healthLastOkAt) / 1000)}s ago)` : ''}
          </div>
        )}
      </div>

      {/* Latency Chart - B-06: Increased axis label size for readability */}
      <div style={{
        padding: '16px',
        backgroundColor: colors.bg.card,
        borderRadius: '8px'
      }}>
        {/* T-03: Section header with icon for visual hierarchy */}
        <h3 style={{ margin: '0 0 16px 0', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span aria-hidden="true">⏱️</span> Message Latency (ms)
        </h3>
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={latencyData}>
            <CartesianGrid strokeDasharray="3 3" stroke={colors.border.primary} />
            {/* B-07: Improved X-axis timestamp readability */}
            <XAxis
              dataKey="time"
              stroke={colors.text.tertiary}
              fontSize={11}
              tickLine={{ stroke: colors.text.disabled }}
              tick={{ fill: colors.text.tertiary }}
              height={40}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            <YAxis
              stroke={colors.text.tertiary}
              fontSize={12}
              tickLine={{ stroke: colors.text.disabled }}
              tick={{ fill: colors.text.tertiary }}
              tickFormatter={(value) => `${value} ms`}
            />
            <Tooltip
              contentStyle={{ backgroundColor: colors.bg.card, border: `1px solid ${colors.border.primary}`, borderRadius: '6px' }}
              labelStyle={{ color: colors.text.light, fontWeight: 'bold' }}
            />
            <Line
              type="monotone"
              dataKey="latency"
              stroke={colors.chart.green}
              strokeWidth={2}
              dot={{ r: 2, fill: colors.chart.green }}
              activeDot={{ r: 4, fill: colors.chart.green, stroke: colors.text.white }}
              name="Latency (ms)"
            />
          </LineChart>
        </ResponsiveContainer>
      </div>

      {/* Error Trend - B-06: Increased axis label size for readability */}
      <div style={{
        padding: '16px',
        backgroundColor: colors.bg.card,
        borderRadius: '8px'
      }}>
        {/* T-03: Section header with icon for visual hierarchy */}
        <h3 style={{ margin: '0 0 16px 0', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span aria-hidden="true">📉</span> Error Trend
        </h3>
        <ResponsiveContainer width="100%" height={200}>
          <BarChart data={throughputData}>
            <CartesianGrid strokeDasharray="3 3" stroke={colors.border.primary} />
            {/* B-07: Improved X-axis timestamp readability */}
            <XAxis
              dataKey="time"
              stroke={colors.text.tertiary}
              fontSize={11}
              tickLine={{ stroke: colors.text.disabled }}
              tick={{ fill: colors.text.tertiary }}
              height={40}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            {/* B-08: Added number formatting to error count axis */}
            <YAxis
              stroke={colors.text.tertiary}
              fontSize={12}
              tickLine={{ stroke: colors.text.disabled }}
              tick={{ fill: colors.text.tertiary }}
              tickFormatter={(value) => `${value.toLocaleString()}`}
            />
            <Tooltip
              contentStyle={{ backgroundColor: colors.bg.card, border: `1px solid ${colors.border.primary}`, borderRadius: '6px' }}
              labelStyle={{ color: colors.text.light, fontWeight: 'bold' }}
            />
            <Bar dataKey="errors" fill={colors.connection.unavailable} name="Cumulative Errors" />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );

  return (
    <div style={{
      padding: '20px',
      fontFamily: 'Inter, system-ui, sans-serif',
      backgroundColor: colors.bg.overlay,
      minHeight: '100vh',
      color: colors.text.white
    }}>
      {/* Header */}
      <header style={{
        marginBottom: '20px',
        borderBottom: `2px solid ${colors.border.primary}`,
        paddingBottom: '16px',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        flexWrap: 'wrap',
        gap: '12px'
      }}>
        <div>
          <h1 style={{ margin: 0, fontSize: '24px' }}>DashStream Observability Dashboard</h1>
          <div style={{ fontSize: '12px', color: colors.text.muted, marginTop: '4px' }}>
            Real-time streaming metrics and event monitoring
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <span style={{
            width: '10px',
            height: '10px',
            borderRadius: '50%',
            backgroundColor: connected ? colors.connection.healthy : colors.connection.unavailable,
            display: 'inline-block'
          }} />
          <span style={{ fontSize: '14px' }}>
            {connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
      </header>

      {/* M-440: Demo mode banner - clear visual indication when showing sample data */}
      {isDemoMode && (
        <div style={{
          backgroundColor: colors.ui.bannerWarning,
          color: colors.text.black,
          padding: '12px 20px',
          borderRadius: '8px',
          marginBottom: '16px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '12px'
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{ fontSize: '20px' }}>&#9888;</span>
            <div>
              <strong style={{ display: 'block' }}>Demo Mode</strong>
              <span style={{ fontSize: '13px', opacity: 0.9 }}>
                Showing sample data. Connect to DashStream or run the demo script to see live data.
              </span>
            </div>
          </div>
          <code style={{
            backgroundColor: colors.alpha.black10,
            padding: '4px 8px',
            borderRadius: '4px',
            fontSize: '12px'
          }}>
            ./demo_observability.sh
          </code>
        </div>
      )}

      {/* M-997: Schema mismatch banner - block state mutation and clearly indicate incompatibility */}
      {schemaVersionMismatchInfo && (
        <div style={{
          backgroundColor: colors.ui.bannerError,
          color: colors.text.white,
          padding: '12px 20px',
          borderRadius: '8px',
          marginBottom: '16px',
          border: `1px solid ${colors.alpha.white15}`,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '12px',
          flexWrap: 'wrap'
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <span style={{ fontSize: '18px' }}>&#9888;</span>
            <div>
              <strong style={{ display: 'block' }}>Schema Version Mismatch</strong>
              <span style={{ fontSize: '13px', opacity: 0.95 }}>
                Streaming is paused to prevent incorrect state. Message schema v{schemaVersionMismatchInfo.messageSchemaVersion} &gt; UI expects v{schemaVersionMismatchInfo.expectedSchemaVersion}.
                Update the UI proto bindings and reload.
              </span>
            </div>
          </div>
          <code style={{
            backgroundColor: colors.alpha.black20,
            padding: '4px 8px',
            borderRadius: '4px',
            fontSize: '12px'
          }}>
            cd observability-ui &amp;&amp; npm run proto:gen &amp;&amp; npm run build
          </code>
        </div>
      )}

      {/* M-1019: Config drift warning - server accepts larger payloads than UI can decode */}
      {configDriftWarning && (
        <div style={{
          backgroundColor: colors.ui.bannerWarningDark,
          color: colors.text.black,
          padding: '12px 20px',
          borderRadius: '8px',
          marginBottom: '16px',
          border: `1px solid ${colors.alpha.black15}`,
          display: 'flex',
          alignItems: 'center',
          gap: '10px'
        }}>
          <span style={{ fontSize: '18px' }}>&#9888;</span>
          <div>
            <strong style={{ display: 'block' }}>Configuration Drift Detected</strong>
            <span style={{ fontSize: '13px', opacity: 0.9 }}>
              {configDriftWarning}
            </span>
          </div>
        </div>
      )}

      {/* Tabs - M-462: Added ARIA attributes for accessibility */}
      <div
        role="tablist"
        aria-label="Dashboard sections"
        style={{
          display: 'flex',
          gap: '8px',
          marginBottom: '20px',
          borderBottom: `1px solid ${colors.border.primary}`,
          paddingBottom: '8px'
        }}
      >
        {(['overview', 'events', 'metrics', 'graph'] as const).map(tab => (
          <button
            key={tab}
            type="button"
            role="tab"
            id={`tab-${tab}`}
            aria-selected={activeTab === tab}
            aria-controls={`tabpanel-${tab}`}
            tabIndex={activeTab === tab ? 0 : -1}
            onClick={() => setActiveTab(tab)}
            // TC-07: Visible focus ring for keyboard navigation
            className="focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/70 focus-visible:ring-offset-2 focus-visible:ring-offset-[#0f0f0f]"
            onKeyDown={(e) => {
              // M-462: Keyboard navigation for tabs
              const tabs = ['overview', 'events', 'metrics', 'graph'] as const;
              const currentIndex = tabs.indexOf(tab);
              if (e.key === 'ArrowRight') {
                const nextIndex = (currentIndex + 1) % tabs.length;
                setActiveTab(tabs[nextIndex]);
                document.getElementById(`tab-${tabs[nextIndex]}`)?.focus();
              } else if (e.key === 'ArrowLeft') {
                const prevIndex = (currentIndex - 1 + tabs.length) % tabs.length;
                setActiveTab(tabs[prevIndex]);
                document.getElementById(`tab-${tabs[prevIndex]}`)?.focus();
              } else if (e.key === 'Home') {
                setActiveTab(tabs[0]);
                document.getElementById(`tab-${tabs[0]}`)?.focus();
              } else if (e.key === 'End') {
                setActiveTab(tabs[tabs.length - 1]);
                document.getElementById(`tab-${tabs[tabs.length - 1]}`)?.focus();
              }
            }}
            style={{
              padding: '8px 16px',
              border: 'none',
              borderRadius: '4px',
              backgroundColor: activeTab === tab ? colors.ui.tabActive : 'transparent',
              color: activeTab === tab ? colors.text.white : colors.text.muted,
              cursor: 'pointer',
              fontSize: '14px',
              fontWeight: activeTab === tab ? 'bold' : 'normal',
              textTransform: 'capitalize'
            }}
          >
            {/* M-462: Added aria-label for graph emoji */}
            {tab === 'graph' ? (
              <span aria-label="Graph visualization">📊 Graph</span>
            ) : tab}
          </button>
        ))}
      </div>

      {/* M-462/A-05: Live region for status updates - announces connection and health status changes */}
      <div
        role="status"
        aria-live="polite"
        aria-atomic="true"
        className="sr-only"
        style={{
          position: 'absolute',
          width: '1px',
          height: '1px',
          padding: 0,
          margin: '-1px',
          overflow: 'hidden',
          clip: 'rect(0, 0, 0, 0)',
          whiteSpace: 'nowrap',
          border: 0
        }}
      >
        {/* A-05: Enhanced status announcement including WebSocket, health, and circuit breaker state */}
        {(() => {
          const parts: string[] = [];
          // WebSocket status
          if (connected) {
            parts.push('Connected to server');
          } else if (wsRetryAttempt) {
            parts.push(`Reconnecting, attempt ${wsRetryAttempt.attempt} of ${wsRetryAttempt.maxRetries}`);
          } else {
            parts.push('Disconnected from server');
          }
          // Health status
          if (health && !healthStale) {
            parts.push(`Server status: ${health.status}`);
            // Circuit breaker status if degraded
            if (health.circuit_breaker && health.circuit_breaker.state !== 'healthy') {
              parts.push(`Circuit breaker: ${health.circuit_breaker.state.replace(/_/g, ' ')}`);
            }
          } else if (healthStale) {
            parts.push('Health data is stale');
          } else if (healthError) {
            parts.push(`Health check error: ${healthError}`);
          }
          return parts.join('. ');
        })()}
      </div>

      {/* Tab Content - M-462: Added tabpanel roles */}
      <main>
        {activeTab === 'overview' && (
          <div
            role="tabpanel"
            id="tabpanel-overview"
            aria-labelledby="tab-overview"
          >
            {renderOverview()}
          </div>
        )}
        {activeTab === 'events' && (
          <div
            role="tabpanel"
            id="tabpanel-events"
            aria-labelledby="tab-events"
          >
            {renderEvents()}
          </div>
        )}
        {activeTab === 'metrics' && (
          <div
            role="tabpanel"
            id="tabpanel-metrics"
            aria-labelledby="tab-metrics"
          >
            {renderMetrics()}
          </div>
        )}
        {activeTab === 'graph' && (
          <div
            role="tabpanel"
            id="tabpanel-graph"
            aria-labelledby="tab-graph"
            style={{ display: 'flex', flexDirection: 'column', height: 'calc(100vh - 200px)' }}
          >
            {/* Graph Header with LIVE indicator and elapsed time */}
            <div className="graph-header" style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '12px 16px',
              backgroundColor: colors.bg.primary,
              borderRadius: '8px 8px 0 0',
              borderBottom: `1px solid ${colors.border.primary}`,
              marginBottom: '0'
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                {/* LIVE indicator */}
                <div className={`live-indicator ${isGraphLive ? 'active' : ''}`} style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '4px 12px',
                  backgroundColor: isGraphLive ? colors.statusBg.errorStrong : colors.statusBg.neutral,
                  border: `1px solid ${isGraphLive ? colors.statusBg.errorBorderSubtle : colors.statusBg.neutralBorder}`,
                  borderRadius: '16px',
                  fontSize: '12px',
                  fontWeight: 600,
                  color: isGraphLive ? colors.status.error : colors.status.neutral,
                  textTransform: 'uppercase'
                }}>
                  <span className={`live-dot ${isGraphLive ? 'active' : ''}`} style={{
                    width: '8px',
                    height: '8px',
                    borderRadius: '50%',
                    backgroundColor: isGraphLive ? colors.status.error : colors.status.neutral
                  }} />
                  {isGraphLive ? 'LIVE' : 'IDLE'}
                </div>
                {/* Graph name */}
                <span style={{ fontSize: '16px', fontWeight: 600, color: colors.text.primary }}>
                  {effectiveSchema?.name || 'Waiting for graph...'}
                </span>
                {/* Thread ID */}
                {(runCursor?.threadId || viewModel?.cursor.threadId) && (
                  <span style={{
                    fontSize: '12px',
                    color: colors.text.muted,
                    backgroundColor: colors.statusBg.neutral,
                    padding: '2px 8px',
                    borderRadius: '4px'
                  }}>
                    thread: {runCursor?.threadId || viewModel?.cursor.threadId}
                  </span>
                )}
	              </div>
	              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
	                {/* Grouping controls (M-388) */}
	                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
	                  <span style={{ fontSize: '11px', color: colors.text.muted }}>Grouping</span>
	                  <select
	                    value={groupingMode}
	                    onChange={(e) => setGroupingMode(e.target.value as GroupingMode)}
	                    style={{
	                      backgroundColor: colors.bg.secondary,
	                      border: `1px solid ${colors.border.primary}`,
	                      borderRadius: '6px',
	                      color: colors.text.lighter,
	                      padding: '4px 8px',
	                      fontSize: '11px',
	                    }}
	                    title="Group nodes in the graph view (also affects Mermaid export)"
	                  >
	                    <option value="none">None</option>
	                    <option value="node_type">By type</option>
	                    <option value="attribute">By attribute</option>
	                  </select>
	                  {groupingMode === 'attribute' && (
	                    <input
	                      value={groupingAttributeKey}
	                      onChange={(e) => setGroupingAttributeKey(e.target.value)}
	                      placeholder="attr key"
	                      style={{
	                        width: 110,
	                        backgroundColor: colors.bg.secondary,
	                        border: `1px solid ${colors.border.primary}`,
	                        borderRadius: '6px',
	                        color: colors.text.lighter,
	                        padding: '4px 8px',
	                        fontSize: '11px',
	                        fontFamily: 'monospace',
	                      }}
	                      title="Node attribute key to group by (e.g. group, phase, module)"
	                    />
	                  )}
	                </div>

	                {/* View toggle */}
	                <div style={{
	                  display: 'flex',
	                  backgroundColor: colors.bg.secondary,
                  borderRadius: '6px',
                  padding: '2px',
                  border: `1px solid ${colors.border.primary}`,
                }}>
                  <button
                    type="button"
                    onClick={() => setGraphViewMode('canvas')}
                    style={{
                      padding: '4px 12px',
                      border: 'none',
                      borderRadius: '4px',
                      backgroundColor: graphViewMode === 'canvas' ? colors.status.info : 'transparent',
                      color: graphViewMode === 'canvas' ? colors.text.white : colors.text.muted,
                      cursor: 'pointer',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}
                  >
                    Canvas
                  </button>
                  <button
                    type="button"
                    onClick={() => setGraphViewMode('mermaid')}
                    style={{
                      padding: '4px 12px',
                      border: 'none',
                      borderRadius: '4px',
                      backgroundColor: graphViewMode === 'mermaid' ? colors.status.info : 'transparent',
                      color: graphViewMode === 'mermaid' ? colors.text.white : colors.text.muted,
                      cursor: 'pointer',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}
                  >
                    Mermaid
                  </button>
                  <button
                    type="button"
                    onClick={() => setGraphViewMode('history')}
                    style={{
                      padding: '4px 12px',
                      border: 'none',
                      borderRadius: '4px',
                      backgroundColor: graphViewMode === 'history' ? colors.status.info : 'transparent',
                      color: graphViewMode === 'history' ? colors.text.white : colors.text.muted,
                      cursor: 'pointer',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}
                  >
                    History
                  </button>
                </div>
                {/* TYP-09: Export graph dropdown */}
                <div style={{ position: 'relative' }}>
                  <button
                    type="button"
                    onClick={() => setShowExportDropdown(!showExportDropdown)}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '4px',
                      padding: '4px 10px',
                      border: `1px solid ${colors.border.primary}`,
                      borderRadius: '6px',
                      backgroundColor: colors.bg.secondary,
                      color: colors.text.muted,
                      cursor: 'pointer',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}
                    title="Export graph schema"
                    disabled={!viewModel?.schema && !graphSchema}
                  >
                    <span style={{ fontSize: '12px' }}>⬇</span>
                    Export
                  </button>
                  {showExportDropdown && (
                    <div style={{
                      position: 'absolute',
                      top: '100%',
                      right: 0,
                      marginTop: '4px',
                      backgroundColor: colors.bg.dropdown,
                      border: `1px solid ${colors.border.primary}`,
                      borderRadius: '6px',
                      boxShadow: shadows.dropdown,
                      zIndex: 50,
                      minWidth: '140px',
                    }}>
                      <button
                        type="button"
                        onClick={() => handleExport('json')}
                        style={{
                          display: 'block',
                          width: '100%',
                          padding: '8px 12px',
                          border: 'none',
                          backgroundColor: 'transparent',
                          color: colors.text.lighter,
                          cursor: 'pointer',
                          fontSize: '11px',
                          textAlign: 'left',
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = colors.bg.elevated}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                      >
                        📄 JSON (.json)
                      </button>
                      <button
                        type="button"
                        onClick={() => handleExport('dot')}
                        style={{
                          display: 'block',
                          width: '100%',
                          padding: '8px 12px',
                          border: 'none',
                          backgroundColor: 'transparent',
                          color: colors.text.lighter,
                          cursor: 'pointer',
                          fontSize: '11px',
                          textAlign: 'left',
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = colors.bg.elevated}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                      >
                        🔗 GraphViz (.dot)
                      </button>
                      <button
                        type="button"
                        onClick={() => handleExport('mermaid')}
                        style={{
                          display: 'block',
                          width: '100%',
                          padding: '8px 12px',
                          border: 'none',
                          backgroundColor: 'transparent',
                          color: colors.text.lighter,
                          cursor: 'pointer',
                          fontSize: '11px',
                          textAlign: 'left',
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = colors.bg.elevated}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                      >
                        📊 Mermaid (.mermaid)
                      </button>
                    </div>
                  )}
                </div>
                {/* M-114: Expected schema baseline UX with mismatch detection */}
                {/* Current schema indicator */}
                {viewModel?.schemaId && (
                  <span style={{
                    fontSize: '11px',
                    color: colors.status.infoHover,
                    fontFamily: 'monospace',
                    backgroundColor: colors.statusBg.infoLight,
                    padding: '4px 8px',
                    borderRadius: '6px',
                    border: `1px solid ${colors.statusBg.infoBorder}`,
                  }}
                  title={`Current schema ID: ${viewModel.schemaId}`}
                  >
                    current: {viewModel.schemaId.slice(0, 8)}
                  </span>
                )}
                {/* Expected schema pin with graph context */}
                {expectedSchemaId && (
                  <span style={{
                    fontSize: '11px',
                    color: colors.accent.amber,
                    fontFamily: 'monospace',
                    backgroundColor: colors.statusBg.warningAmber,
                    padding: '4px 8px',
                    borderRadius: '6px',
                    border: `1px solid ${colors.statusBg.warningBorder}`,
                  }}
                  title={`Expected schema ID: ${expectedSchemaId}${expectedSchemaGraphName ? `\nFor graph: ${expectedSchemaGraphName}` : ''}`}
                  >
                    expected: {expectedSchemaId.slice(0, 8)}
                    {expectedSchemaGraphName && expectedSchemaGraphName !== currentGraphName && (
                      <span style={{ color: colors.text.muted, marginLeft: '4px' }}>
                        ({expectedSchemaGraphName})
                      </span>
                    )}
                  </span>
                )}
                {/* M-114: Mismatch indicator with reason */}
                {viewModel?.schemaId && expectedSchemaId && viewModel.schemaId !== expectedSchemaId && (
                  <span style={{
                    fontSize: '11px',
                    color: colors.status.error,
                    fontFamily: 'system-ui',
                    backgroundColor: colors.statusBg.errorStrong,
                    padding: '4px 8px',
                    borderRadius: '6px',
                    border: `1px solid ${colors.statusBg.errorBorderSubtle}`,
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                  }}
                  title={`Schema mismatch detected!\n\nCurrent: ${viewModel.schemaId}\nExpected: ${expectedSchemaId}\n\nThe graph structure has changed from the expected baseline. This may indicate:\n- Node additions/removals\n- Edge changes\n- Schema version updates`}
                  >
                    <span style={{ fontWeight: 600 }}>⚠</span>
                    schema mismatch
                  </span>
                )}
                {/* Match indicator when schemas are the same */}
                {viewModel?.schemaId && expectedSchemaId && viewModel.schemaId === expectedSchemaId && (
                  <span style={{
                    fontSize: '11px',
                    color: colors.status.emerald,
                    fontFamily: 'system-ui',
                    backgroundColor: colors.statusBg.emerald,
                    padding: '4px 8px',
                    borderRadius: '6px',
                    border: `1px solid ${colors.statusBg.emeraldBorder}`,
                  }}
                  title="Current schema matches the expected baseline"
                  >
                    ✓ matches baseline
                  </span>
                )}
                <button
                  type="button"
                  onClick={handleSetExpectedSchema}
                  disabled={!viewModel?.schemaId}
                  style={{
                    backgroundColor: colors.bg.secondary,
                    border: `1px solid ${colors.border.primary}`,
                    borderRadius: '6px',
                    color: colors.text.muted,
                    padding: '4px 10px',
                    cursor: 'pointer',
                    fontSize: '11px',
                    fontWeight: 500,
                    opacity: !viewModel?.schemaId ? 0.5 : 1,
                  }}
                  title="Pin current schema as the expected graph baseline"
                >
                  Set expected
                </button>
                {expectedSchemaId && (
                  <button
                    type="button"
                    onClick={handleClearExpectedSchema}
                    style={{
                      backgroundColor: colors.bg.secondary,
                      border: `1px solid ${colors.border.primary}`,
                      borderRadius: '6px',
                      color: colors.text.muted,
                      padding: '4px 10px',
                      cursor: 'pointer',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}
                    title="Clear expected schema baseline"
                  >
                    Clear
                  </button>
                )}
                {/* Elapsed time counter */}
                <span className="elapsed-timer" style={{
                  fontFamily: "'SF Mono', 'Monaco', 'Inconsolata', monospace",
                  fontSize: '14px',
                  fontWeight: 500,
                  color: colors.status.emerald,
                  backgroundColor: colors.statusBg.emerald,
                  padding: '4px 10px',
                  borderRadius: '6px',
                  border: `1px solid ${colors.statusBg.emeraldBorder}`
                }}>
                  {elapsedTime}
                </span>
                {/* Active node indicator */}
                {effectiveCurrentNode && (
                  <span style={{
                    fontSize: '12px',
                    color: colors.status.info,
                    backgroundColor: colors.statusBg.info,
                    padding: '4px 10px',
                    borderRadius: '6px',
                    border: `1px solid ${colors.statusBg.infoBorder}`
                  }}>
                    Running: {effectiveCurrentNode}
                  </span>
                )}
              </div>
            </div>
            {/* Main content area - top section with graph */}
            <div style={{ display: 'flex', gap: '16px', flex: 2, minHeight: '300px' }}>
              {/* Graph View (Canvas, Mermaid, or History) - M-455: Wrapped in ErrorBoundary */}
              <div style={{ flex: 2, minHeight: '300px', height: '100%', position: 'relative' }}>
                <ErrorBoundary name="Graph View">
                  {graphViewMode === 'canvas' && (
                    <GraphCanvas
                      schema={effectiveSchema}
                      nodeExecutions={effectiveNodeExecutions}
                      currentNode={effectiveCurrentNode}
                      selectedNode={selectedNode}
                      onNodeClick={(nodeName) => setSelectedNode(nodeName)}
                      grouping={{
                        mode: groupingMode,
                        attributeKey: groupingMode === 'attribute' ? groupingAttributeKey : undefined,
                      }}
                      // M-447: Pass out-of-schema nodes for visual distinction (red dotted border + ! badge)
                      outOfSchemaNodes={viewModel?.outOfSchemaNodes}
                    />
                  )}
                  {graphViewMode === 'mermaid' && (
                    <MermaidView
                      viewModel={viewModel}
                      options={{
                        grouping: {
                          mode: groupingMode,
                          attributeKey: groupingMode === 'attribute' ? groupingAttributeKey : undefined,
                        },
                      }}
                    />
                  )}
                  {graphViewMode === 'history' && (
                    <div style={{
                      height: '100%',
                      backgroundColor: colors.bg.primary,
                      borderRadius: '8px',
                      border: `1px solid ${colors.border.primary}`,
                      overflow: 'hidden',
                    }}>
                      <SchemaHistoryPanel
                        observations={Array.from(schemaObservations.values())}
                        expectedSchemaId={expectedSchemaId}
                        onSetExpected={(schemaId, graphName) => {
                          setExpectedSchemaId(schemaId);
                          setExpectedSchemaGraphName(graphName);
                          // M-113: Use graph name for per-graph baselines
                          const endpoint = getExpectedSchemaEndpoint(graphName);
                          fetch(endpoint, {
                            method: 'PUT',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ schema_id: schemaId }),
                          }).catch(logError);
                        }}
                      />
                    </div>
                  )}
                </ErrorBoundary>
              </div>
              {/* Node Details Panel - M-455: Wrapped in ErrorBoundary */}
              <div style={{
                flex: 1,
                backgroundColor: colors.bg.primary,
                borderRadius: '8px',
                border: `1px solid ${colors.border.primary}`,
                minWidth: '280px',
                maxWidth: '350px'
              }}>
                <ErrorBoundary name="Node Details">
                  <NodeDetailsPanel
                    node={effectiveSchema?.nodes.find(n => n.name === selectedNode) || null}
                    execution={selectedNode ? effectiveNodeExecutions[selectedNode] : undefined}
                    currentState={selectedNodeState || effectiveGraphState}
                    previousState={selectedNodePreviousState || undefined}
                  />
                </ErrorBoundary>
              </div>
            </div>

            {/* Time-Travel Slider - M-455: Wrapped in ErrorBoundary */}
            <div style={{ marginTop: '12px', marginBottom: '8px' }}>
              <ErrorBoundary name="Timeline Slider">
                <TimelineSlider
                  runs={runs}
                  getRunStore={getRunStore}
                  cursor={runCursor}
                  isLive={isRunLive}
                  onCursorChange={setRunCursor}
                  onLiveModeChange={setRunLiveMode}
                  expectedSchemaId={expectedSchemaId}
                />
              </ErrorBoundary>
            </div>

            {/* Bottom section - Timeline and State Diff - M-455: Wrapped in ErrorBoundary */}
            <div style={{ display: 'flex', gap: '16px', flex: 1, minHeight: '200px', maxHeight: '250px' }}>
              {/* Execution Timeline */}
              <div style={{ flex: 1 }}>
                <ErrorBoundary name="Execution Timeline">
                  <ExecutionTimeline
                    events={timelineEvents}
                    startTime={executionStartTime}
                    maxHeight="200px"
                    expectedSchemaId={expectedSchemaId}
                    selectedIndex={timelineSelectedIndex}
                    onEventClick={setTimelineSelectedIndex}
                  />
                </ErrorBoundary>
              </div>
              {/* State Diff Viewer */}
              <div style={{ flex: 1 }}>
                <ErrorBoundary name="State Diff">
                  <StateDiffViewer
                    currentState={effectiveGraphState}
                    previousState={previousGraphState}
                    maxHeight="200px"
                  />
                </ErrorBoundary>
              </div>
            </div>
          </div>
        )}
      </main>

      {/* Footer - V-18: Enhanced link styling for better clickability */}
      <footer style={{
        marginTop: '24px',
        paddingTop: '16px',
        borderTop: `1px solid ${colors.border.primary}`,
        fontSize: '12px',
        color: colors.text.muted,
        display: 'flex',
        justifyContent: 'space-between',
        flexWrap: 'wrap',
        gap: '8px'
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
          {/* V-18: WebSocket URL displayed as code with copy hint */}
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
            <strong style={{ color: colors.text.tertiary }}>WebSocket:</strong>
            <code style={{
              backgroundColor: colors.alpha.white05,
              padding: '2px 6px',
              borderRadius: '4px',
              fontSize: '11px',
              color: colors.text.link
            }}>
              {getWebSocketUrl()}
            </code>
          </span>
          <span style={{ color: colors.border.secondary }}>|</span>
          {/* V-18: Metrics link with hover underline and external icon */}
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
            <strong style={{ color: colors.text.tertiary }}>Metrics:</strong>
            <a
              href="/metrics"
              target="_blank"
              rel="noopener noreferrer"
              style={{
                color: colors.text.link,
                textDecoration: 'none',
                cursor: 'pointer',
                display: 'inline-flex',
                alignItems: 'center',
                gap: '2px'
              }}
              onMouseEnter={(e) => (e.currentTarget.style.textDecoration = 'underline')}
              onMouseLeave={(e) => (e.currentTarget.style.textDecoration = 'none')}
            >
              /metrics <span style={{ fontSize: '10px' }}>↗</span>
            </a>
          </span>
          <span style={{ color: colors.border.secondary }}>|</span>
          {/* V-18: Health link with hover underline and external icon */}
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
            <strong style={{ color: colors.text.tertiary }}>Health:</strong>
            <a
              href="/health"
              target="_blank"
              rel="noopener noreferrer"
              style={{
                color: colors.text.link,
                textDecoration: 'none',
                cursor: 'pointer',
                display: 'inline-flex',
                alignItems: 'center',
                gap: '2px'
              }}
              onMouseEnter={(e) => (e.currentTarget.style.textDecoration = 'underline')}
              onMouseLeave={(e) => (e.currentTarget.style.textDecoration = 'none')}
            >
              /health <span style={{ fontSize: '10px' }}>↗</span>
            </a>
          </span>
        </div>
        <div style={{ color: colors.text.faint }}>
          DashFlow Observability v1.0
        </div>
      </footer>
    </div>
  );
}

export default App;
