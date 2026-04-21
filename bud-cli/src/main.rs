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
        nonce: Option<u64>,
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
        nonce: Option<u64>,
        #[arg(short, long)]
        block_height: Option<u64>,
        #[arg(short, long)]
        args: Vec<u64>,
    },
    Deploy {
        #[arg(short, long)]
        program: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    Call {
        #[arg(short, long)]
        bytecode: String,
        #[arg(short, long)]
        sender: Option<u64>,
        #[arg(short, long)]
        nonce: Option<u64>,
        #[arg(short, long)]
        args: Vec<u64>,
    },
    Verify {
        #[arg(short, long)]
        proof_file: String,
    },
    Test,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Run { program, sender, nonce, block_height, args } => {
            let content = std::fs::read_to_string(program).expect("Failed to read file");
            let mut parser = bud_compiler::parser::Parser::new(&content);
            let contract = parser.parse_contract();
            
            let mut sema = bud_compiler::sema::SemanticAnalyzer::new();
            sema.analyze(&contract);

            let mut state = bud_state::State::load("state.json");
            println!("Pre-state Root: {:?}", state.root());

            let mut codegen = bud_compiler::codegen::Codegen::new();
            let bytecode = codegen.generate(&contract);
            
            println!("Generated {} instructions", bytecode.len());

            let mut vm = bud_vm::Vm::new(1024);
            if let Some(s) = *sender { 
                vm.context.sender = s;
                let acc = state.accounts.entry(s).or_insert(bud_state::Account { balance: 1000, nonce: 0 });
                vm.context.nonce = acc.nonce;
            }
            if let Some(n) = *nonce { vm.context.nonce = n; }
            if let Some(bh) = *block_height { vm.context.block_height = bh; }
            
            for (i, val) in args.iter().enumerate() {
                if i < 31 {
                    vm.registers[i + 1] = *val;
                }
            }
            
            vm.run(&bytecode);
            
            if let Some(s) = *sender {
                let acc = state.accounts.get_mut(&s).unwrap();
                acc.nonce += 1;
            }
            state.save();

            println!("Execution Trace (Steps: {})", vm.trace.len());
            for (i, step) in vm.trace.iter().enumerate() {
                println!("Step {}: PC={} OP={:?} R1={}", i, step.pc, step.instruction.opcode, step.registers[1]);
            }
            
            println!("Emitted Events: {:?}", vm.events);
            
            let num_steps = vm.trace.len();
            let matrix = bud_proof::Prover::generate_matrix(&vm.trace);
            let proof = bud_proof::Prover::prove(&matrix, num_steps);
            bud_proof::Verifier::verify(&proof, num_steps);

            println!("Post-state Root: {:?}", state.root());
        }
        Commands::Batch { programs, sender, nonce, block_height, args } => {
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
                if let Some(n) = *nonce { vm.context.nonce = n; }
                if let Some(bh) = *block_height { vm.context.block_height = bh; }
                
                vm.run(&bytecode);
                
                let matrix = bud_proof::Prover::generate_matrix(&vm.trace);
                let proof = bud_proof::Prover::prove(&matrix, vm.trace.len());
                all_proofs.push(proof);
            }
            
            let final_proof = bud_proof::RecursiveProver::aggregate(&all_proofs);
            println!("Final Block Proof Hash: {:?}", final_proof.data);
        }
        Commands::Deploy { program, output } => {
            let content = std::fs::read_to_string(program).expect("Failed to read file");
            let mut parser = bud_compiler::parser::Parser::new(&content);
            let contract = parser.parse_contract();
            let mut codegen = bud_compiler::codegen::Codegen::new();
            let bytecode = codegen.generate(&contract);
            
            let out_name = output.clone().unwrap_or_else(|| format!("{}.budc", program));
            let bytes: Vec<u8> = bytecode.iter().flat_map(|&b| b.to_le_bytes().to_vec()).collect();
            std::fs::write(&out_name, bytes).expect("Failed to write bytecode");
            println!("Contract deployed to: {}", out_name);
        }
        Commands::Call { bytecode, sender, nonce, args } => {
            let bytes = std::fs::read(&bytecode).expect("Failed to read bytecode");
            let mut prog = Vec::new();
            for chunk in bytes.chunks_exact(8) {
                let mut b = [0u8; 8];
                b.copy_from_slice(chunk);
                prog.push(u64::from_le_bytes(b));
            }
            
            let mut state = bud_state::State::load("state.json");
            println!("Pre-state Root: {:?}", state.root());

            let mut vm = bud_vm::Vm::new(1024);
            if let Some(s) = *sender { 
                vm.context.sender = s;
                let acc = state.accounts.entry(s).or_insert(bud_state::Account { balance: 1000, nonce: 0 });
                vm.context.nonce = acc.nonce;
            }
            if let Some(n) = *nonce { vm.context.nonce = n; }
            
            for (i, val) in args.iter().enumerate() {
                if i < 31 { vm.registers[i + 1] = *val; }
            }
            
            vm.run(&prog);

            if let Some(s) = *sender {
                let acc = state.accounts.get_mut(&s).unwrap();
                acc.nonce += 1;
            }
            state.save();
            println!("Execution Trace (Steps: {})", vm.trace.len());
            println!("Emitted Events: {:?}", vm.events);
            
            let num_steps = vm.trace.len();
            let matrix = bud_proof::Prover::generate_matrix(&vm.trace);
            let proof = bud_proof::Prover::prove(&matrix, num_steps);
            bud_proof::Verifier::verify(&proof, num_steps);
        }
        Commands::Verify { proof_file } => {
            let data = std::fs::read(proof_file).expect("Failed to read proof file");
            let proof = bud_proof::Proof { data };
            let valid = bud_proof::Verifier::verify(&proof, 0);
            if valid {
                println!("Result: VALID");
            } else {
                println!("Result: INVALID");
                std::process::exit(1);
            }
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
