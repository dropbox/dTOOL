;; Division by zero
;; Tests trap handling for arithmetic errors
(module
  (func $divide_by_zero (export "divide_by_zero") (result i32)
    (i32.div_s (i32.const 42) (i32.const 0))
  )

  (func $modulo_by_zero (export "modulo_by_zero") (result i32)
    (i32.rem_s (i32.const 42) (i32.const 0))
  )
)
