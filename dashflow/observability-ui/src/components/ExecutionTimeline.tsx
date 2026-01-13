import { useCallback, useEffect, useRef } from 'react';
import { FixedSizeList as List, ListChildComponentProps } from 'react-window';
import { colors, spacing, borderRadius, fontSize } from '../styles/tokens';

// M-471: Virtualization threshold - render directly below this, virtualize above
const VIRTUALIZATION_THRESHOLD = 50;
// Height of each event row in pixels (must be fixed for react-window)
const EVENT_ROW_HEIGHT = 42;

export interface TimelineEvent {
  timestamp: number;
  elapsed_ms: number;
  event_type: string;
  node_id?: string;
  details?: string;
  // Schema ID for out-of-schema highlighting (events from different schemas get highlighted)
  schema_id?: string;
}

interface ExecutionTimelineProps {
  events: TimelineEvent[];
  startTime: number | null;
  maxHeight?: string;
  // Expected schema ID for out-of-schema highlighting
  expectedSchemaId?: string;
  // Time-travel: selected event index for graph state visualization
  selectedIndex?: number;
  // Callback when user clicks an event to jump to that point in time
  onEventClick?: (index: number) => void;
}

// Event type colors and symbols (TC-04: using design tokens)
const EVENT_STYLES: Record<string, { color: string; symbol: string; bgColor: string }> = {
  GraphStart: { color: colors.status.info, symbol: '>', bgColor: colors.statusBg.info },
  GraphEnd: { color: colors.status.info, symbol: '<', bgColor: colors.statusBg.info },
  NodeStart: { color: colors.status.warning, symbol: '\u25b6', bgColor: colors.statusBg.warning },
  NodeEnd: { color: colors.status.success, symbol: '\u25c0', bgColor: colors.statusBg.success },
  NodeError: { color: colors.status.error, symbol: '\u2716', bgColor: colors.statusBg.error },
  StateUpdate: { color: colors.accent.purple, symbol: '\u2022', bgColor: colors.statusBg.purple },
  default: { color: colors.status.neutral, symbol: '\u2022', bgColor: colors.statusBg.neutral },
};

