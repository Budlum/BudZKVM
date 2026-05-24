pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod sema;

use bud_isa::IsaProfile;

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
    let mut parser = parser::Parser::new(source);

    // Parse contract, catching any recursive descent panics cleanly
    let contract_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parser.parse_contract()));

    let contract = match contract_result {
        Ok(c) => c,
        Err(err) => {
            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown parser error".to_string()
            };
            return Err(CompileError::ParserError(msg));
        }
    };

    let mut sema = sema::SemanticAnalyzer::new();
    sema.analyze(&contract)?;

    let mut codegen = codegen::Codegen::new_with_profile(profile);
    codegen.generate(&contract)
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
        let source = r#"
            contract ForTest {
                pub fn main() {
                    let sum = 0;
                    for i in 0..5 {
                        sum = sum + i;
                    }
                }
            }
        "#;

        let res = compile(source, IsaProfile::Production);
        assert!(res.is_err());
        assert!(matches!(
            res.unwrap_err(),
            CompileError::ExperimentalOpcodeDisabled(_)
        ));
    }
}
