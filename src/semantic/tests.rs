use super::*;
use crate::parser::Parser;
use std::error::Error;

#[test]
fn test_redefine_variable_would_get_error() {
    let code = "\
    x: int = 1;
    x: int = 2;
    ";
    let err = check_code(code).unwrap_err();
    assert_eq!(err.description(), "name redefined");
}

#[test]
fn test_function_and_variable_use_the_same_space() {
    let code = "\
    x: int = 1;
    x(): void {}
    ";
    let err = check_code(code).unwrap_err();
    assert_eq!(err.description(), "name redefined");
}

#[test]
fn test_type_mismatched() {
    let code = "\
    x: int = \"str\";
    ";
    let err = check_code(code).unwrap_err();
    assert_eq!(err.description(), "type mismatched");
}

#[test]
fn test_check_function_call() {
    let code = "\
    x(a: int): int = a;
    y: int = x(2);
    ";
    let result = check_code(code);
    assert_eq!(result.is_ok(), true);
}

#[test]
fn test_unify_list_type() {
    let code = "\
    x: List[int] = [1, 2, 3];
    ";
    let result = check_code(code);
    assert_eq!(result.is_ok(), true);
}

#[test]
fn test_unify_free_var() {
    let code = "\
    x: List[int] = [];
    ";
    let result = check_code(code);
    assert_eq!(result.is_ok(), true);
}

#[test]
fn test_check_return_nothing() {
    let code = "\
    x(): void {
      return;
    }
    ";
    let result = check_code(code);
    assert_eq!(result.is_ok(), true);
}

#[test]
fn test_check_local_variable_define() {
    let code = "\
    x(): int {
      y: int = 1;
      return y;
    }
    ";
    let result = check_code(code);
    assert_eq!(result.is_ok(), true);
}

// helpers, must put tests before this line
fn check_code(code: &'static str) -> Result<()> {
    let program = Parser::parse_program("", code).unwrap();
    let mut checker = SemanticChecker::new();
    checker.check_program(program)
}