function formatElapsedTime(ms: number): string {
  const seconds = ms / 1000;
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${String(minutes).padStart(2, '0')}:${secs.toFixed(1).padStart(4, '0')}`;
}

// M-471: Virtualized row renderer using react-window
interface RowData {
  events: TimelineEvent[];
  startTime: number | null;
  expectedSchemaId?: string;
  selectedIndex?: number;
  onEventClick?: (index: number) => void;
}

function EventRow({ index, style, data }: ListChildComponentProps<RowData>) {
  const { events, startTime, expectedSchemaId, selectedIndex, onEventClick } = data;
  const event = events[index];
  const eventStyle = EVENT_STYLES[event.event_type] || EVENT_STYLES.default;
  const elapsedMs = event.elapsed_ms ??
    (startTime != null ? event.timestamp - startTime : 0);
  const elapsedStr = formatElapsedTime(elapsedMs);
  const isOutOfSchema = expectedSchemaId && event.schema_id && event.schema_id !== expectedSchemaId;
  const isSelected = selectedIndex === index;
  const isClickable = !!onEventClick;

  return (
    <div
      onClick={isClickable ? () => onEventClick(index) : undefined}
      style={{
        ...style,
        display: 'flex',
        alignItems: 'center',
        gap: spacing[2],
        padding: `${spacing[1]} ${spacing[2]}`,
        backgroundColor: isSelected
          ? 'rgba(59, 130, 246, 0.25)'
          : isOutOfSchema ? colors.statusBg.errorStrong : eventStyle.bgColor,
        borderRadius: borderRadius.md,
        borderLeft: `3px solid ${isSelected ? colors.status.info : isOutOfSchema ? colors.status.error : eventStyle.color}`,
        fontSize: fontSize.base,
        margin: `0 ${spacing[2]} ${spacing[1]} ${spacing[2]}`,
        boxSizing: 'border-box',
        height: EVENT_ROW_HEIGHT - 4, // Account for margin
        cursor: isClickable ? 'pointer' : 'default',
        transition: 'background-color 0.15s ease',
        ...(isOutOfSchema ? { boxShadow: 'inset 0 0 0 1px rgba(239, 68, 68, 0.3)' } : {}),
        ...(isSelected ? { boxShadow: 'inset 0 0 0 1px rgba(59, 130, 246, 0.5)' } : {}),
      }}
    >
      {isOutOfSchema && (
        <span
          title={`Schema mismatch: expected ${expectedSchemaId?.slice(0, 8)}..., got ${event.schema_id?.slice(0, 8)}...`}
          style={{
            color: colors.status.error,
            fontSize: fontSize.xs,
            fontWeight: 700,
            padding: `1px ${spacing[1]}`,
            backgroundColor: 'rgba(239, 68, 68, 0.2)',
            borderRadius: borderRadius.sm,
            marginRight: spacing[1],
          }}
        >
          !
        </span>
      )}
      <span style={{
        fontFamily: "'SF Mono', 'Monaco', 'Inconsolata', monospace",
        color: isOutOfSchema ? colors.status.error : colors.text.faint,
        minWidth: '60px'
      }}>
        {elapsedStr}
      </span>
      {event.node_id && (
        <span
          style={{
            color: isOutOfSchema ? colors.accent.lightRed : colors.accent.cyan,
            fontWeight: 500,
            minWidth: '80px',
            maxWidth: '120px',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap'
          }}
          title={event.node_id}
        >
          {event.node_id}
        </span>
      )}
      <span style={{
        color: isOutOfSchema ? colors.status.error : eventStyle.color,
        fontWeight: 'bold'
      }}>
        {eventStyle.symbol}
      </span>
      <span style={{
        color: isOutOfSchema ? colors.accent.lightRed : eventStyle.color,
        fontWeight: 500
      }}>
        {event.event_type}
      </span>
      {event.details && (
        <span
          style={{
            color: isOutOfSchema ? colors.accent.mediumRed : colors.text.muted,
            flex: 1,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap'
          }}
          title={event.details}
        >
          {event.details}
        </span>
      )}
    </div>
  );
}

export function ExecutionTimeline({
  events,
  startTime,
  maxHeight = '300px',
  expectedSchemaId,
  selectedIndex,
  onEventClick,
}: ExecutionTimelineProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<List>(null);
  const shouldVirtualize = events.length > VIRTUALIZATION_THRESHOLD;

  // Parse maxHeight to number for react-window
  const maxHeightNum = parseInt(maxHeight, 10) || 300;

  // Auto-scroll to bottom when new events arrive
  useEffect(() => {
    if (shouldVirtualize) {
      // M-471: Use react-window's scrollToItem for virtualized list
      listRef.current?.scrollToItem(events.length - 1, 'end');
    } else if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [events.length, shouldVirtualize]);

  // Memoized item data for react-window
  const itemData = useCallback((): RowData => ({
    events,
    startTime,
    expectedSchemaId,
    selectedIndex,
    onEventClick,
  }), [events, startTime, expectedSchemaId, selectedIndex, onEventClick]);

  if (events.length === 0) {
    return (
      <div data-testid="execution-timeline" style={{
        height: maxHeight,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: colors.bg.primary,
        borderRadius: borderRadius.lg,
        border: `1px solid ${colors.border.primary}`,
        color: colors.text.faint,
      }}>
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: '1.5rem', marginBottom: spacing[2] }}>Timeline</div>
          <div style={{ fontSize: '0.875rem' }}>Waiting for events...</div>
        </div>
      </div>
    );
  }

  return (
    <div data-testid="execution-timeline" style={{
      backgroundColor: colors.bg.primary,
      borderRadius: borderRadius.lg,
      border: `1px solid ${colors.border.primary}`,
      overflow: 'hidden'
    }}>
      {/* Header */}
      <div style={{
        padding: `${spacing[2]} ${spacing[3]}`,
        borderBottom: `1px solid ${colors.border.primary}`,
        backgroundColor: colors.bg.secondary,
        fontSize: fontSize.base,
        fontWeight: 600,
        color: colors.text.muted,
      }}>
        EXECUTION TIMELINE ({events.length} events)
      </div>

      {/* Event list - M-471: Use virtualization for large lists */}
      {shouldVirtualize ? (
        <List
          ref={listRef}
          height={maxHeightNum}
          itemCount={events.length}
          itemSize={EVENT_ROW_HEIGHT}
          width="100%"
          itemData={itemData()}
          style={{ padding: '8px 0' }}
        >
          {EventRow}
        </List>
      ) : (
        <div
          ref={containerRef}
          style={{
            maxHeight,
            overflowY: 'auto',
            padding: spacing[2],
          }}
        >
          {events.map((event, index) => {
            const style = EVENT_STYLES[event.event_type] || EVENT_STYLES.default;
            const elapsedMs = event.elapsed_ms ??
              (startTime != null ? event.timestamp - startTime : 0);
            const elapsedStr = formatElapsedTime(elapsedMs);
            const isOutOfSchema = expectedSchemaId && event.schema_id && event.schema_id !== expectedSchemaId;
            const isSelected = selectedIndex === index;
            const isClickable = !!onEventClick;

            return (
              <div
                key={index}
                onClick={isClickable ? () => onEventClick(index) : undefined}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: spacing[2],
                  padding: `${spacing[1]} ${spacing[2]}`,
                  marginBottom: spacing[1],
                  backgroundColor: isSelected
                    ? 'rgba(59, 130, 246, 0.25)'
                    : isOutOfSchema ? colors.statusBg.errorStrong : style.bgColor,
                  borderRadius: borderRadius.md,
                  borderLeft: `3px solid ${isSelected ? colors.status.info : isOutOfSchema ? colors.status.error : style.color}`,
                  fontSize: fontSize.base,
                  cursor: isClickable ? 'pointer' : 'default',
                  transition: 'background-color 0.15s ease',
                  ...(isOutOfSchema ? { boxShadow: 'inset 0 0 0 1px rgba(239, 68, 68, 0.3)' } : {}),
                  ...(isSelected ? { boxShadow: 'inset 0 0 0 1px rgba(59, 130, 246, 0.5)' } : {}),
                }}
              >
                {isOutOfSchema && (
                  <span
                    title={`Schema mismatch: expected ${expectedSchemaId?.slice(0, 8)}..., got ${event.schema_id?.slice(0, 8)}...`}
                    style={{
                      color: colors.status.error,
                      fontSize: fontSize.xs,
                      fontWeight: 700,
                      padding: `1px ${spacing[1]}`,
                      backgroundColor: 'rgba(239, 68, 68, 0.2)',
                      borderRadius: borderRadius.sm,
                      marginRight: spacing[1],
                    }}
                  >
                    !
                  </span>
                )}
                <span style={{
                  fontFamily: "'SF Mono', 'Monaco', 'Inconsolata', monospace",
                  color: isOutOfSchema ? colors.status.error : colors.text.faint,
                  minWidth: '60px'
                }}>
                  {elapsedStr}
                </span>
                {event.node_id && (
                  <span
                    style={{
                      color: isOutOfSchema ? colors.accent.lightRed : colors.accent.cyan,
                      fontWeight: 500,
                      minWidth: '80px',
                      maxWidth: '120px',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap'
                    }}
                    title={event.node_id}
                  >
                    {event.node_id}
                  </span>
                )}
                <span style={{
                  color: isOutOfSchema ? colors.status.error : style.color,
                  fontWeight: 'bold'
                }}>
                  {style.symbol}
                </span>
                <span style={{
                  color: isOutOfSchema ? colors.accent.lightRed : style.color,
                  fontWeight: 500
                }}>
                  {event.event_type}
                </span>
                {event.details && (
                  <span
                    style={{
                      color: isOutOfSchema ? colors.accent.mediumRed : colors.text.muted,
                      flex: 1,
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap'
                    }}
                    title={event.details}
                  >
                    {event.details}
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default ExecutionTimeline;
