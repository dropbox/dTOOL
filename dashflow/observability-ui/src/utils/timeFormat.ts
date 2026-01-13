export function formatUptime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0s';

  const wholeSeconds = Math.floor(seconds);
  if (wholeSeconds < 60) return `${wholeSeconds}s`;

  if (wholeSeconds < 3600) {
    const mins = Math.floor(wholeSeconds / 60);
    const secs = wholeSeconds % 60;
    return `${mins}m ${secs}s`;
  }

  // C-09: For durations >= 24 hours, show days
  if (wholeSeconds >= 86400) {
    const days = Math.floor(wholeSeconds / 86400);
    const hours = Math.floor((wholeSeconds % 86400) / 3600);
    const mins = Math.floor((wholeSeconds % 3600) / 60);
    return `${days}d ${hours}h ${mins}m`;
  }

  const hours = Math.floor(wholeSeconds / 3600);
  const mins = Math.floor((wholeSeconds % 3600) / 60);
  return `${hours}h ${mins}m`;
}

// T-06: Consistent 24-hour timestamp formatting for UI display
// Avoids locale-dependent formats like "7:03:09 PM" vs "19:03:09"
export function formatTimestamp(date: Date | number): string {
  const d = typeof date === 'number' ? new Date(date) : date;
  return d.toLocaleTimeString('en-GB', { hour12: false });
}

// T-08: Map internal Kafka status strings to user-friendly text
// Internal states like "waiting_for_messages" become "Waiting for data..."
export function formatKafkaStatus(status: string): string {
  const mapping: Record<string, string> = {
    'connected': 'Connected',
    'waiting_for_messages': 'Waiting for data...',
    'waiting': 'Waiting for data...',
    'reconnecting': 'Reconnecting...',
    'disconnected': 'Disconnected',
    'error': 'Error',
    'healthy': 'Connected',
    'degraded': 'Degraded',
  };
  // Return mapped value, or capitalize first letter of unknown status
  return mapping[status.toLowerCase()] ??
    status.charAt(0).toUpperCase() + status.slice(1).replace(/_/g, ' ');
}

