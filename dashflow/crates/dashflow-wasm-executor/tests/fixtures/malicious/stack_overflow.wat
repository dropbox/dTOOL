;; Stack overflow: Attempts to overflow the stack
;; Tests stack limit enforcement
(module
  ;; Deep recursion without base case check
  (func $deep_recursion (export "deep_recursion") (param $n i32) (result i32)
    ;; Minimal base case - will recurse 1000000 times
    (if (result i32) (i32.eq (local.get $n) (i32.const 0))
      (then
        (i32.const 1)
      )
      (else
        (i32.add
          (i32.const 1)
          (call $deep_recursion (i32.sub (local.get $n) (i32.const 1)))
        )
      )
    )
  )
)
