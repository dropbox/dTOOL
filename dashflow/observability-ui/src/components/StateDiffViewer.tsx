import { useCallback, useMemo, useState } from 'react';
import { colors, spacing, borderRadius, fontSize } from '../styles/tokens';
import { Tooltip } from './Tooltip';

export type DiffType = 'unchanged' | 'new' | 'changed' | 'removed';

export interface StateDiffEntry {
  key: string;
  value: unknown;
  diffType: DiffType;
  previousValue?: unknown;
}

interface StateDiffViewerProps {
  currentState: Record<string, unknown>;
  previousState: Record<string, unknown>;
  maxHeight?: string;
  // Cursor state for header display (live vs paused)
  isLive?: boolean;
  // M-749: cursorSeq is string after M-693 BigInt migration
  cursorSeq?: string;
}

// Diff type styles (TC-04: using design tokens)
const DIFF_STYLES: Record<DiffType, { color: string; bgColor: string; marker: string }> = {
  unchanged: { color: colors.text.primary, bgColor: 'transparent', marker: '  ' },
  new: { color: colors.status.success, bgColor: colors.statusBg.success, marker: '+ ' },
  changed: { color: colors.status.warning, bgColor: colors.statusBg.warning, marker: '~ ' },
  removed: { color: colors.status.error, bgColor: colors.statusBg.error, marker: '- ' },
};

type DisplayValue = {
  preview: string;
  fullText: string;
  truncated: boolean;
  fullLength: number;
  previewIsPrefix: boolean;
};

function safeStringify(value: unknown, pretty: boolean): string {
  try {
    const json = JSON.stringify(value, null, pretty ? 2 : undefined);
    if (json === undefined) return String(value);
    return json;
  } catch {
    return String(value);
  }
}

// SV-05: Enhanced value formatting with tooltip, expansion, and modal support
function formatValue(value: unknown, maxPreviewChars: number = 60): DisplayValue {
  if (value === null) return { preview: 'null', fullText: 'null', truncated: false, fullLength: 4, previewIsPrefix: true };
  if (value === undefined) return { preview: 'undefined', fullText: 'undefined', truncated: false, fullLength: 9, previewIsPrefix: true };

  if (typeof value === 'string') {
    const fullText = safeStringify(value, false);
    if (fullText.length > maxPreviewChars) {
      const preview = fullText.slice(0, maxPreviewChars);
      return { preview, fullText, truncated: true, fullLength: fullText.length, previewIsPrefix: true };
    }
    return { preview: fullText, fullText, truncated: false, fullLength: fullText.length, previewIsPrefix: true };
  }

  if (typeof value === 'number' || typeof value === 'boolean') {
    const text = String(value);
    return { preview: text, fullText: text, truncated: false, fullLength: text.length, previewIsPrefix: true };
  }

  if (Array.isArray(value)) {
    const fullText = safeStringify(value, true);
    if (value.length === 0) return { preview: '[]', fullText, truncated: false, fullLength: 2, previewIsPrefix: true };
    const preview = `[${value.length} items]`;
    return { preview, fullText, truncated: true, fullLength: fullText.length, previewIsPrefix: false };
  }

  if (typeof value === 'object') {
    const fullText = safeStringify(value, true);
    const keys = Object.keys(value as Record<string, unknown>);
    if (keys.length === 0) return { preview: '{}', fullText, truncated: false, fullLength: 2, previewIsPrefix: true };
    if (fullText.length <= maxPreviewChars) {
      return { preview: fullText, fullText, truncated: false, fullLength: fullText.length, previewIsPrefix: true };
    }
    const preview = `{${keys.length} keys}`;
    return { preview, fullText, truncated: true, fullLength: fullText.length, previewIsPrefix: false };
  }

  const text = String(value);
  return { preview: text, fullText: text, truncated: false, fullLength: text.length, previewIsPrefix: true };
}


