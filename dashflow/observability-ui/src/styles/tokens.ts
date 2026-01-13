// Design tokens for consistent UI theming
// TC-04: Centralized color palette replacing hard-coded values

export const colors = {
  // Background colors (dark theme)
  bg: {
    primary: '#1a1a2e',      // Main canvas/panel background
    secondary: '#151525',    // Headers, nested sections
    tertiary: '#0f0f1a',     // Deep background, borders
    surface: '#1f2937',      // Elevated surfaces, buttons
    surfaceHover: '#374151', // Hover state for surfaces
    overlay: '#242424',      // Overlays, badges
    emptyState: 'rgba(40, 40, 40, 0.5)', // Empty states, loading placeholders
    slider: '#0b1020',       // Slider thumb border
    card: '#1a1a1a',         // Card backgrounds, panels
    elevated: '#2a2a3a',     // Elevated hover states
    dropdown: '#1f1f2f',     // Dropdown menu background
  },

  // Border colors
  border: {
    primary: '#333',         // Standard borders
    secondary: '#444',       // Heavier borders
    hover: '#555',           // Border on hover
    muted: '#2a2a3e',        // Subtle borders
    dashed: '#444',          // Dashed borders for loading states
    separator: '#444',       // Vertical/horizontal separators
  },

  // Text colors
  text: {
    primary: '#e0e0e0',      // Main text
    secondary: '#bbb',       // Dimmer text
    tertiary: '#aaa',        // Chart axis text, labels
    muted: '#888',           // Secondary/label text
    faint: '#666',           // Very dim text
    disabled: '#555',        // Disabled state
    light: '#ccc',           // Light text on dark backgrounds
    lighter: '#ddd',         // Lighter text
    white: '#fff',           // White text
    black: '#000',           // Black text (on light warning banners)
    link: '#8ab4f8',         // Link text, clickable items
    code: '#82ca9d',         // Code/monospace text
  },

  // Status/semantic colors
  status: {
    success: '#22c55e',      // Completed, new, valid
    successDark: '#16a34a',  // Darker green for diff additions
    successLime: '#65a30d',  // Lime green for "identical" status
    emerald: '#10b981',      // Emerald green for match/verified status
    error: '#ef4444',        // Error, removed, invalid
    errorDark: '#dc2626',    // Darker red for diff removals
    warning: '#f59e0b',      // Warning, changed
    warningDark: '#ca8a04',  // Darker amber for modifications
    info: '#3b82f6',         // Info, running, active
    infoHover: '#60a5fa',    // Info hover
    neutral: '#9ca3af',      // Default/unknown status
    neutralDark: '#6b7280',  // Darker neutral
  },

  // Status backgrounds (with opacity for subtle highlighting)
  statusBg: {
    success: 'rgba(34, 197, 94, 0.1)',
    // Circuit breaker healthy state (Material green)
    successMaterial: 'rgba(76, 175, 80, 0.1)',
    // Emerald status backgrounds for match/verified
    emerald: 'rgba(16, 185, 129, 0.1)',
    emeraldBorder: 'rgba(16, 185, 129, 0.3)',
    error: 'rgba(239, 68, 68, 0.1)',
    errorStrong: 'rgba(239, 68, 68, 0.15)',
    // Solid error backgrounds for error boundaries/alerts
    errorSolid: '#2d1f1f',
    errorSolidStrong: '#7f1d1d',
    errorBorder: '#dc2626',
    // Border variants for live/error indicators
    errorBorderSubtle: 'rgba(239, 68, 68, 0.4)',
    // Circuit breaker will_restart state (Material red)
    errorMaterial: 'rgba(244, 67, 54, 0.1)',
    warningBorder: 'rgba(251, 191, 36, 0.3)',
    warning: 'rgba(245, 158, 11, 0.1)',
    // Amber warning (brighter variant for schema indicators)
    warningAmber: 'rgba(251, 191, 36, 0.1)',
    // Circuit breaker degraded state (Material orange)
    warningMaterial: 'rgba(255, 152, 0, 0.1)',
    info: 'rgba(59, 130, 246, 0.1)',
    // Info light variant (using infoHover color)
    infoLight: 'rgba(96, 165, 250, 0.1)',
    infoBorder: 'rgba(96, 165, 250, 0.3)',
    neutral: 'rgba(156, 163, 175, 0.1)',
    neutralBorder: 'rgba(156, 163, 175, 0.4)',
    purple: 'rgba(139, 92, 246, 0.1)',
  },

  alpha: {
    white05: 'rgba(255, 255, 255, 0.05)',
    white08: 'rgba(255, 255, 255, 0.08)',
    white10: 'rgba(255, 255, 255, 0.1)',
    white15: 'rgba(255, 255, 255, 0.15)',
    // Black overlays for button highlights on light backgrounds
    black10: 'rgba(0, 0, 0, 0.1)',
    black15: 'rgba(0, 0, 0, 0.15)',
    black20: 'rgba(0, 0, 0, 0.2)',
    // Gray overlays for loading states/skeletons
    gray08: 'rgba(136, 136, 136, 0.08)',
    gray25: 'rgba(136, 136, 136, 0.25)',
  },

  // Accent colors
  accent: {
    cyan: '#06b6d4',         // Keys, node IDs, highlights
    purple: '#8b5cf6',       // State updates
    amber: '#fbbf24',        // Warning highlights (amber-400)
    lightRed: '#fca5a5',     // Error text on dark
    mediumRed: '#f87171',    // Error details
  },

  // Graph-specific status colors (for nodes/edges)
  graph: {
    pending: '#9ca3af',
    active: '#3b82f6',
    completed: '#22c55e',
    error: '#ef4444',
    // Darker variants for strokes/borders
    pendingStroke: '#9ca3af',
    activeStroke: '#1d4ed8',
    completedStroke: '#15803d',
    errorStroke: '#b91c1c',
    // Edge type colors
    conditional: '#f59e0b',
    parallel: '#8b5cf6',
  },

  // Chart colors for data visualization (Recharts, etc.)
  chart: {
    purple: '#8884d8',
    green: '#82ca9d',
    yellow: '#ffc658',
    orange: '#ff7300',
    teal: '#00C49F',
  },

  // Connection status colors (WebSocket health, etc.)
  connection: {
    healthy: '#4CAF50',
    degraded: '#ff9800',
    reconnecting: '#2196F3',
    waiting: '#9e9e9e',
    unavailable: '#f44336',
  },

  // UI component colors (tabs, banners, etc.)
  ui: {
    tabActive: '#2196F3',        // Active tab background
    bannerWarning: '#f59e0b',    // Demo mode banner (amber-500)
    bannerWarningDark: '#d97706', // Config drift banner (amber-600)
    bannerError: '#b91c1c',      // Schema mismatch banner (red-700)
  },
} as const;

