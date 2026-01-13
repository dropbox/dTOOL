// Timeline slider for time-travel debugging
// Slider with session markers and schema segments

import { useCallback, useMemo } from 'react';
import { RunCursor, RunStateStore } from '../hooks/useRunStateStore';
import { EventType } from '../proto/dashstream';
import { colors, spacing, borderRadius, fontSize, shadows, durations } from '../styles/tokens';
import { Tooltip } from './Tooltip';

interface TimelineSliderProps {
  // Available runs
  runs: string[];
  getRunStore: (threadId: string) => RunStateStore | undefined;

  // Current cursor state
  cursor: RunCursor | null;
  isLive: boolean;

  // Cursor controls
  onCursorChange: (cursor: RunCursor) => void;
  onLiveModeChange: (live: boolean) => void;

  // Expected schema for mismatch detection
  expectedSchemaId?: string;
}

// Format timestamp for display
function formatTime(timestamp: number, baseTime: number): string {
  const elapsed = timestamp - baseTime;
  const seconds = elapsed / 1000;
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${String(minutes).padStart(2, '0')}:${secs.toFixed(1).padStart(4, '0')}`;
}

// Get status color (TC-04: using design tokens)
function getStatusColor(status: 'running' | 'completed' | 'error'): string {
  switch (status) {
    case 'running':
      return colors.status.info;
    case 'completed':
      return colors.status.success;
    case 'error':
      return colors.status.error;
    default:
      return colors.status.neutralDark;
  }
}

export function TimelineSlider({
  runs,
  getRunStore,
  cursor,
  isLive,
  onCursorChange,
  onLiveModeChange,
  expectedSchemaId,
}: TimelineSliderProps) {
  const controlBaseClassName =
    'rounded border text-xs transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60 disabled:opacity-60 disabled:cursor-not-allowed disabled:bg-gray-800 disabled:border-gray-700 disabled:text-gray-500';
  const stepButtonClassName = `${controlBaseClassName} bg-gray-800 border-gray-700 text-gray-200 px-2 py-1 hover:bg-gray-700 hover:border-gray-600 active:bg-gray-600 disabled:hover:bg-gray-800 disabled:hover:border-gray-700`;
  const liveToggleClassName = `${controlBaseClassName} flex items-center gap-1.5 px-3 py-1 font-medium ${
    isLive
      ? 'bg-red-500/20 border-red-500/60 text-red-300 hover:bg-red-500/25'
      : 'bg-gray-800 border-gray-700 text-gray-200 hover:bg-gray-700 hover:border-gray-600 active:bg-gray-600'
  }`;

  // Get current run store
  const currentStore = useMemo(() => {
    if (!cursor) return undefined;
    return getRunStore(cursor.threadId);
  }, [cursor, getRunStore]);

  // Sort runs by recency and compute display info
  const sortedRunsWithInfo = useMemo(() => {
    return runs
      .map(threadId => {
        const store = getRunStore(threadId);
        return {
          threadId,
          store,
          startTime: store?.startTime ?? 0,
          endTime: store?.endTime,
        };
      })
      .sort((a, b) => b.startTime - a.startTime) // Most recent first
      .map(({ threadId, store, startTime, endTime }) => {
        // Format time range for display
        const startDate = new Date(startTime);
        const timeStr = startDate.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });

        let durationStr = '';
        if (store?.status === 'running') {
          durationStr = ' (running)';
        } else if (endTime && startTime) {
          const durationMs = endTime - startTime;
          if (durationMs < 1000) {
            durationStr = ` (${durationMs}ms)`;
          } else if (durationMs < 60000) {
            durationStr = ` (${(durationMs / 1000).toFixed(1)}s)`;
          } else {
            durationStr = ` (${(durationMs / 60000).toFixed(1)}m)`;
          }
        }

        const corrupted = store?.corrupted ? ' [!]' : '';

        return {
          threadId,
          store,
          label: `${store?.graphName || threadId} @ ${timeStr}${durationStr}${corrupted}`,
        };
      });
  }, [runs, getRunStore]);

  // Build event markers for slider (includes NodeStart/NodeEnd for time-travel visualization)
  // M-693: seq is string to prevent precision loss for values > 2^53
  const eventMarkers = useMemo(() => {
    if (!currentStore) return [];

    const markers: Array<{
      seq: string;
      timestamp: number;
      type: 'session' | 'schema' | 'node_start' | 'node_end' | 'error';
      label: string;
    }> = [];

    // Track last schemaId to detect schema changes
    let lastSchemaId: string | undefined;

    for (const event of currentStore.events) {
      if (event.eventType === EventType.EVENT_TYPE_GRAPH_START) {
        markers.push({
          seq: event.seq,
          timestamp: event.timestamp,
          type: 'session',
          label: 'Graph Start',
        });
        // Check for schema_id in attributes for schema change detection
        const schemaId = event.attributes?.schema_id as string | undefined;
        if (schemaId && lastSchemaId && schemaId !== lastSchemaId) {
          markers.push({
            seq: event.seq,
            timestamp: event.timestamp,
            type: 'schema',
            label: 'Schema Changed',
          });
        }
        lastSchemaId = schemaId;
      } else if (event.eventType === EventType.EVENT_TYPE_GRAPH_END) {
        markers.push({
          seq: event.seq,
          timestamp: event.timestamp,
          type: 'session',
          label: 'Graph End',
        });
      } else if (event.eventType === EventType.EVENT_TYPE_NODE_START) {
        // Add NodeStart markers
        markers.push({
          seq: event.seq,
          timestamp: event.timestamp,
          type: 'node_start',
          label: event.nodeId ? `${event.nodeId} (start)` : 'Node Start',
        });
      } else if (event.eventType === EventType.EVENT_TYPE_NODE_END) {
        // Add NodeEnd markers
        markers.push({
          seq: event.seq,
          timestamp: event.timestamp,
          type: 'node_end',
          label: event.nodeId ? `${event.nodeId} (end)` : 'Node End',
        });
      } else if (event.eventType === EventType.EVENT_TYPE_NODE_ERROR) {
        markers.push({
          seq: event.seq,
          timestamp: event.timestamp,
          type: 'error',
          label: event.nodeId || 'Error',
        });
      }
    }

    return markers;
  }, [currentStore]);

  // Slider range
  // M-693: seq is string; we use event index for slider range for precise UI positioning
  // This avoids precision issues with large sequences and works better with HTML input range
  const sliderRange = useMemo(() => {
    if (!currentStore || currentStore.events.length === 0) {
      return { min: 0, max: 100, value: 0 };
    }

    const events = currentStore.events;
    // Use index-based slider: min=0, max=events.length-1
    const min = 0;
    const max = events.length - 1;

    // Find current cursor's index in the events array
    let value = max; // Default to latest
    if (cursor?.seq) {
      const cursorIndex = events.findIndex(e => e.seq === cursor.seq);
      if (cursorIndex >= 0) {
        value = cursorIndex;
      } else {
        // Cursor seq not found, use max (live)
        value = max;
      }
    }

    return { min, max, value };
  }, [currentStore, cursor]);

  // Handle run selection
  const handleRunSelect = useCallback((e: React.ChangeEvent<HTMLSelectElement>) => {
    const threadId = e.target.value;
    const store = getRunStore(threadId);
    if (store && store.events.length > 0) {
      const latestEvent = store.events[store.events.length - 1];
      onCursorChange({ threadId, seq: latestEvent.seq });
      onLiveModeChange(store.status === 'running');
    }
  }, [getRunStore, onCursorChange, onLiveModeChange]);

  // Handle slider change
  // M-693: Slider uses index; convert index to seq string
  const handleSliderChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (!cursor || !currentStore) return;

    const index = parseInt(e.target.value, 10);
    const events = currentStore.events;
    if (index >= 0 && index < events.length) {
      const seq = events[index].seq;
      onCursorChange({ threadId: cursor.threadId, seq });
      onLiveModeChange(false);
    }
  }, [cursor, currentStore, onCursorChange, onLiveModeChange]);

  // Handle live toggle
  const handleLiveToggle = useCallback(() => {
    onLiveModeChange(!isLive);
  }, [isLive, onLiveModeChange]);

  // Step forward/backward
  const handleStep = useCallback((direction: 'back' | 'forward') => {
    if (!cursor || !currentStore) return;

    const events = currentStore.events;
    const currentIndex = events.findIndex(e => e.seq === cursor.seq);
    if (currentIndex === -1) return;

    const newIndex = direction === 'forward'
      ? Math.min(currentIndex + 1, events.length - 1)
      : Math.max(currentIndex - 1, 0);

    onCursorChange({ threadId: cursor.threadId, seq: events[newIndex].seq });
    onLiveModeChange(false);
  }, [cursor, currentStore, onCursorChange, onLiveModeChange]);

  // Jump to start/end
  const handleJump = useCallback((target: 'start' | 'end') => {
    if (!cursor || !currentStore) return;

    const events = currentStore.events;
    if (events.length === 0) return;

    const targetEvent = target === 'start' ? events[0] : events[events.length - 1];
    onCursorChange({ threadId: cursor.threadId, seq: targetEvent.seq });
    onLiveModeChange(target === 'end' && currentStore.status === 'running');
  }, [cursor, currentStore, onCursorChange, onLiveModeChange]);

  // Check for schema mismatch
  const hasSchemaMismatch = useMemo(() => {
    if (!expectedSchemaId || !currentStore?.schemaId) return false;
    return expectedSchemaId !== currentStore.schemaId;
  }, [expectedSchemaId, currentStore]);

  // Current event info
  const currentEvent = useMemo(() => {
    if (!cursor || !currentStore) return null;
    return currentStore.events.find(e => e.seq === cursor.seq);
  }, [cursor, currentStore]);

  return (
    <div
      data-testid="timeline-slider"
      style={{
      backgroundColor: colors.bg.primary,
      borderRadius: borderRadius.lg,
      border: `1px solid ${colors.border.primary}`,
      padding: spacing[3],
    }}
    >
      {/* Top bar: Run selector + schema info + live toggle */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: spacing[3],
        marginBottom: spacing[3],
      }}>
        {/* I-10: Enhanced run selector with context and tooltip */}
        <div style={{ display: 'flex', alignItems: 'center', gap: spacing[2] }}>
          <label
            style={{ fontSize: fontSize.base, color: colors.text.muted, fontWeight: 500, cursor: 'help' }}
            title="Select an execution trace to view. Each run represents a complete graph execution."
          >
            EXECUTION:
          </label>
          <select
            data-testid="run-selector"
            value={cursor?.threadId || ''}
            onChange={handleRunSelect}
            disabled={sortedRunsWithInfo.length === 0}
            className={`${controlBaseClassName} bg-gray-900 border-gray-700 text-gray-200 px-2.5 py-1.5 min-w-[240px] hover:border-gray-600`}
            title="Select an execution run to inspect"
          >
            {/* I-10: Better placeholder with context */}
            {sortedRunsWithInfo.length === 0 && (
              <option value="">No execution traces yet...</option>
            )}
            {sortedRunsWithInfo.length > 0 && !cursor?.threadId && (
              <option value="" disabled>Select execution run...</option>
            )}
            {sortedRunsWithInfo.map(({ threadId, label }) => (
              <option key={threadId} value={threadId}>
                {label}
              </option>
            ))}
          </select>
          {/* I-10: Run count indicator */}
          {sortedRunsWithInfo.length > 0 && (
            <span style={{
              fontSize: fontSize.xs,
              color: colors.text.faint,
              backgroundColor: colors.bg.overlay,
              padding: `2px ${spacing[1]}`,
              borderRadius: borderRadius.md,
            }}>
              {sortedRunsWithInfo.length} run{sortedRunsWithInfo.length !== 1 ? 's' : ''}
            </span>
          )}
        </div>

        {/* Status badge */}
        {currentStore && (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: spacing[1],
            backgroundColor: colors.bg.secondary,
            padding: `${spacing[1]} ${spacing[2]}`,
            borderRadius: borderRadius.md,
            fontSize: fontSize.base,
          }}>
            <div style={{
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              backgroundColor: getStatusColor(currentStore.status),
            }} />
            <span style={{ color: colors.text.muted, textTransform: 'uppercase' }}>
              {currentStore.status}
            </span>
          </div>
        )}

        {/* Schema ID badge */}
        {currentStore?.schemaId && (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: spacing[1],
            backgroundColor: hasSchemaMismatch ? 'rgba(239, 68, 68, 0.2)' : colors.bg.secondary,
            border: hasSchemaMismatch ? `1px solid ${colors.status.error}` : '1px solid transparent',
            padding: `${spacing[1]} ${spacing[2]}`,
            borderRadius: borderRadius.md,
            fontSize: fontSize.sm,
            fontFamily: 'monospace',
          }}>
            <span style={{ color: colors.text.muted }}>SCHEMA:</span>
            <span style={{ color: hasSchemaMismatch ? colors.status.error : colors.accent.cyan }}>
              {currentStore.schemaId.slice(0, 8)}...
            </span>
            {hasSchemaMismatch && (
              <span style={{ color: colors.status.error, fontWeight: 500 }}>MISMATCH</span>
            )}
          </div>
        )}

        {/* Spacer */}
        <div style={{ flex: 1 }} />

        {/* Live toggle */}
        <button
          type="button"
          onClick={handleLiveToggle}
          className={liveToggleClassName}
        >
          <div style={{
            width: '8px',
            height: '8px',
            borderRadius: '50%',
            backgroundColor: isLive ? colors.status.error : colors.text.faint,
            animation: isLive ? 'pulse 1.5s infinite' : 'none',
          }} />
          {isLive ? 'LIVE' : 'PAUSED'}
        </button>
      </div>

      {/* Schema mismatch banner */}
      {hasSchemaMismatch && (
        <div style={{
          backgroundColor: colors.statusBg.error,
          border: `1px solid ${colors.status.error}`,
          borderRadius: borderRadius.md,
          padding: `${spacing[2]} ${spacing[3]}`,
          marginBottom: spacing[3],
          display: 'flex',
          alignItems: 'center',
          gap: spacing[2],
          fontSize: fontSize.base,
        }}>
          <span style={{ color: colors.status.error, fontWeight: 600 }}>WARNING:</span>
          <span style={{ color: colors.accent.lightRed }}>
            Schema mismatch detected. Expected: {expectedSchemaId?.slice(0, 8)}..., Actual: {currentStore?.schemaId?.slice(0, 8)}...
          </span>
        </div>
      )}

      {/* Timeline slider */}
      <div style={{ marginBottom: spacing[2] }}>
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: spacing[2],
        }}>
          {/* Step back buttons - TC-08: Using Tooltip component */}
          <Tooltip content="Jump to first event in timeline">
            <button
              type="button"
              onClick={() => handleJump('start')}
              disabled={!currentStore || sliderRange.value === sliderRange.min}
              className={stepButtonClassName}
            >
              |&lt;
            </button>
          </Tooltip>
          <Tooltip content="Step to previous event">
            <button
              type="button"
              onClick={() => handleStep('back')}
              disabled={!currentStore || sliderRange.value === sliderRange.min}
              className={stepButtonClassName}
            >
              &lt;
            </button>
          </Tooltip>

          {/* Slider */}
          <div style={{ flex: 1, position: 'relative' }}>
            <input
              type="range"
              min={sliderRange.min}
              max={sliderRange.max}
              value={sliderRange.value}
              onChange={handleSliderChange}
              disabled={!currentStore || currentStore.events.length === 0}
              data-testid="timeline-range"
              aria-label="Timeline position"
              className="timeline-range disabled:opacity-50"
            />
            {/* Event markers with NodeStart/NodeEnd and schema change indicators */}
            {/* M-693: Use index-based positioning since we now use index for slider range */}
            {eventMarkers.map((marker, i) => {
              const denominator = sliderRange.max - sliderRange.min;
              // Find marker's index in events array for position calculation
              const markerIndex = currentStore?.events.findIndex(e => e.seq === marker.seq) ?? 0;
              const position = denominator > 0
                ? Math.min(100, Math.max(0, ((markerIndex - sliderRange.min) / denominator) * 100))
                : 0;
              // Different colors and shapes for different marker types (TC-04: using design tokens)
              const markerStyle = {
                session: { color: colors.accent.cyan, size: 6, shape: 'diamond' },    // Cyan diamond for session start/end
                schema: { color: colors.status.warning, size: 8, shape: 'triangle' }, // Amber triangle for schema changes
                node_start: { color: colors.status.success, size: 4, shape: 'circle' },  // Green for node start
                node_end: { color: colors.status.info, size: 4, shape: 'circle' },    // Blue for node end
                error: { color: colors.status.error, size: 6, shape: 'circle' },      // Red for errors
              }[marker.type];

              const { color, size, shape } = markerStyle;

              // Generate border-radius based on shape
              // Use distinct name to avoid shadowing imported borderRadius token
              const markerBorderRadius = shape === 'circle' ? '50%' :
                                         shape === 'diamond' ? '2px' :
                                         '0';
              // For diamond, rotate 45deg
              const transform = shape === 'diamond'
                ? 'translateX(-50%) rotate(45deg)'
                : shape === 'triangle'
                ? 'translateX(-50%)'
                : 'translateX(-50%)';

              // Triangle needs special CSS (border trick)
              if (shape === 'triangle') {
                return (
                  <div
                    key={i}
                    className="event-marker"
                    data-testid="event-marker"
                    style={{
                      position: 'absolute',
                      left: `${position}%`,
                      top: '-6px',
                      width: 0,
                      height: 0,
                      borderLeft: `${size / 2}px solid transparent`,
                      borderRight: `${size / 2}px solid transparent`,
                      borderBottom: `${size}px solid ${color}`,
                      transform: 'translateX(-50%)',
                      pointerEvents: 'none',
                    }}
                    title={marker.label}
                  />
                );
              }

              return (
                <div
                  key={i}
                  className="event-marker"
                  data-testid="event-marker"
                  style={{
                    position: 'absolute',
                    left: `${position}%`,
                    top: '-4px',
                    width: `${size}px`,
                    height: `${size}px`,
                    backgroundColor: color,
                    borderRadius: markerBorderRadius,
                    transform,
                    pointerEvents: 'none',
                  }}
                  title={marker.label}
                />
              );
            })}
          </div>

          {/* Step forward buttons - TC-08: Using Tooltip component */}
          <Tooltip content="Step to next event">
            <button
              type="button"
              onClick={() => handleStep('forward')}
              disabled={!currentStore || sliderRange.value === sliderRange.max}
              className={stepButtonClassName}
            >
              &gt;
            </button>
          </Tooltip>
          <Tooltip content="Jump to last event in timeline">
            <button
              type="button"
              onClick={() => handleJump('end')}
              disabled={!currentStore || sliderRange.value === sliderRange.max}
              className={stepButtonClassName}
            >
              &gt;|
            </button>
          </Tooltip>
        </div>
      </div>

      {/* Current position info */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        fontSize: fontSize.sm,
        color: colors.text.faint,
      }}>
        <span>
          {currentStore && cursor ? (
            <>
              {/* M-752: Display index-based position (1-indexed for user display), not raw seq string */}
              Event {sliderRange.value + 1} of {sliderRange.max + 1}
              {currentEvent?.nodeId && ` - ${currentEvent.nodeId}`}
            </>
          ) : (
            'No events'
          )}
        </span>
        <span>
          {currentStore && currentEvent ? (
            formatTime(currentEvent.timestamp, currentStore.startTime)
          ) : (
            '00:00.0'
          )}
        </span>
      </div>

      {/* CSS animation for live pulse */}
      {/* TC-04: Using design tokens for slider colors */}
      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }

        /* TC-03: Style native range slider for consistent dark theme */
        .timeline-range {
          width: 100%;
          height: 6px;
          background: transparent;
          -webkit-appearance: none;
          appearance: none;
          cursor: pointer;
        }

        .timeline-range:disabled {
          cursor: not-allowed;
        }

        .timeline-range::-webkit-slider-runnable-track {
          height: 6px;
          background: ${colors.bg.surface};
          border: 1px solid ${colors.border.primary};
          border-radius: ${borderRadius.full};
        }

        .timeline-range::-webkit-slider-thumb {
          -webkit-appearance: none;
          appearance: none;
          margin-top: -6px;
          width: 16px;
          height: 16px;
          border-radius: ${borderRadius.full};
          background: ${colors.status.info};
          border: 2px solid ${colors.bg.slider};
          box-shadow: ${shadows.thumbGlow};
          transition: transform ${durations.fast} ease, background-color ${durations.normal} ease, box-shadow ${durations.normal} ease;
        }

        .timeline-range:hover:not(:disabled)::-webkit-slider-thumb {
          background: ${colors.status.infoHover};
        }

        .timeline-range:active:not(:disabled)::-webkit-slider-thumb {
          transform: scale(1.05);
        }

        .timeline-range:focus-visible::-webkit-slider-thumb {
          box-shadow: ${shadows.focusLarge};
        }

        .timeline-range::-moz-range-track {
          height: 6px;
          background: ${colors.bg.surface};
          border: 1px solid ${colors.border.primary};
          border-radius: ${borderRadius.full};
        }

        .timeline-range::-moz-range-thumb {
          width: 16px;
          height: 16px;
          border-radius: ${borderRadius.full};
          background: ${colors.status.info};
          border: 2px solid ${colors.bg.slider};
          box-shadow: ${shadows.thumbGlow};
          transition: transform ${durations.fast} ease, background-color ${durations.normal} ease, box-shadow ${durations.normal} ease;
        }

        .timeline-range:hover:not(:disabled)::-moz-range-thumb {
          background: ${colors.status.infoHover};
        }

        .timeline-range:active:not(:disabled)::-moz-range-thumb {
          transform: scale(1.05);
        }

        .timeline-range:focus-visible::-moz-range-thumb {
          box-shadow: ${shadows.focusLarge};
        }
      `}</style>
    </div>
  );
}

export default TimelineSlider;
