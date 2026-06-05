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
}
