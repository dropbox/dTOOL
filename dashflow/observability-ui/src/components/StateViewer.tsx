import { useEffect, useMemo, useCallback, useState } from 'react';
import { Tooltip } from './Tooltip';

interface StateViewerProps {
  state: Record<string, unknown>;
  previousState?: Record<string, unknown>;
  highlightChanges?: boolean;
  maxDepth?: number;
}

function getChangedKeys(
  current: Record<string, unknown>,
  previous?: Record<string, unknown>
): Set<string> {
  if (!previous) return new Set();
  const changed = new Set<string>();

  for (const key of Object.keys(current)) {
    if (!(key in previous)) {
      changed.add(key);
    } else if (JSON.stringify(current[key]) !== JSON.stringify(previous[key])) {
      changed.add(key);
    }
  }

  return changed;
}

function formatValue(value: unknown, depth: number = 0, maxDepth: number = 3): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (typeof value === 'string') {
    if (value.length > 100) {
      return `"${value.substring(0, 100)}..."`;
    }
    return `"${value}"`;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  if (Array.isArray(value)) {
    if (depth >= maxDepth) return `[Array(${value.length})]`;
    if (value.length === 0) return '[]';
    if (value.length > 3) {
      return `[${value.slice(0, 3).map(v => formatValue(v, depth + 1, maxDepth)).join(', ')}, ...]`;
    }
    return `[${value.map(v => formatValue(v, depth + 1, maxDepth)).join(', ')}]`;
  }
  if (typeof value === 'object') {
    if (depth >= maxDepth) return '{...}';
    const keys = Object.keys(value as object);
    if (keys.length === 0) return '{}';
    if (keys.length > 3) {
      const preview = keys.slice(0, 3).map(k => `${k}: ...`).join(', ');
      return `{${preview}, ...}`;
    }
    return '{...}';
  }
  return String(value);
}

function getPathDepth(path: string): number {
  return path.split('/').filter(Boolean).length;
}

function highlightMatch(text: string, searchTerm?: string) {
  if (!searchTerm) return text;
  const index = text.toLowerCase().indexOf(searchTerm.toLowerCase());
  if (index < 0) return text;
  const before = text.slice(0, index);
  const match = text.slice(index, index + searchTerm.length);
  const after = text.slice(index + searchTerm.length);
  return (
    <>
      {before}
      <span className="px-0.5 rounded bg-blue-500/20 text-blue-100 ring-1 ring-blue-500/30">
        {match}
      </span>
      {after}
    </>
  );
}

