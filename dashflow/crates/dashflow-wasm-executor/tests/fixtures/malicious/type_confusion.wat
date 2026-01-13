;; Type confusion attempts
;; Tests type safety in WASM
(module
  ;; Function that expects i32 but might be called with wrong type
  (func $expects_i32 (export "expects_i32") (param $x i32) (result i32)
    (i32.add (local.get $x) (i32.const 10))
  )

  ;; Function with multiple param types
  (func $mixed_types (export "mixed_types") (param $a i32) (param $b i64) (result i32)
    ;; This should fail if types are confused
    (i32.wrap_i64 (local.get $b))
  )
)
