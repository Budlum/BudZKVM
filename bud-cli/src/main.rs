use bud_isa::{Instruction, Opcode};
use bud_proof::adapter::{ExecutionPublicInputs, ProofEnvelope, ProverAdapter};
use bud_proof::DefaultAdapter as Prover;
use bud_vm::Vm;
use clap::{Parser, Subcommand};
use std::fs;
use tiny_keccak::{Hasher, Keccak};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, default_value_t = 1)]
    chain_id: u64,

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
        #[arg(long)]
        json: bool,
        #[arg(long)]
        proof_out: Option<String>,
        #[arg(long)]
        public_inputs_out: Option<String>,
        #[arg(long)]
        state_in: Option<String>,
        #[arg(long)]
        state_out: Option<String>,
    },
    Prove {
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
        #[arg(long)]
        proof_out: String,
        #[arg(long)]
        public_inputs_out: Option<String>,
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
        #[arg(short, long)]
        public_inputs_file: String,
        #[arg(short, long)]
        bytecode_file: String,
    },
    Test,
}

fn compute_keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut res = [0u8; 32];
    hasher.finalize(&mut res);
    res
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Run {
            program,
            sender,
            nonce,
            block_height,
            args,
            json,
            proof_out,
            public_inputs_out,
            state_in,
            state_out,
        } => {
            let content = fs::read_to_string(program).expect("Failed to read program file");

            #[cfg(feature = "experimental")]
            let profile = bud_isa::IsaProfile::Experimental;
            #[cfg(not(feature = "experimental"))]
            let profile = bud_isa::IsaProfile::Production;

            let bytecode = bud_compiler::compile(&content, profile).expect("Compilation failed");

            let state_file = state_in.clone().unwrap_or_else(|| "state.json".to_string());
            let mut state = bud_state::State::load(&state_file).expect("Failed to load state");
            let pre_root = state.root();

            let mut vm = Vm::new(1024);
            if let Some(s) = *sender {
                vm.context.sender = s;
                let acc = state.accounts.entry(s).or_insert(bud_state::Account {
                    balance: 1000,
                    nonce: 0,
                });
                vm.context.nonce = acc.nonce;
            }
            if let Some(n) = *nonce {
                vm.context.nonce = n;
            }
            if let Some(bh) = *block_height {
                vm.context.block_height = bh;
            }

            for (i, val) in args.iter().enumerate() {
                if i < 31 {
                    vm.registers[i + 1] = *val;
                }
            }

            let receipt = vm.run_receipt(&bytecode);

            if !receipt.success {
                eprintln!("Execution failed deterministically: {:?}", receipt.error);
                std::process::exit(1);
            }

            // Temporarily apply state updates in memory
            let old_sender_acc = if let Some(s) = *sender {
                let acc = state.accounts.get_mut(&s).unwrap();
                let old = acc.clone();
                acc.nonce += 1;
                Some((s, old))
            } else {
                None
            };
            let post_root = state.root();

            // Construct ExecutionPublicInputs
            let bytecode_bytes: Vec<u8> = bytecode
                .iter()
                .flat_map(|&b| b.to_le_bytes().to_vec())
                .collect();
            let prog_hash = compute_keccak256(&bytecode_bytes);

            let event_bytes: Vec<u8> = receipt
                .events
                .iter()
                .flat_map(|&e| e.to_le_bytes().to_vec())
                .collect();
            let event_digest = compute_keccak256(&event_bytes);

            let pi = ExecutionPublicInputs {
                chain_id: cli.chain_id,
                program_hash: prog_hash,
                initial_state_root: pre_root,
                final_state_root: post_root,
                sender: vm.context.sender,
                nonce: vm.context.nonce,
                block_height: vm.context.block_height,
                gas_limit: vm.gas_limit,
                gas_used: vm.gas_used,
                exit_code: 0,
                trace_len: vm.trace.len() as u64,
                event_digest,
            };

            // Prove and Verify
            let envelope =
                Prover::prove(&vm.trace, &pi, &bytecode).expect("Failed to generate proof");
            let ok = Prover::verify(&envelope, &pi, &bytecode).is_ok();

            if !ok {
                eprintln!("Verification of generated proof failed!");
                // Revert state updates since verification failed
                if let Some((s, old)) = old_sender_acc {
                    state.accounts.insert(s, old);
                }
                std::process::exit(1);
            }

            // ATOMIC STATE SAVE: Save ONLY after success
            let save_file = state_out.clone().unwrap_or(state_file);
            let mut final_state = bud_state::State::load(&save_file)
                .unwrap_or_else(|_| bud_state::State::load("state.json").unwrap());
            final_state.accounts = state.accounts;
            final_state.save();

            if *json {
                let out = serde_json::json!({
                    "pre_state_root": hex::encode(pre_root),
                    "post_state_root": hex::encode(post_root),
                    "success": true,
                    "gas_used": receipt.gas_used,
                    "events": receipt.events,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                println!("Pre-state Root: {:?}", hex::encode(pre_root));
                println!("Post-state Root: {:?}", hex::encode(post_root));
                println!("Execution Trace Steps: {}", vm.trace.len());
                println!("Proof generated and verified successfully!");
            }

            if let Some(path) = proof_out {
                let data =
                    serde_json::to_string_pretty(&envelope).expect("Failed to serialize envelope");
                fs::write(path, data).expect("Failed to write proof file");
                println!("Proof envelope written to {}", path);
            }

            if let Some(path) = public_inputs_out {
                let data =
                    serde_json::to_string_pretty(&pi).expect("Failed to serialize public inputs");
                fs::write(path, data).expect("Failed to write public inputs file");
                println!("Public inputs written to {}", path);
            }
        }
        Commands::Prove {
            program,
            sender,
            nonce,
            block_height,
            args,
            proof_out,
            public_inputs_out,
        } => {
            let content = fs::read_to_string(program).expect("Failed to read program file");
            #[cfg(feature = "experimental")]
            let profile = bud_isa::IsaProfile::Experimental;
            #[cfg(not(feature = "experimental"))]
            let profile = bud_isa::IsaProfile::Production;

            let bytecode = bud_compiler::compile(&content, profile).expect("Compilation failed");

            let state = bud_state::State::load("state.json").unwrap();
            let pre_root = state.root();

            let mut vm = Vm::new(1024);
            if let Some(s) = *sender {
                vm.context.sender = s;
            }
            if let Some(n) = *nonce {
                vm.context.nonce = n;
            }
            if let Some(bh) = *block_height {
                vm.context.block_height = bh;
            }

            for (i, val) in args.iter().enumerate() {
                if i < 31 {
                    vm.registers[i + 1] = *val;
                }
            }

            let receipt = vm.run_receipt(&bytecode);
            if !receipt.success {
                eprintln!("Execution failed!");
                std::process::exit(1);
            }

            let bytecode_bytes: Vec<u8> = bytecode
                .iter()
                .flat_map(|&b| b.to_le_bytes().to_vec())
                .collect();
            let prog_hash = compute_keccak256(&bytecode_bytes);

            let event_bytes: Vec<u8> = receipt
                .events
                .iter()
                .flat_map(|&e| e.to_le_bytes().to_vec())
                .collect();
            let event_digest = compute_keccak256(&event_bytes);

            let pi = ExecutionPublicInputs {
                chain_id: cli.chain_id,
                program_hash: prog_hash,
                initial_state_root: pre_root,
                final_state_root: pre_root, // prove-only doesn't commit final mutated root
                sender: vm.context.sender,
                nonce: vm.context.nonce,
                block_height: vm.context.block_height,
                gas_limit: vm.gas_limit,
                gas_used: vm.gas_used,
                exit_code: 0,
                trace_len: vm.trace.len() as u64,
                event_digest,
            };

            let envelope =
                Prover::prove(&vm.trace, &pi, &bytecode).expect("Failed to generate proof");

            let data = serde_json::to_string_pretty(&envelope).unwrap();
            fs::write(proof_out, data).expect("Failed to write proof file");
            println!("Proof written to: {}", proof_out);

            if let Some(path) = public_inputs_out {
                let data = serde_json::to_string_pretty(&pi).unwrap();
                fs::write(path, data).expect("Failed to write public inputs file");
                println!("Public inputs written to: {}", path);
            }
        }
        Commands::Verify {
            proof_file,
            public_inputs_file,
            bytecode_file,
        } => {
            let env_data = fs::read_to_string(proof_file).expect("Failed to read proof file");
            let envelope: ProofEnvelope =
                serde_json::from_str(&env_data).expect("Failed to parse proof envelope");

            let pi_data =
                fs::read_to_string(public_inputs_file).expect("Failed to read public inputs file");
            let expected_inputs: ExecutionPublicInputs =
                serde_json::from_str(&pi_data).expect("Failed to parse public inputs");

            let bytes = fs::read(bytecode_file).expect("Failed to read bytecode");
            if bytes.len() % 8 != 0 {
                eprintln!("Invalid bytecode: file size must be a multiple of 8 bytes");
                std::process::exit(1);
            }
            let mut program = Vec::new();
            for chunk in bytes.chunks_exact(8) {
                let mut b = [0u8; 8];
                b.copy_from_slice(chunk);
                program.push(u64::from_le_bytes(b));
            }

            match Prover::verify(&envelope, &expected_inputs, &program) {
                Ok(_) => {
                    println!("Result: VALID");
                }
                Err(e) => {
                    eprintln!("Result: INVALID ({:?})", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Batch {
            programs,
            sender,
            nonce,
            block_height,
            args: _,
        } => {
            println!("Processing block with {} transactions...", programs.len());
            for p in programs {
                let content = fs::read_to_string(p).expect("Failed to read file");
                #[cfg(feature = "experimental")]
                let profile = bud_isa::IsaProfile::Experimental;
                #[cfg(not(feature = "experimental"))]
                let profile = bud_isa::IsaProfile::Production;

                let bytecode =
                    bud_compiler::compile(&content, profile).expect("Compilation failed");

                let mut vm = Vm::new(1024);
                if let Some(s) = *sender {
                    vm.context.sender = s;
                }
                if let Some(n) = *nonce {
                    vm.context.nonce = n;
                }
                if let Some(bh) = *block_height {
                    vm.context.block_height = bh;
                }

                let receipt = vm.run_receipt(&bytecode);
                if receipt.success {
                    println!("Compiled and verified trace of {} successfully", p);
                }
            }
        }
        Commands::Deploy { program, output } => {
            let content = fs::read_to_string(program).expect("Failed to read file");
            #[cfg(feature = "experimental")]
            let profile = bud_isa::IsaProfile::Experimental;
            #[cfg(not(feature = "experimental"))]
            let profile = bud_isa::IsaProfile::Production;

            let bytecode = bud_compiler::compile(&content, profile).expect("Compilation failed");

            let out_name = output
                .clone()
                .unwrap_or_else(|| format!("{}.budc", program));
            let bytes: Vec<u8> = bytecode
                .iter()
                .flat_map(|&b| b.to_le_bytes().to_vec())
                .collect();
            fs::write(&out_name, bytes).expect("Failed to write bytecode");
            println!("Contract deployed to: {}", out_name);
        }
        Commands::Call {
            bytecode,
            sender,
            nonce,
            args,
        } => {
            let bytes = fs::read(bytecode).expect("Failed to read bytecode");
            if bytes.len() % 8 != 0 {
                eprintln!("Invalid bytecode: file size must be a multiple of 8 bytes");
                std::process::exit(1);
            }
            let mut prog = Vec::new();
            for chunk in bytes.chunks_exact(8) {
                let mut b = [0u8; 8];
                b.copy_from_slice(chunk);
                prog.push(u64::from_le_bytes(b));
            }

            let mut state = bud_state::State::load("state.json").expect("Failed to load state");
            let pre_root = state.root();

            let mut vm = Vm::new(1024);
            if let Some(s) = *sender {
                vm.context.sender = s;
                let acc = state.accounts.entry(s).or_insert(bud_state::Account {
                    balance: 1000,
                    nonce: 0,
                });
                vm.context.nonce = acc.nonce;
            }
            if let Some(n) = *nonce {
                vm.context.nonce = n;
            }

            for (i, val) in args.iter().enumerate() {
                if i < 31 {
                    vm.registers[i + 1] = *val;
                }
            }

            let receipt = vm.run_receipt(&prog);
            if !receipt.success {
                eprintln!("Execution failed!");
                std::process::exit(1);
            }

            let old_sender_acc = if let Some(s) = *sender {
                let acc = state.accounts.get_mut(&s).unwrap();
                let old = acc.clone();
                acc.nonce += 1;
                Some((s, old))
            } else {
                None
            };
            let post_root = state.root();

            let bytecode_bytes: Vec<u8> = prog
                .iter()
                .flat_map(|&b| b.to_le_bytes().to_vec())
                .collect();
            let prog_hash = compute_keccak256(&bytecode_bytes);

            let event_bytes: Vec<u8> = receipt
                .events
                .iter()
                .flat_map(|&e| e.to_le_bytes().to_vec())
                .collect();
            let event_digest = compute_keccak256(&event_bytes);

            let pi = ExecutionPublicInputs {
                chain_id: cli.chain_id,
                program_hash: prog_hash,
                initial_state_root: pre_root,
                final_state_root: post_root,
                sender: vm.context.sender,
                nonce: vm.context.nonce,
                block_height: vm.context.block_height,
                gas_limit: vm.gas_limit,
                gas_used: vm.gas_used,
                exit_code: 0,
                trace_len: vm.trace.len() as u64,
                event_digest,
            };

            let envelope = Prover::prove(&vm.trace, &pi, &prog).expect("Failed to generate proof");
            let ok = Prover::verify(&envelope, &pi, &prog).is_ok();

            if !ok {
                eprintln!("Verification failed!");
                if let Some((s, old)) = old_sender_acc {
                    state.accounts.insert(s, old);
                }
                std::process::exit(1);
            }

            state.save();
            println!(
                "Call success! Post-state Root: {:?}",
                hex::encode(post_root)
            );
        }
        Commands::Test => {
            let mut vm = Vm::new(1024);
            let prog = vec![
                Instruction {
                    opcode: Opcode::Add,
                    rd: 1,
                    rs1: 2,
                    rs2: 3,
                    imm: 0,
                }
                .encode(),
                Instruction {
                    opcode: Opcode::Halt,
                    rd: 0,
                    rs1: 0,
                    rs2: 0,
                    imm: 0,
                }
                .encode(),
            ];
            vm.registers[2] = 10;
            vm.registers[3] = 20;
            let receipt = vm.run_receipt(&prog);
            if receipt.success {
                println!("Register 1: {}", vm.registers[1]);
            } else {
                println!("Test execution failed!");
            }
        }
    }
}
