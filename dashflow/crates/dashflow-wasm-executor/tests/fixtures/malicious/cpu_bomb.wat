;; CPU bomb: Consumes excessive CPU via deep recursion
;; Tests fuel limit enforcement
(module
  ;; Recursive function that consumes lots of CPU
  (func $recursive (export "recursive") (param $n i32) (result i32)
    (if (result i32) (i32.gt_u (local.get $n) (i32.const 0))
      (then
        ;; Recursive call
        (i32.add
          (local.get $n)
          (call $recursive (i32.sub (local.get $n) (i32.const 1)))
        )
      )
      (else
        (i32.const 0)
      )
    )
  )

  ;; Exponential complexity function
  (func $fibonacci (export "fibonacci") (param $n i32) (result i32)
    (if (result i32) (i32.le_u (local.get $n) (i32.const 1))
      (then
        (local.get $n)
      )
      (else
        (i32.add
          (call $fibonacci (i32.sub (local.get $n) (i32.const 1)))
          (call $fibonacci (i32.sub (local.get $n) (i32.const 2)))
        )
      )
    )
  )
)
