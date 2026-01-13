(module
  ;; Declare memory (1 page = 64KB)
  (memory (export "memory") 1)

  ;; Store value at offset 0
  (func $store_value (export "store_value") (param $val i32)
    i32.const 0
    local.get $val
    i32.store
  )

  ;; Load value from offset 0
  (func $load_value (export "load_value") (result i32)
    i32.const 0
    i32.load
  )
)