// SV-01: StateValue with dark theme for proper contrast (WCAG AA 4.5:1 ratio)
// SV-03: Added copy-on-click for individual values
function StateValue({
  name,
  value,
  isChanged,
  path,
  depth = 0,
  maxDepth = 3,
  expandedState,
  onToggleExpanded,
  forceExpanded,
  searchTerm,
  onToast,
}: {
  name: string;
  value: unknown;
  isChanged: boolean;
  path: string;
  depth?: number;
  maxDepth?: number;
  expandedState: Record<string, boolean>;
  onToggleExpanded: (path: string, next: boolean) => void;
  forceExpanded?: boolean;
  searchTerm?: string;
  onToast: (message: string) => void;
}) {
  const isExpandable = typeof value === 'object' && value !== null;
  const [copied, setCopied] = useState(false);

  // SV-02: Support forced expansion state
  const defaultExpanded = depth < 1;
  const isExpanded = expandedState[path] ?? defaultExpanded;
  const effectiveExpanded = forceExpanded !== undefined ? forceExpanded : isExpanded;

  const toggleExpand = () => {
    if (isExpandable && forceExpanded === undefined) {
      onToggleExpanded(path, !isExpanded);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.key === 'Enter' || e.key === ' ') && isExpandable) {
      e.preventDefault();
      toggleExpand();
    }
  };

  // SV-03: Copy value to clipboard
  const handleCopy = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    const jsonValue = JSON.stringify(value, null, 2);
    navigator.clipboard.writeText(jsonValue).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
      onToast('Copied value to clipboard');
    });
  }, [value, onToast]);

  // SV-04: Highlight search matches
  const nameMatches = searchTerm && name.toLowerCase().includes(searchTerm.toLowerCase());

  return (
    <div className={`${depth > 0 ? 'ml-4' : ''}`}>
      <div
        className={`
          flex items-start gap-2 py-1 rounded px-2 -mx-1 group
          ${isChanged ? 'bg-yellow-900/30' : ''}
          ${nameMatches ? 'bg-blue-900/30 ring-1 ring-blue-500/50' : ''}
          ${isExpandable ? 'cursor-pointer hover:bg-gray-700/50 focus:outline-none focus:ring-2 focus:ring-blue-500/50' : ''}
        `}
        onClick={toggleExpand}
        onKeyDown={handleKeyDown}
        tabIndex={isExpandable ? 0 : undefined}
        role={isExpandable ? 'button' : undefined}
        aria-expanded={isExpandable ? effectiveExpanded : undefined}
      >
        {isExpandable && (
          <span className="text-gray-400 text-xs mt-0.5 w-3">
            {effectiveExpanded ? '▼' : '▶'}
          </span>
        )}
        {!isExpandable && <span className="w-3" />}
        <span className={`font-medium ${isChanged ? 'text-yellow-400' : nameMatches ? 'text-blue-400' : 'text-cyan-400'}`}>
          {highlightMatch(`${name}:`, searchTerm)}
        </span>
        {!effectiveExpanded || !isExpandable ? (
          <span
            className="text-gray-300 font-mono text-xs flex-1 truncate"
            title={
              typeof value === 'string'
                ? value.length > 100
                  ? `${value.substring(0, 100)}...`
                  : undefined
                : typeof value === 'object' && value !== null
                  ? JSON.stringify(value, null, 2).slice(0, 500)
                  : undefined
            }
          >
            {formatValue(value, depth, maxDepth)}
          </span>
        ) : null}
        {isChanged && (
          <span className="text-yellow-400 text-xs">●</span>
        )}
        {/* SV-03: Copy button on hover */}
        <button
          type="button"
          onClick={handleCopy}
          className="opacity-0 group-hover:opacity-100 text-gray-400 hover:text-gray-300 transition-opacity text-xs px-1"
          title="Copy value as JSON"
        >
          {copied ? '✓' : '📋'}
        </button>
      </div>

      {effectiveExpanded && isExpandable && depth < maxDepth && (
        <div className="border-l border-gray-600 ml-1.5">
          {Array.isArray(value) ? (
            value.slice(0, 10).map((item, index) => (
              <StateValue
                key={index}
                name={`[${index}]`}
                value={item}
                isChanged={false}
                path={`${path}/${index}`}
                depth={depth + 1}
                maxDepth={maxDepth}
                expandedState={expandedState}
                onToggleExpanded={onToggleExpanded}
                forceExpanded={forceExpanded}
                searchTerm={searchTerm}
                onToast={onToast}
              />
            ))
          ) : (
            Object.entries(value as Record<string, unknown>).slice(0, 10).map(([key, val]) => (
              <StateValue
                key={key}
                name={key}
                value={val}
                isChanged={false}
                path={`${path}/${key}`}
                depth={depth + 1}
                maxDepth={maxDepth}
                expandedState={expandedState}
                onToggleExpanded={onToggleExpanded}
                forceExpanded={forceExpanded}
                searchTerm={searchTerm}
                onToast={onToast}
              />
            ))
          )}
          {(Array.isArray(value) ? value.length : Object.keys(value as object).length) > 10 && (
            <div className="ml-4 text-gray-400 text-xs">
              ... and {(Array.isArray(value) ? value.length : Object.keys(value as object).length) - 10} more
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function StateViewer({
  state,
  previousState,
  highlightChanges = false,
  maxDepth = 3,
}: StateViewerProps) {
  const localStorageKey = 'dashflow:stateViewer:expandedState:v1';
  // SV-02: Expand/collapse all state
  const [expandAll, setExpandAll] = useState<boolean | undefined>(undefined);
  // SV-04: Search/filter state
  const [searchTerm, setSearchTerm] = useState('');
  // SV-03: Copy all feedback
  const [copiedAll, setCopiedAll] = useState(false);
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  const [expandedState, setExpandedState] = useState<Record<string, boolean>>(() => {
    if (typeof window === 'undefined') return {};
    try {
      const raw = window.localStorage.getItem(localStorageKey);
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== 'object') return {};
      return parsed as Record<string, boolean>;
    } catch {
      return {};
    }
  });

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(localStorageKey, JSON.stringify(expandedState));
    } catch {
      // ignore localStorage failures (private mode, quota, etc.)
    }
  }, [expandedState]);

  const showToast = useCallback((message: string) => {
    setToastMessage(message);
    setTimeout(() => setToastMessage(null), 1500);
  }, []);

  const changedKeys = useMemo(
    () => (highlightChanges ? getChangedKeys(state, previousState) : new Set<string>()),
    [state, previousState, highlightChanges]
  );

  const entries = Object.entries(state);

  // SV-04: Filter entries by search term
  const filteredEntries = useMemo(() => {
    if (!searchTerm) return entries;
    const term = searchTerm.toLowerCase();
    return entries.filter(([key, value]) => {
      // Match key name
      if (key.toLowerCase().includes(term)) return true;
      // Match string values
      if (typeof value === 'string' && value.toLowerCase().includes(term)) return true;
      // Match in nested object keys
      if (typeof value === 'object' && value !== null) {
        return JSON.stringify(value).toLowerCase().includes(term);
      }
      return false;
    });
  }, [entries, searchTerm]);

  // SV-03: Copy all state as JSON
  const handleCopyAll = useCallback(() => {
    const jsonState = JSON.stringify(state, null, 2);
    navigator.clipboard.writeText(jsonState).then(() => {
      setCopiedAll(true);
      setTimeout(() => setCopiedAll(false), 1500);
      showToast('Copied full state to clipboard');
    });
  }, [state, showToast]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.target && (e.target as HTMLElement).tagName === 'INPUT') return;

    const key = e.key.toLowerCase();
    const toggle = (e.ctrlKey || e.metaKey) && key === 'e';
    if (!toggle) return;

    e.preventDefault();
    setExpandAll((prev) => {
      if (prev === true) return false;
      if (prev === false) return true;
      return true;
    });
  }, []);

  const expandedDepth = useMemo(() => {
    if (expandAll === true) return maxDepth;
    if (expandAll === false) return 0;

    let maxSeen = 1; // Root keys are expanded by default at depth < 1
    for (const [path, expanded] of Object.entries(expandedState)) {
      if (!expanded) continue;
      maxSeen = Math.max(maxSeen, getPathDepth(path));
    }
    return Math.min(maxDepth, maxSeen);
  }, [expandedState, expandAll, maxDepth]);

  const handleToggleExpanded = useCallback((path: string, next: boolean) => {
    setExpandedState((prev) => {
      const nextState = { ...prev };
      nextState[path] = next;
      return nextState;
    });
  }, []);

  // SV-01: Dark theme container for proper contrast
  if (entries.length === 0) {
    return (
      <div className="text-gray-400 text-sm italic bg-gray-900 rounded p-3 border border-gray-700">
        Empty state
      </div>
    );
  }

  return (
    <div
      className="relative bg-gray-900 rounded border border-gray-700 overflow-hidden focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60"
      tabIndex={0}
      onKeyDown={handleKeyDown}
      aria-label="State viewer. Use Ctrl+E (or Cmd+E) to toggle expand/collapse all."
    >
      {/* SV-02, SV-03, SV-04: Header with controls */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-gray-700 bg-gray-800/50">
        {/* SV-04: Search input */}
        <div className="flex-1 relative">
          <input
            type="text"
            placeholder="Search keys..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full bg-gray-800 border border-gray-600 rounded pl-2 pr-7 py-1 text-xs text-gray-300 placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          {searchTerm && (
            <Tooltip content="Clear search">
              <button
                type="button"
                onClick={() => setSearchTerm('')}
                className="absolute right-1 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-300 px-1 py-0.5 rounded hover:bg-gray-700 focus:outline-none focus-visible:ring-1 focus-visible:ring-blue-500/60"
                aria-label="Clear search"
              >
                ×
              </button>
            </Tooltip>
          )}
        </div>
        {searchTerm && (
          <span className="text-xs text-gray-400">
            {filteredEntries.length}/{entries.length}
          </span>
        )}
        <span className="text-xs text-gray-400">
          Depth: {expandedDepth}/{maxDepth}
        </span>
        {/* SV-02: Expand/Collapse buttons - TC-08: Using Tooltip component */}
        <Tooltip content="Expand all nested keys (Ctrl+E)">
          <button
            type="button"
            onClick={() => setExpandAll(true)}
            className="text-xs text-gray-400 hover:text-gray-200 px-2 py-1 rounded hover:bg-gray-700"
          >
            ⊞
          </button>
        </Tooltip>
        <Tooltip content="Collapse all nested keys">
          <button
            type="button"
            onClick={() => setExpandAll(false)}
            className="text-xs text-gray-400 hover:text-gray-200 px-2 py-1 rounded hover:bg-gray-700"
          >
            ⊟
          </button>
        </Tooltip>
        <Tooltip content="Reset to default expansion">
          <button
            type="button"
            onClick={() => setExpandAll(undefined)}
            className="text-xs text-gray-400 hover:text-gray-200 px-2 py-1 rounded hover:bg-gray-700"
          >
            ↺
          </button>
        </Tooltip>
        {/* SV-03: Copy all button - TC-08: Using Tooltip component */}
        <Tooltip content="Copy all state as formatted JSON">
          <button
            type="button"
            onClick={handleCopyAll}
            className="text-xs text-gray-400 hover:text-gray-200 px-2 py-1 rounded hover:bg-gray-700"
          >
            {copiedAll ? '✓' : '📋'}
          </button>
        </Tooltip>
      </div>

        {/* State content */}
      <div className="font-mono text-sm p-3 max-h-64 overflow-y-auto">
        {filteredEntries.length === 0 ? (
          <div className="text-gray-400 text-center py-4">
            No keys match "{searchTerm}"
          </div>
        ) : (
          filteredEntries.map(([key, value]) => (
            <StateValue
              key={key}
              name={key}
              value={value}
              isChanged={changedKeys.has(key)}
              path={`/${key}`}
              maxDepth={maxDepth}
              expandedState={expandedState}
              onToggleExpanded={handleToggleExpanded}
              forceExpanded={expandAll}
              searchTerm={searchTerm}
              onToast={showToast}
            />
          ))
        )}
      </div>

      {/* SV-03: Simple toast feedback */}
      {toastMessage && (
        <div className="absolute bottom-3 right-3 bg-gray-800 text-gray-200 text-xs px-3 py-1.5 rounded shadow-lg ring-1 ring-white/10">
          {toastMessage}
        </div>
      )}
    </div>
  );
}

export default StateViewer;
