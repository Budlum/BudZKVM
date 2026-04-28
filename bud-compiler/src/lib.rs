pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod sema;

#[cfg(test)]
mod tests {
    use super::{codegen::Codegen, parser::Parser, sema::SemanticAnalyzer};

    #[test]
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

        let mut parser = Parser::new(source);
        let contract = parser.parse_contract();
        let mut sema = SemanticAnalyzer::new();
        sema.analyze(&contract);

        let mut codegen = Codegen::new();
        let bytecode = codegen.generate(&contract);

        let mut vm = bud_vm::Vm::new(1024);
        vm.run(&bytecode);

        assert_eq!(vm.events, vec![10]);
    }
}
