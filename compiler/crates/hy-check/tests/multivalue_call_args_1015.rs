//! multivalue_call_args_1015.rs — regression tests for issue #1015.
//!
//! A lone multi-value call used as the whole argument list expands to one
//! argument per result (`g(two())`). Before the fix three paths disagreed:
//! the argument *type* checks returned early on multi-value (accepting
//! `takes_ints(returns_strings())`, whose Go does not build), while the method
//! and local-closure *arity* checks counted it as one argument (falsely
//! rejecting the valid `s.takes_two(two())`).

use hy_check::{check_file, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

fn check(src: &str) -> hy_check::CheckResult {
    let path = "inline.hy";
    let ast = parse_file(path, src)
        .unwrap_or_else(|diags| panic!("must parse (parser contract): {:?}", diags));
    check_file(&ast)
}

fn codes(r: &hy_check::CheckResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.clone())
        .collect()
}

fn assert_accept(src: &str, what: &str) {
    let r = check(src);
    assert_eq!(
        r.verdict,
        Verdict::Accept,
        "expected ACCEPT ({}), got {:?} with {:?}",
        what,
        r.verdict,
        codes(&r)
    );
}

fn assert_e1002(src: &str, what: &str) {
    let r = check(src);
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E1002"),
        "expected HYB-E1002 ({}), got {:?}",
        what,
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject, "{}", what);
}

// --- soundness: multi-value results must match the parameter types ---------

#[test]
fn multivalue_argument_types_are_checked() {
    assert_e1002(
        r#"
fn takes_ints(a int, b int) {}
fn returns_strings() (string, string) { return "a", "b" }
fn main() {
    takes_ints(returns_strings())
}
"#,
        "string results into int parameters",
    );
}

#[test]
fn multivalue_argument_types_are_checked_at_task_spawn() {
    assert_e1002(
        r#"
fn takes_ints(a int, b int) {}
fn returns_strings() (string, string) { return "a", "b" }
fn main() {
    task takes_ints(returns_strings())
    for i in 0..1 {}
}
"#,
        "same bypass at a task spawn site",
    );
}

#[test]
fn task_local_closure_shadowing_top_level_function_uses_local_signature() {
    // `task` on a local closure is itself rejected by the move/capture rules
    // (HYB-E3001) — that rule is unrelated to argument types. What this pins is
    // that the callee resolves to the local closure signature, so the typed
    // expanded-argument check does not compare int results against the shadowed
    // top-level `cb(a string, b string)` and add a spurious HYB-E1002.
    let r = check(
        r#"
fn cb(a string, b string) {}
fn two() (int, int) { return 1, 2 }
fn main() {
    cb := fn(a int, b int) {}
    task cb(two())
    for i in 0..1 {}
}
"#,
    );
    let codes = codes(&r);
    assert!(
        !codes.iter().any(|c| c == "HYB-E1002"),
        "task callee must use the shadowing local signature, got {codes:?}"
    );
}

#[test]
fn task_local_closure_shadowing_top_level_function_checks_local_types() {
    assert_e1002(
        r#"
fn cb(a int, b int) {}
fn two_ints() (int, int) { return 1, 2 }
fn main() {
    cb := fn(a string, b string) {}
    task cb(two_ints())
    for i in 0..1 {}
}
"#,
        "task callee rejects results mismatching the shadowing local closure",
    );
}

#[test]
fn multivalue_argument_types_are_checked_on_methods() {
    assert_e1002(
        r#"
struct st {}
fn (s st) takes_ints(a int, b int) {}
fn returns_strings() (string, string) { return "a", "b" }
fn main() {
    var s st
    s.takes_ints(returns_strings())
}
"#,
        "string results into int method parameters",
    );
}

#[test]
fn multivalue_argument_types_are_checked_on_closures() {
    assert_e1002(
        r#"
fn returns_strings() (string, string) { return "a", "b" }
fn main() {
    f := fn(a int, b int) {}
    f(returns_strings())
}
"#,
        "string results into int closure parameters",
    );
}

#[test]
fn multivalue_argument_types_are_checked_on_function_typed_fields() {
    assert_e1002(
        r#"
struct holder {
    callback fn(a int, b int)
}
fn returns_strings() (string, string) { return "a", "b" }
fn main() {
    var h holder
    h.callback(returns_strings())
}
"#,
        "string results into int function-field parameters",
    );
}

#[test]
fn multivalue_typed_int_results_do_not_match_float_parameters() {
    assert_e1002(
        r#"
fn takes_floats(a float64, b float64) {}
fn two() (int, int) { return 1, 2 }
fn main() {
    takes_floats(two())
}
"#,
        "typed int call results into float parameters",
    );
}

#[test]
fn multivalue_typed_int_results_do_not_match_local_float_parameters() {
    assert_e1002(
        r#"
fn two() (int, int) { return 1, 2 }
fn main() {
    f := fn(a float64, b float64) {}
    f(two())
}
"#,
        "typed int call results into local closure float parameters",
    );
}

#[test]
fn multivalue_typed_int_results_do_not_match_method_float_parameters() {
    assert_e1002(
        r#"
struct st {}
fn (s st) takes_floats(a float64, b float64) {}
fn two() (int, int) { return 1, 2 }
fn main() {
    var s st
    s.takes_floats(two())
}
"#,
        "typed int call results into method float parameters",
    );
}

