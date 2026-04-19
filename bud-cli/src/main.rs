use clap::{Parser, Subcommand};
use bud_vm::Vm;
use bud_isa::{Instruction, Opcode};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(short, long)]
        program: String,
        #[arg(short, long)]
        sender: Option<u64>,
        #[arg(short, long)]
        block_height: Option<u64>,
        #[arg(short, long)]
        args: Vec<u64>,
    },
    Batch {
        #[arg(short, long)]
        programs: Vec<String>,
        #[arg(short, long)]
        sender: Option<u64>,
        #[arg(short, long)]
        block_height: Option<u64>,
        #[arg(short, long)]
        args: Vec<u64>,
    },
    Test,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Run { program, sender, block_height, args } => {
            let content = std::fs::read_to_string(program).expect("Failed to read file");
            let mut parser = bud_compiler::parser::Parser::new(&content);
            let contract = parser.parse_contract();
            
            let mut sema = bud_compiler::sema::SemanticAnalyzer::new();
            sema.analyze(&contract);

            let state = bud_state::State::new();
            println!("Pre-state Root: {:?}", state.root());

            let mut codegen = bud_compiler::codegen::Codegen::new();
            let bytecode = codegen.generate(&contract);
            
            println!("Generated {} instructions", bytecode.len());

            let mut vm = bud_vm::Vm::new(1024);
            if let Some(s) = *sender { vm.context.sender = s; }
            if let Some(bh) = *block_height { vm.context.block_height = bh; }
            
            for (i, val) in args.iter().enumerate() {
                if i < 31 {
                    vm.registers[i + 1] = *val;
                }
            }
            
            vm.run(&bytecode);

            println!("Execution Trace (Steps: {})", vm.trace.len());
            for (i, step) in vm.trace.iter().enumerate() {
                println!("Step {}: PC={} OP={:?} R1={}", i, step.pc, step.instruction.opcode, step.registers[1]);
            }
            
            println!("Emitted Events: {:?}", vm.events);
            
            let matrix = bud_proof::Prover::generate_matrix(&vm.trace);
            bud_proof::Prover::prove(&matrix);
        }
        Commands::Batch { programs, sender, block_height, args } => {
            println!("Processing block with {} transactions...", programs.len());
            let mut all_proofs = Vec::new();
            
            for p in programs {
                let content = std::fs::read_to_string(p).expect("Failed to read file");
                let mut parser = bud_compiler::parser::Parser::new(&content);
                let contract = parser.parse_contract();
                let mut codegen = bud_compiler::codegen::Codegen::new();
                let bytecode = codegen.generate(&contract);
                
                let mut vm = bud_vm::Vm::new(1024);
                if let Some(s) = *sender { vm.context.sender = s; }
                if let Some(bh) = *block_height { vm.context.block_height = bh; }
                
                vm.run(&bytecode);
                
                let matrix = bud_proof::Prover::generate_matrix(&vm.trace);
                let proof = bud_proof::Prover::prove(&matrix);
                all_proofs.push(proof);
            }
            
            let final_proof = bud_proof::RecursiveProver::aggregate(&all_proofs);
            println!("Final Block Proof Hash: {:?}", final_proof.data);
        }
        Commands::Test => {
            let mut vm = Vm::new(1024);
            let prog = vec![
                Instruction { opcode: Opcode::Add, rd: 1, rs1: 2, rs2: 3, imm: 0 }.encode(),
                Instruction { opcode: Opcode::Halt, rd: 0, rs1: 0, rs2: 0, imm: 0 }.encode(),
            ];
            vm.registers[2] = 10;
            vm.registers[3] = 20;
            vm.run(&prog);
            println!("Register 1: {}", vm.registers[1]);
        }
    }
}