// Deep equality check without JSON.stringify for better performance
// Compares values recursively, tracking changed paths
function deepEqual(a: unknown, b: unknown): boolean {
  // Primitive comparison (fast path)
  if (a === b) return true;
  if (a === null || b === null) return a === b;
  if (typeof a !== typeof b) return false;

  // Arrays
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!deepEqual(a[i], b[i])) return false;
    }
    return true;
  }

  // Objects
  if (typeof a === 'object' && typeof b === 'object') {
    const aKeys = Object.keys(a as Record<string, unknown>);
    const bKeys = Object.keys(b as Record<string, unknown>);
    if (aKeys.length !== bKeys.length) return false;

    for (const key of aKeys) {
      if (!(key in (b as Record<string, unknown>))) return false;
      if (!deepEqual(
        (a as Record<string, unknown>)[key],
        (b as Record<string, unknown>)[key]
      )) return false;
    }
    return true;
  }

  // Other primitives (numbers, strings, booleans)
  return a === b;
}

// Get changed paths within a nested object (JSON Pointer format)
// Returns array of paths that differ between current and previous
// Exported for use by other components needing path-based diffing
export function getChangedPaths(
  current: unknown,
  previous: unknown,
  basePath: string = ''
): string[] {
  const changedPaths: string[] = [];

  // Both null/undefined
  if (current === undefined && previous === undefined) return [];
  if (current === null && previous === null) return [];

  // One is null/undefined
  if (current === undefined || current === null ||
      previous === undefined || previous === null) {
    return [basePath || '/'];
  }

  // Different types
  if (typeof current !== typeof previous) {
    return [basePath || '/'];
  }

  // Arrays
  if (Array.isArray(current) && Array.isArray(previous)) {
    const maxLen = Math.max(current.length, previous.length);
    for (let i = 0; i < maxLen; i++) {
      const path = `${basePath}/${i}`;
      if (i >= current.length || i >= previous.length) {
        changedPaths.push(path);
      } else if (!deepEqual(current[i], previous[i])) {
        // Recurse for nested changes
        const nested = getChangedPaths(current[i], previous[i], path);
        changedPaths.push(...nested);
      }
    }
    return changedPaths;
  }

  // Objects
  if (typeof current === 'object' && typeof previous === 'object') {
    const currentObj = current as Record<string, unknown>;
    const previousObj = previous as Record<string, unknown>;
    const allKeys = new Set([...Object.keys(currentObj), ...Object.keys(previousObj)]);

    for (const key of allKeys) {
      // Escape JSON Pointer special characters
      const escapedKey = key.replace(/~/g, '~0').replace(/\//g, '~1');
      const path = `${basePath}/${escapedKey}`;

      if (!(key in currentObj) || !(key in previousObj)) {
        changedPaths.push(path);
      } else if (!deepEqual(currentObj[key], previousObj[key])) {
        // Recurse for nested changes
        const nested = getChangedPaths(currentObj[key], previousObj[key], path);
        changedPaths.push(...nested);
      }
    }
    return changedPaths;
  }

  // Primitive values that differ
  if (current !== previous) {
    return [basePath || '/'];
  }

  return [];
}

function computeDiff(
  current: Record<string, unknown>,
  previous: Record<string, unknown>
): StateDiffEntry[] {
  const entries: StateDiffEntry[] = [];
  const allKeys = new Set([...Object.keys(current), ...Object.keys(previous)]);
  const sortedKeys = Array.from(allKeys).sort();

  for (const key of sortedKeys) {
    const currentVal = current[key];
    const previousVal = previous[key];
    const inCurrent = key in current;
    const inPrevious = key in previous;

    if (inCurrent && inPrevious) {
      // Use deepEqual instead of JSON.stringify for performance
      const isEqual = deepEqual(currentVal, previousVal);
      entries.push({
        key,
        value: currentVal,
        diffType: isEqual ? 'unchanged' : 'changed',
        previousValue: isEqual ? undefined : previousVal,
      });
    } else if (inCurrent && !inPrevious) {
      // New key
      entries.push({
        key,
        value: currentVal,
        diffType: 'new',
      });
    } else if (!inCurrent && inPrevious) {
      // Removed key
      entries.push({
        key,
        value: previousVal,
        diffType: 'removed',
      });
    }
  }

  return entries;
}

export function StateDiffViewer({
  currentState,
  previousState,
  maxHeight = '300px',
  isLive = true,
  cursorSeq,
}: StateDiffViewerProps) {
  const [expandedKeys, setExpandedKeys] = useState<Set<string>>(new Set());
  const [modal, setModal] = useState<{ title: string; content: string } | null>(null);

  const toggleExpanded = useCallback((key: string) => {
    setExpandedKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const handleValueClick = useCallback((entryId: string, title: string, formatted: DisplayValue) => {
    if (!formatted.truncated) return;
    if (formatted.fullLength > 500) {
      setModal({ title, content: formatted.fullText });
      return;
    }
    toggleExpanded(entryId);
  }, [toggleExpanded]);

  const handleCopy = useCallback((text: string) => {
    navigator.clipboard.writeText(text);
  }, []);

  const diffEntries = useMemo(
    () => computeDiff(currentState, previousState),
    [currentState, previousState]
  );

  const isEmpty = Object.keys(currentState).length === 0 && Object.keys(previousState).length === 0;

  if (isEmpty) {
    return (
      <div data-testid="state-diff" style={{
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
          <div style={{ fontSize: '1.5rem', marginBottom: spacing[2] }}>{'{}'}</div>
          <div style={{ fontSize: '0.875rem' }}>Waiting for state...</div>
        </div>
      </div>
    );
  }

  // Count changes
  const counts = {
    new: diffEntries.filter(e => e.diffType === 'new').length,
    changed: diffEntries.filter(e => e.diffType === 'changed').length,
    removed: diffEntries.filter(e => e.diffType === 'removed').length,
  };

  return (
    <div data-testid="state-diff" style={{
      backgroundColor: colors.bg.primary,
      borderRadius: borderRadius.lg,
      border: `1px solid ${colors.border.primary}`,
      overflow: 'hidden'
    }}>
      {/* Header - Reflects live vs paused cursor state */}
      <div style={{
        padding: `${spacing[2]} ${spacing[3]}`,
        borderBottom: `1px solid ${colors.border.primary}`,
        backgroundColor: colors.bg.secondary,
        fontSize: fontSize.base,
        fontWeight: 600,
        color: colors.text.muted,
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center'
      }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: spacing[1] }}>
          {isLive ? (
            <>
              <span style={{
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                backgroundColor: colors.status.error,
                animation: 'pulse 1.5s infinite',
              }} />
              <span style={{ color: colors.status.error }}>LIVE STATE</span>
            </>
          ) : (
            <>
              <span style={{
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                backgroundColor: colors.text.faint,
              }} />
              <span>STATE @ seq={cursorSeq ?? '?'}</span>
            </>
          )}
        </span>
        <div style={{ display: 'flex', gap: spacing[2] }}>
          {counts.new > 0 && (
            <span style={{ color: DIFF_STYLES.new.color }}>+{counts.new}</span>
          )}
          {counts.changed > 0 && (
            <span style={{ color: DIFF_STYLES.changed.color }}>~{counts.changed}</span>
          )}
          {counts.removed > 0 && (
            <span style={{ color: DIFF_STYLES.removed.color }}>-{counts.removed}</span>
          )}
        </div>
      </div>

      {/* Legend */}
      <div style={{
        padding: `${spacing[1]} ${spacing[3]}`,
        borderBottom: `1px solid ${colors.border.primary}`,
        backgroundColor: colors.bg.secondary,
        fontSize: fontSize.xs,
        color: colors.text.faint,
        display: 'flex',
        gap: spacing[3],
      }}>
        <span><span style={{ color: DIFF_STYLES.new.color }}>+</span> new</span>
        <span><span style={{ color: DIFF_STYLES.changed.color }}>~</span> changed</span>
        <span><span style={{ color: DIFF_STYLES.removed.color }}>-</span> removed</span>
      </div>

      {/* State entries */}
      <div style={{
        maxHeight,
        overflowY: 'auto',
        padding: spacing[2],
        fontFamily: "'SF Mono', 'Monaco', 'Inconsolata', monospace",
        fontSize: fontSize.base,
      }}>
        {diffEntries.map((entry, index) => {
          const style = DIFF_STYLES[entry.diffType];
          const entryId = `${entry.key}-${index}`;
          const formatted = formatValue(entry.value);
          const expanded = expandedKeys.has(entryId);
          const fullPreview = formatted.fullText.length > 500
            ? `${formatted.fullText.slice(0, 500)}\n…`
            : formatted.fullText;
          const moreChars = formatted.previewIsPrefix
            ? Math.max(0, formatted.fullLength - formatted.preview.length)
            : formatted.fullLength;

          return (
            <div
              key={`${entry.key}-${index}`}
              style={{
                display: 'flex',
                flexDirection: expanded ? 'column' : 'row',
                padding: `${spacing[1]} ${spacing[1]}`,
                marginBottom: '2px',
                backgroundColor: style.bgColor,
                borderRadius: borderRadius.sm,
                color: style.color
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', width: '100%' }}>
                {/* Diff marker */}
                <span style={{
                  minWidth: '20px',
                  fontWeight: 'bold',
                  color: style.color
                }}>
                  {style.marker}
                </span>

                {/* Key */}
                <span style={{
                  color: colors.accent.cyan,
                  marginRight: spacing[1],
                }}>
                  {entry.key}
                </span>

                <span style={{ color: colors.text.faint }}>:</span>

                {/* SV-05: Value with tooltip + expand/modal */}
                <Tooltip
                  content={
                    formatted.truncated ? (
                      <pre style={{ margin: 0, whiteSpace: 'pre-wrap', maxWidth: 460 }}>
                        {fullPreview}
                      </pre>
                    ) : ''
                  }
                  disabled={!formatted.truncated}
                  position="top"
                  maxWidth={520}
                >
                  <span
                    style={{
                      marginLeft: spacing[1],
                      color: style.color,
                      fontWeight: entry.diffType !== 'unchanged' ? 600 : 400,
                      flex: 1,
                      overflow: 'hidden',
                      whiteSpace: expanded ? 'normal' : 'nowrap',
                      textOverflow: expanded ? 'clip' : 'ellipsis',
                      display: 'flex',
                      alignItems: 'center',
                      gap: spacing[1],
                      cursor: formatted.truncated ? 'pointer' : 'default',
                      position: 'relative',
                    }}
                    onClick={() => handleValueClick(entryId, entry.key, formatted)}
                    title={formatted.truncated ? `Click to ${formatted.fullLength > 500 ? 'view' : expanded ? 'collapse' : 'expand'}` : undefined}
                  >
                    <span style={{
                      overflow: 'hidden',
                      textOverflow: expanded ? 'clip' : 'ellipsis',
                      whiteSpace: expanded ? 'pre-wrap' : 'nowrap',
                      flex: 1,
                      position: 'relative',
                    }}>
                      {expanded ? formatted.fullText : formatted.preview}
                      {formatted.truncated && !expanded && formatted.previewIsPrefix && (
                        <span
                          style={{
                            position: 'absolute',
                            right: 0,
                            top: 0,
                            bottom: 0,
                            width: '42px',
                            background: `linear-gradient(to right, rgba(0,0,0,0), ${colors.bg.primary})`,
                            pointerEvents: 'none',
                          }}
                        />
                      )}
                    </span>
                    {formatted.truncated && (
                      <span
                        style={{
                          fontSize: '9px',
                          color: colors.text.muted,
                          backgroundColor: colors.border.primary,
                          padding: `1px ${spacing[1]}`,
                          borderRadius: borderRadius.sm,
                          whiteSpace: 'nowrap',
                          cursor: 'pointer',
                          flexShrink: 0,
                        }}
                        onClick={(e) => {
                          e.stopPropagation();
                          if (formatted.fullLength > 500) setModal({ title: entry.key, content: formatted.fullText });
                          else toggleExpanded(entryId);
                        }}
                        title={formatted.previewIsPrefix ? `+${moreChars.toLocaleString()} more chars` : `${formatted.fullLength.toLocaleString()} chars`}
                      >
                        {formatted.previewIsPrefix ? `+${moreChars.toLocaleString()} more` : `${formatted.fullLength.toLocaleString()} chars`}
                      </span>
                    )}
                    {expanded && (
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleCopy(formatted.fullText);
                        }}
                        style={{
                          fontSize: '10px',
                          color: colors.text.faint,
                          backgroundColor: colors.bg.secondary,
                          border: `1px solid ${colors.border.secondary}`,
                          borderRadius: borderRadius.sm,
                          padding: `1px ${spacing[1]}`,
                          cursor: 'pointer',
                          flexShrink: 0,
                        }}
                        title="Copy full value"
                      >
                        Copy
                      </button>
                    )}
                  </span>
                </Tooltip>
              </div>

              {/* Previous value for changed entries */}
              {entry.diffType === 'changed' && entry.previousValue !== undefined && (() => {
                const prevFormatted = formatValue(entry.previousValue, 30);
                return (
                  <span
                    style={{
                      marginLeft: spacing[2],
                      color: colors.text.faint,
                      fontSize: fontSize.xs,
                      textDecoration: 'line-through',
                      maxWidth: '150px',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                    title={prevFormatted.truncated ? `Previous value (${prevFormatted.fullLength} chars)` : undefined}
                  >
                    was: {prevFormatted.preview}
                  </span>
                );
              })()}
            </div>
          );
        })}
      </div>

      {/* SV-05: Modal for very long values */}
      {modal && (
        <div
          role="dialog"
          aria-modal="true"
          style={{
            position: 'fixed',
            inset: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 9999,
            padding: spacing[4],
          }}
          onClick={() => setModal(null)}
        >
          <div
            style={{
              width: 'min(900px, 96vw)',
              maxHeight: '80vh',
              backgroundColor: colors.bg.primary,
              border: `1px solid ${colors.border.primary}`,
              borderRadius: borderRadius.lg,
              overflow: 'hidden',
              boxShadow: '0 10px 30px rgba(0,0,0,0.5)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{
              padding: `${spacing[2]} ${spacing[3]}`,
              backgroundColor: colors.bg.secondary,
              borderBottom: `1px solid ${colors.border.primary}`,
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              gap: spacing[2],
            }}>
              <div style={{ color: colors.text.primary, fontWeight: 600 }}>
                {modal.title}
              </div>
              <div style={{ display: 'flex', gap: spacing[2] }}>
                <button
                  type="button"
                  onClick={() => handleCopy(modal.content)}
                  style={{
                    backgroundColor: colors.bg.overlay,
                    color: colors.text.primary,
                    border: `1px solid ${colors.border.secondary}`,
                    borderRadius: borderRadius.md,
                    padding: `4px ${spacing[2]}`,
                    cursor: 'pointer',
                    fontSize: fontSize.sm,
                  }}
                >
                  Copy
                </button>
                <button
                  type="button"
                  onClick={() => setModal(null)}
                  style={{
                    backgroundColor: 'transparent',
                    color: colors.text.faint,
                    border: `1px solid ${colors.border.secondary}`,
                    borderRadius: borderRadius.md,
                    padding: `4px ${spacing[2]}`,
                    cursor: 'pointer',
                    fontSize: fontSize.sm,
                  }}
                >
                  Close
                </button>
              </div>
            </div>
            <pre style={{
              margin: 0,
              padding: spacing[3],
              overflow: 'auto',
              maxHeight: 'calc(80vh - 60px)',
              color: colors.text.primary,
              fontSize: fontSize.sm,
              whiteSpace: 'pre-wrap',
            }}>
              {modal.content}
            </pre>
          </div>
        </div>
      )}

      {/* CSS animation for live pulse */}
      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }
      `}</style>
    </div>
  );
}

export default StateDiffViewer;
