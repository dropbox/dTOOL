(module
  ;; Infinite loop for testing timeouts and fuel limits
  (func $infinite_loop (export "infinite_loop") (result i32)
    (local $counter i32)
    (local.set $counter (i32.const 0))
    (block $break
      (loop $continue
        ;; Increment counter
        (local.set $counter (i32.add (local.get $counter) (i32.const 1)))
        ;; Infinite loop - never breaks
        (br $continue)
      )
    )
    (local.get $counter)
  )
)
