import { useState, useRef, useEffect, ReactNode } from 'react';
import { colors, spacing, borderRadius, fontSize } from '../styles/tokens';

export type TooltipPosition = 'top' | 'bottom' | 'left' | 'right';

export interface RectLike {
  left: number;
  top: number;
  width: number;
  height: number;
  right: number;
  bottom: number;
}

export function computeTooltipCoordsUnclamped(
  triggerRect: RectLike,
  tooltipRect: Pick<RectLike, 'width' | 'height'>,
  position: TooltipPosition,
  gap: number = 8
): { x: number; y: number } {
  let x = 0;
  let y = 0;

  switch (position) {
    case 'top':
      x = triggerRect.left + (triggerRect.width - tooltipRect.width) / 2;
      y = triggerRect.top - tooltipRect.height - gap;
      break;
    case 'bottom':
      x = triggerRect.left + (triggerRect.width - tooltipRect.width) / 2;
      y = triggerRect.bottom + gap;
      break;
    case 'left':
      x = triggerRect.left - tooltipRect.width - gap;
      y = triggerRect.top + (triggerRect.height - tooltipRect.height) / 2;
      break;
    case 'right':
      x = triggerRect.right + gap;
      y = triggerRect.top + (triggerRect.height - tooltipRect.height) / 2;
      break;
  }

  return { x, y };
}

export function clampTooltipCoordsToViewport(
  coords: { x: number; y: number },
  tooltipRect: Pick<RectLike, 'width' | 'height'>,
  viewport: { width: number; height: number },
  padding: number = 8
): { x: number; y: number } {
  const x = Math.max(
    padding,
    Math.min(coords.x, viewport.width - tooltipRect.width - padding)
  );
  const y = Math.max(
    padding,
    Math.min(coords.y, viewport.height - tooltipRect.height - padding)
  );
  return { x, y };
}

export function computeTooltipCoords(
  triggerRect: RectLike,
  tooltipRect: Pick<RectLike, 'width' | 'height'>,
  position: TooltipPosition,
  viewport: { width: number; height: number },
  gap: number = 8,
  viewportPadding: number = 8
): { x: number; y: number } {
  return clampTooltipCoordsToViewport(
    computeTooltipCoordsUnclamped(triggerRect, tooltipRect, position, gap),
    tooltipRect,
    viewport,
    viewportPadding
  );
}

interface TooltipProps {
  content: string | ReactNode;
  children: ReactNode;
  /** Position relative to trigger element */
  position?: TooltipPosition;
  /** Delay in ms before showing tooltip */
  delay?: number;
  /** Max width of tooltip */
  maxWidth?: number;
  /** Disabled state - tooltip won't show */
  disabled?: boolean;
}

/**
 * TC-08: Consistent tooltip component for the observability UI
 *
 * Usage:
 *   <Tooltip content="Helpful text">
 *     <button>Hover me</button>
 *   </Tooltip>
 */
export function Tooltip({
  content,
  children,
  position = 'top',
  delay = 200,
  maxWidth = 250,
  disabled = false,
}: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const [coords, setCoords] = useState({ x: 0, y: 0 });
  const triggerRef = useRef<HTMLDivElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>();

  // Calculate tooltip position based on trigger element
  const updatePosition = () => {
    if (!triggerRef.current || !tooltipRef.current) return;

    const triggerRect = triggerRef.current.getBoundingClientRect();
    const tooltipRect = tooltipRef.current.getBoundingClientRect();
    setCoords(
      computeTooltipCoords(
        triggerRect as RectLike,
        tooltipRect as RectLike,
        position,
        { width: window.innerWidth, height: window.innerHeight }
      )
    );
  };

  useEffect(() => {
    if (visible) {
      updatePosition();
    }
  }, [visible, position]);

  const handleMouseEnter = () => {
    if (disabled || !content) return;
    timeoutRef.current = setTimeout(() => {
      setVisible(true);
    }, delay);
  };

  const handleMouseLeave = () => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    setVisible(false);
  };

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  // Don't render tooltip wrapper if disabled or no content
  if (disabled || !content) {
    return <>{children}</>;
  }

  return (
    <>
      <div
        ref={triggerRef}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        onFocus={handleMouseEnter}
        onBlur={handleMouseLeave}
        style={{ display: 'inline-block' }}
      >
        {children}
      </div>
      {visible && (
        <div
          ref={tooltipRef}
          role="tooltip"
          style={{
            position: 'fixed',
            left: coords.x,
            top: coords.y,
            zIndex: 9999,
            maxWidth,
            padding: `${spacing['1']} ${spacing['2']}`,
            backgroundColor: colors.bg.overlay,
            color: colors.text.primary,
            fontSize: fontSize.sm,
            borderRadius: borderRadius.md,
            border: `1px solid ${colors.border.secondary}`,
            boxShadow: '0 4px 12px rgba(0, 0, 0, 0.4)',
            pointerEvents: 'none',
            whiteSpace: 'normal',
            wordWrap: 'break-word',
            lineHeight: 1.4,
          }}
        >
          {content}
        </div>
      )}
    </>
  );
}

export default Tooltip;