#[test]
fn multivalue_typed_int_results_do_not_match_function_field_float_parameters() {
    assert_e1002(
        r#"
struct holder {
    callback fn(a float64, b float64)
}
fn two() (int, int) { return 1, 2 }
fn main() {
    var h holder
    h.callback(two())
}
"#,
        "typed int call results into function-field float parameters",
    );
}

#[test]
fn task_multivalue_typed_int_results_do_not_match_float_parameters() {
    assert_e1002(
        r#"
fn takes_floats(a float64, b float64) {}
fn two() (int, int) { return 1, 2 }
fn main() {
    task takes_floats(two())
    for i in 0..1 {}
}
"#,
        "typed int call results into task float parameters",
    );
}

#[test]
fn multivalue_typed_nested_int_results_do_not_match_float_slice_parameters() {
    assert_e1002(
        r#"
fn takes_float_slices(a []float64, b []float64) {}
fn two_int_slices() ([]int, []int) { return nil, nil }
fn main() {
    takes_float_slices(two_int_slices())
}
"#,
        "typed int slice elements into float slice parameters",
    );
}

#[test]
fn untyped_int_constant_remains_assignable_to_float_parameter() {
    assert_accept(
        r#"
fn takes_float(a float64) {}
fn main() {
    takes_float(1)
}
"#,
        "an untyped int constant adapts to a float parameter",
    );
}

// --- precision: valid multi-value calls must not be rejected ---------------

#[test]
fn multivalue_into_matching_parameters_is_accepted() {
    assert_accept(
        r#"
fn takes_strs(a string, b string) {}
fn returns_strings() (string, string) { return "a", "b" }
fn main() {
    takes_strs(returns_strings())
}
"#,
        "two string results into two string parameters",
    );
}

#[test]
fn multivalue_into_matching_method_parameters_is_accepted() {
    assert_accept(
        r#"
struct st {}
fn (s st) takes_two(a int, b int) {}
fn two() (int, int) { return 1, 2 }
fn main() {
    var s st
    s.takes_two(two())
}
"#,
        "method called with a lone multi-value call",
    );
}

#[test]
fn multivalue_into_matching_closure_parameters_is_accepted() {
    assert_accept(
        r#"
fn two() (int, int) { return 1, 2 }
fn main() {
    f := fn(a int, b int) {}
    f(two())
}
"#,
        "closure called with a lone multi-value call",
    );
}

#[test]
fn closure_shadowing_top_level_fn_uses_local_arity_and_types() {
    // The shadowing local binds `cb` for the whole block, so the top-level
    // `cb(a int)` must not judge this call: the local two-parameter closure
    // accepts both expanded results.
    assert_accept(
        r#"
fn cb(a int) {}
fn two() (int, int) { return 1, 2 }
fn main() {
    cb := fn(a int, b int) {}
    cb(two())
}
"#,
        "local closure shadowing a top-level fn owns the call's arity",
    );
}

#[test]
fn multivalue_into_too_few_closure_parameters_rejects() {
    assert_e1002(
        r#"
fn two() (int, int) { return 1, 2 }
fn main() {
    f := fn(a int) {}
    f(two())
}
"#,
        "two results into a one-parameter closure",
    );
}

#[test]
fn task_multivalue_into_too_few_parameters_rejects() {
    assert_e1002(
        r#"
fn one(a int) {}
fn two() (int, int) { return 1, 2 }
fn main() {
    task one(two())
    for i in 0..1 {}
}
"#,
        "task spawn with an expanded argument list that exceeds the callee's arity",
    );
}

#[test]
fn generic_fn_binding_in_array_literal_is_not_rejected() {
    // A generic function's parameter kind resolves to `Unknown`, while the
    // declared array element type has a concrete parameter kind. Function
    // signatures are compared at call sites, so element compatibility must
    // stay permissive here.
    assert_accept(
        r#"
fn sink[t any](x t) {}
fn main() {
    fns := [1]fn(x int){sink}
    fns[0](1)
}
"#,
        "generic function as an array-literal element",
    );
}

#[test]
fn wrong_arity_fn_in_array_literal_rejects() {
    assert_e1002(
        r#"
fn wrong(a int, b int) {}
fn main() {
    fns := [1]fn(x int){wrong}
    _ = fns
}
"#,
        "two-parameter function as a one-parameter array element",
    );
}

#[test]
fn expanded_function_valued_result_checks_callback_signature() {
    assert_e1002(
        r#"
fn bad(x string) {}
fn pair() (fn(x string), int) { return bad, 1 }
fn takes(cb fn(x int), n int) {}
fn main() {
    takes(pair())
}
"#,
        "callback parameter with a provably different signature",
    );
}

#[test]
fn indexed_function_value_expands_its_results() {
    assert_accept(
        r#"
fn two() (int, int) { return 1, 2 }
fn main() {
    fns := []fn() int int{two}
    cb := fn(a int, b int) {}
    cb(fns[0]())
}
"#,
        "indexed callee whose function kind declares two results",
    );
}

#[test]
fn expanded_interface_results_do_not_become_concrete_arguments() {
    assert_e1002(
        r#"
fn pair() (any, any) { return 1, 2 }
fn takes(a int, b int) {}
fn main() {
    takes(pair())
}
"#,
        "typed interface results passed to concrete parameters",
    );
}

#[test]
fn expanded_callback_signature_uses_typed_numeric_rules() {
    assert_e1002(
        r#"
fn cb(x int) {}
fn pair() (fn(x int), int) { return cb, 1 }
fn takes(f fn(x float64), n int) {}
fn main() {
    takes(pair())
}
"#,
        "callback parameter whose typed int parameter differs from float64",
    );
}
