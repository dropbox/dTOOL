;; Out of bounds memory access
;; Tests WASM trap handling
(module
  (memory $mem 1) ;; 1 page = 64KB

  ;; Try to read beyond memory bounds
  (func $read_oob (export "read_oob") (result i32)
    ;; Page is 64KB, try to read at 100KB
    (i32.load (i32.const 100000))
  )

  ;; Try to write beyond memory bounds
  (func $write_oob (export "write_oob") (result i32)
    ;; Try to write at 100KB (beyond 64KB page)
    (i32.store (i32.const 100000) (i32.const 42))
    (i32.const 1)
  )
)
