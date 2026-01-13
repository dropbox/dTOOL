// M-998: Worker pool manager for decode operations
// Manages worker lifecycle and provides timeout support for decode operations.
// If a decode times out, the worker is terminated and recreated.
// M-2488: Handle React Strict Mode which terminates pool during cleanup

import type { DecodedMessageSerializable } from './decode.worker';

// Import the worker using Vite's worker import syntax
import DecodeWorker from './decode.worker?worker';

type WorkerRequest = { type: 'init'; id: number } | { type: 'decode'; id: number; buffer: ArrayBuffer };
type WorkerResponse = { type: 'init_result'; id: number; success: boolean; error?: string } | { type: 'decode_result'; id: number; result: DecodedMessageSerializable | null; error?: string };

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timeoutId: number;
}

export class DecodeWorkerPool {
  private worker: Worker | null = null;
  private initialized = false;
  private initPromise: Promise<void> | null = null;
  private requestId = 0;
  private pendingRequests = new Map<number, PendingRequest>();
  private defaultTimeoutMs: number;
  private terminated = false;

  constructor(timeoutMs = 30_000) {
    this.defaultTimeoutMs = timeoutMs;
  }

  private createWorker(): Worker {
    const worker = new DecodeWorker();
    worker.onmessage = this.handleMessage.bind(this);
    worker.onerror = this.handleError.bind(this);
    return worker;
  }

  private handleMessage(event: MessageEvent<WorkerResponse>) {
    const response = event.data;
    const pending = this.pendingRequests.get(response.id);
    if (!pending) return;

    window.clearTimeout(pending.timeoutId);
    this.pendingRequests.delete(response.id);

    if (response.type === 'init_result') {
      if (response.success) {
        pending.resolve(undefined);
      } else {
        pending.reject(new Error(response.error || 'Worker init failed'));
      }
    } else if (response.type === 'decode_result') {
      pending.resolve(response.result);
    }
  }

  private handleError(event: ErrorEvent) {
    console.error('[DecodeWorkerPool] Worker error:', event);
    // Reject all pending requests
    for (const [id, pending] of this.pendingRequests) {
      window.clearTimeout(pending.timeoutId);
      pending.reject(new Error('Worker error: ' + event.message));
      this.pendingRequests.delete(id);
    }
    // Reset worker state
    this.terminateWorker();
  }

  private terminateWorker() {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }
    this.initialized = false;
    this.initPromise = null;
  }

  private handleTimeout(id: number) {
    const pending = this.pendingRequests.get(id);
    if (!pending) return;

    this.pendingRequests.delete(id);
    // M-1023: Use TimeoutError name for consistent timeout classification in App.tsx
    const err = new Error('Decode operation timed out');
    err.name = 'TimeoutError';
    pending.reject(err);

    // Terminate and recreate worker on timeout
    console.warn('[DecodeWorkerPool] Decode timed out, terminating and recreating worker');
    this.terminateWorker();
  }

  private sendRequest<T>(request: WorkerRequest, timeoutMs?: number): Promise<T> {
    return new Promise((resolve, reject) => {
      if (!this.worker) {
        reject(new Error('Worker not initialized'));
        return;
      }

      const id = request.id;
      const timeout = timeoutMs ?? this.defaultTimeoutMs;
      const timeoutId = window.setTimeout(() => this.handleTimeout(id), timeout);

      this.pendingRequests.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timeoutId,
      });

      // Transfer buffer ownership for decode requests
      if (request.type === 'decode') {
        this.worker.postMessage(request, [request.buffer]);
      } else {
        this.worker.postMessage(request);
      }
    });
  }

  async init(): Promise<void> {
    if (this.initialized) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = (async () => {
      this.worker = this.createWorker();
      const id = ++this.requestId;
      await this.sendRequest<void>({ type: 'init', id }, 10_000); // 10s init timeout
      this.initialized = true;
    })();

    try {
      await this.initPromise;
    } catch (e) {
      this.initPromise = null;
      throw e;
    }
  }

  async decode(buffer: ArrayBuffer, timeoutMs?: number): Promise<DecodedMessageSerializable | null> {
    // Ensure initialized
    if (!this.initialized) {
      await this.init();
    }

    const id = ++this.requestId;
    return this.sendRequest<DecodedMessageSerializable | null>(
      { type: 'decode', id, buffer },
      timeoutMs
    );
  }

  isInitialized(): boolean {
    return this.initialized;
  }

  terminate() {
    // Cancel all pending requests
    for (const [id, pending] of this.pendingRequests) {
      window.clearTimeout(pending.timeoutId);
      pending.reject(new Error('Worker pool terminated'));
      this.pendingRequests.delete(id);
    }
    this.terminateWorker();
    this.terminated = true;
  }

  isTerminated(): boolean {
    return this.terminated;
  }
}

// Singleton instance
let workerPool: DecodeWorkerPool | null = null;

export function getDecodeWorkerPool(): DecodeWorkerPool {
  // M-2488: Handle React Strict Mode which terminates the pool during cleanup
  // If the singleton was terminated, create a new instance
  if (!workerPool || workerPool.isTerminated()) {
    workerPool = new DecodeWorkerPool();
  }
  return workerPool;
}

export type { DecodedMessageSerializable };