// TC-05: Standardized spacing scale (4px base unit)
export const spacing = {
  '0': '0px',
  '1': '4px',
  '2': '8px',
  '3': '12px',
  '4': '16px',
  '5': '20px',
  '6': '24px',
  '8': '32px',
  '10': '40px',
  '12': '48px',
} as const;

// Font sizes
export const fontSize = {
  xs: '10px',
  sm: '11px',
  base: '12px',
  md: '14px',
  lg: '16px',
  xl: '20px',
} as const;

// Border radius
export const borderRadius = {
  sm: '3px',
  md: '4px',
  lg: '8px',
  full: '9999px',
} as const;

// Shadows
export const shadows = {
  focus: '0 0 0 2px rgba(59, 130, 246, 0.35)',
  focusLarge: '0 0 0 4px rgba(59, 130, 246, 0.35)',
  thumbGlow: '0 0 0 1px rgba(59, 130, 246, 0.2)',
  error: 'inset 0 0 0 1px rgba(239, 68, 68, 0.3)',
  dropdown: '0 4px 12px rgba(0,0,0,0.4)',
} as const;

// Animation durations
export const durations = {
  fast: '80ms',
  normal: '120ms',
  slow: '200ms',
} as const;

// Type exports for type safety
export type ColorKey = keyof typeof colors;
export type SpacingKey = keyof typeof spacing;
export type FontSizeKey = keyof typeof fontSize;
