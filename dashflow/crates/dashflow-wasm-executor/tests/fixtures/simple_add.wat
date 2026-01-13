(module
  ;; Simple add function: takes two i32 params, returns i32
  (func $add (export "add") (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add
  )

  ;; Multiply function
  (func $multiply (export "multiply") (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.mul
  )

  ;; Function that returns a constant
  (func $get_constant (export "get_constant") (result i32)
    i32.const 42
  )
)
