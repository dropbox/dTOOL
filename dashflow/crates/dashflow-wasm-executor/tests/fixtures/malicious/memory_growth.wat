;; Memory growth attack: Tries to grow memory beyond limits
;; Tests dynamic memory growth limits
(module
  ;; Start with 1 page, max 10000 pages (640MB)
  (memory $mem 1 10000)

  (func $try_grow_memory (export "try_grow_memory") (result i32)
    ;; Try to grow memory by 5000 pages (320MB)
    ;; This should fail if memory limits are enforced
    (memory.grow (i32.const 5000))
  )
)
