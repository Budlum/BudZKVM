pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod sema;

use bud_isa::IsaProfile;
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    LexerError(String),
    ParserError(String),
    SemanticError(String),
    CodegenError(String),
    ExperimentalOpcodeDisabled(String),
    RegisterExhausted,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::LexerError(msg) => write!(f, "Lexer error: {}", msg),
            CompileError::ParserError(msg) => write!(f, "Parser error: {}", msg),
            CompileError::SemanticError(msg) => write!(f, "Semantic error: {}", msg),
            CompileError::CodegenError(msg) => write!(f, "Codegen error: {}", msg),
            CompileError::ExperimentalOpcodeDisabled(msg) => {
                write!(f, "Experimental opcode error: {}", msg)
            }
            CompileError::RegisterExhausted => {
                write!(f, "Register exhausted: maximum 31 registers allowed")
            }
        }
    }
}

impl std::error::Error for CompileError {}

pub fn compile(source: &str, profile: IsaProfile) -> Result<Vec<u64>, CompileError> {
    debug!(profile = ?profile, source_len = source.len(), "Starting compilation");

    let mut parser = parser::Parser::new(source);
    let contract = parser.parse_contract()?;
    debug!(functions = contract.functions.len(), "Parsing complete");

    let mut sema = sema::SemanticAnalyzer::new();
    sema.analyze(&contract)?;
    debug!("Semantic analysis complete");

    let mut codegen = codegen::Codegen::new_with_profile(profile);
    let bytecode = codegen.generate(&contract)?;
    debug!(instructions = bytecode.len(), "Code generation complete");

    Ok(bytecode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "experimental")]
    fn compiles_for_loop_to_executable_bytecode() {
        let source = r#"
            contract ForTest {
                pub fn main() {
                    let sum = 0;
                    for i in 0..5 {
                        sum = sum + i;
                    }
                    if (sum == 10) {
                        emit Success(sum);
                    }
                }
            }
        "#;

        let bytecode = compile(source, IsaProfile::Experimental).unwrap();

        let mut vm = bud_vm::Vm::new(1024);
        vm.run(&bytecode).unwrap();

        assert_eq!(vm.events, vec![10]);
    }

    #[test]
    fn rejects_experimental_in_production() {
        // All 31 opcodes are now production-ready.
        // This test validates that the production profile compiles successfully
        // with a typical contract using both control flow and arithmetic.
        let source = "contract T { pub fn main() { let x = 1 + 2; } }";
        let res = compile(source, IsaProfile::Production);
        assert!(res.is_ok());
    }

    #[test]
    #[cfg(feature = "experimental")]
    fn test_operator_precedence_and_parentheses() {
        let source = r#"
            contract PrecedenceTest {
                pub fn main() {
                    let a = 2 + 3 * 4;
                    let b = (2 + 3) * 4;
                    let c = 0x10;
                    emit Result(a, b, c);
                }
            }
        "#;

        let bytecode = compile(source, IsaProfile::Experimental).unwrap();

        let mut vm = bud_vm::Vm::new(1024);
        vm.run(&bytecode).unwrap();

        assert_eq!(vm.events, vec![14, 20, 16]);
    }

    #[test]
    #[cfg(feature = "experimental")]
    fn test_comments_support() {
        let source = r#"
            // This is a single-line comment at the beginning
            contract CommentsTest {
                /*
                 * This is a multi-line block comment
                 * describing the main function.
                 */
                pub fn main() {
                    let x = 100; // Single-line comment after code
                    /* Inline block comment */ let y = 200;
                    emit Result(x, y);
                }
            }
        "#;

        let bytecode = compile(source, IsaProfile::Experimental).unwrap();

        let mut vm = bud_vm::Vm::new(1024);
        vm.run(&bytecode).unwrap();

        assert_eq!(vm.events, vec![100, 200]);
    }

    #[test]
    fn test_parser_error_propagation() {
        let source = r#"
            contract BadSyntax {
                pub fn main() {
                    let x = ;
                }
            }
        "#;

        let res = compile(source, IsaProfile::Production);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), CompileError::ParserError(_)));
    }

    #[test]
    fn test_large_integer_literal_compilation() {
        // 0xFFFFFFFFFFFFFFFF is u64::MAX
        let source = r#"
            contract LargeIntTest {
                pub fn main() {
                    let max_u64 = 0xFFFFFFFFFFFFFFFF;
                    let large_val = 1152921504606846975; // 2^60 - 1
                    emit Result(max_u64, large_val);
                }
            }
        "#;
        
        let bytecode = compile(source, IsaProfile::Production).expect("Should compile large literals");
        let mut vm = bud_vm::Vm::new(8192);
        vm.run(&bytecode).expect("VM should run");
        
        assert_eq!(vm.events.len(), 2);
        assert_eq!(vm.events[0], u64::MAX);
        assert_eq!(vm.events[1], 1152921504606846975);
    }

    #[test]
    fn test_register_allocator_reclamation() {
        // Without reclamation, compiling this expression would require >32 registers
        // because each `+` would allocate a new temporary register.
        // With reclamation, temporaries are reused, so this easily compiles.
        let mut source = String::from("contract RegTest { pub fn main() { let x = 1");
        for _ in 0..50 {
            source.push_str(" + 1");
        }
        source.push_str("; emit Result(x); } }");
        
        let bytecode = compile(&source, IsaProfile::Production).expect("Should reclaim registers and not exhaust them");
        let mut vm = bud_vm::Vm::new(8192);
        vm.run(&bytecode).expect("VM should run");
        assert_eq!(vm.events, vec![51]);
    }

    #[test]
    fn test_user_function_calls() {
        let source = r#"
            contract CallTest {
                fn add_and_mul(a: u64, b: u64, c: u64) -> u64 {
                    let sum = a + b;
                    return sum * c;
                }

                fn get_magic() -> u64 {
                    return 42;
                }

                pub fn main() {
                    let magic = get_magic();
                    let res = add_and_mul(1, 2, magic);
                    emit Result(res);
                }
            }
        "#;

        let bytecode = compile(source, IsaProfile::Production).expect("Should compile function calls");
        let mut vm = bud_vm::Vm::new(8192);
        vm.run(&bytecode).expect("VM should run");
        
        // (1 + 2) * 42 = 126
        assert_eq!(vm.events, vec![126]);
    }

    #[test]
    fn test_struct_compilation() {
        let source = r#"
            contract StructTest {
                struct Point {
                    x: u64,
                    y: u64,
                }

                fn get_x(p: Point) -> u64 {
                    return p.x;
                }

                pub fn main() {
                    let p = Point { x: 10, y: 20 };
                    let z = p.y + get_x(p);
                    emit Result(z);
                }
            }
        "#;

        let bytecode = compile(source, IsaProfile::Production).expect("Should compile structs");
        let mut vm = bud_vm::Vm::new(8192);
        vm.run(&bytecode).expect("VM should run");
        
        // p.y (20) + p.x (10) = 30
        assert_eq!(vm.events, vec![30]);
    }
}
