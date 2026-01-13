;; Memory bomb: Attempts to allocate excessive memory
;; Tests memory limit enforcement
(module
  ;; Request 1000 pages (64MB) of memory - should be blocked by memory limit
  (memory $mem 1000 1000)

  (func $allocate_memory (export "allocate_memory") (result i32)
    ;; Try to write to high memory addresses
    (i32.store (i32.const 65000000) (i32.const 42))
    (i32.load (i32.const 65000000))
  )
)
