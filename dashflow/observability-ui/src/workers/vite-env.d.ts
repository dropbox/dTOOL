// Vite Web Worker type declarations
// See: https://vitejs.dev/guide/features.html#web-workers

declare module '*?worker' {
  const WorkerFactory: {
    new (): Worker;
  };
  export default WorkerFactory;
}

declare module '*?worker&inline' {
  const WorkerFactory: {
    new (): Worker;
  };
  export default WorkerFactory;
}
