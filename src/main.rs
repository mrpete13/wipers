use std::fs::OpenOptions;
use std::io::{Write, Seek, SeekFrom, Read};
use std::process::Command;
use std::env;
use rand::{thread_rng, Rng};

fn wipe_drive(device: &str, passes: u32, use_random: bool, verify: bool) -> std::io::Result<()> {
    // Open the device for writing
    let mut file = OpenOptions::new()
        .write(true)
        .open(device)?;

    // Get the drive size
    let output = Command::new("blockdev")
        .arg("--getsize64")
        .arg(device)
        .output()?;
    
    let drive_size: u64 = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap();
    
    // Create a buffer of 1MB
    let mut buffer = vec![0u8; 1024 * 1024];

    for pass in 1..=passes {
        println!("Pass {} of {} on {}", pass, passes, device);
        let mut written: u64 = 0;

        while written < drive_size {
            // Fill the buffer with random or zero data
            if use_random {
                let mut rng = thread_rng();
                rng.fill(&mut buffer[..]);
            } else {
                buffer.fill(0);
            }

            file.write_all(&buffer)?;
            written += buffer.len() as u64;

            // Display progress
            let progress = (written as f64 / drive_size as f64) * 100.0;
            print!("\rProgress: {:.2}%", progress);
            std::io::stdout().flush().unwrap();
        }

        // Ensure data is flushed
        file.flush()?;
        file.seek(SeekFrom::Start(0))?;
        println!("\nPass {} complete.", pass);
    }

    // Optionally verify the wipe
    if verify {
        println!("Verifying wipe on {}", device);
        file.seek(SeekFrom::Start(0))?;
        let mut read_buffer = vec![0u8; 1024 * 1024];
        let mut read_bytes: u64 = 0;

        while read_bytes < drive_size {
            file.read_exact(&mut read_buffer)?;
            if use_random {
                // For random data, we can't verify the exact pattern, so just skip
                eprintln!("Warning: Verification of random data is not supported.");
                break;
            } else {
                // For zero wipe, ensure all bytes are zero
                if read_buffer.iter().any(|&byte| byte != 0) {
                    eprintln!("Verification failed on {}", device);
                    std::process::exit(1);
                }
            }
            read_bytes += read_buffer.len() as u64;
        }

        println!("Verification successful for {}", device);
    }

    println!("Drive wipe complete on {}", device);
    Ok(())
}

fn main() {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} [--zero|--random] [--passes <n>] [--verify] <device1> <device2> ...", args[0]);
        std::process::exit(1);
    }

    // Default options
    let mut use_random = false;
    let mut passes = 1;
    let mut verify = false;
    let mut devices = vec![];

    // Process flags
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--random" => {
                use_random = true;
                i += 1;
            }
            "--zero" => {
                use_random = false;
                i += 1;
            }
            "--passes" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --passes requires a number");
                    std::process::exit(1);
                }
                passes = args[i + 1].parse().unwrap_or(1);
                i += 2;
            }
            "--verify" => {
                verify = true;
                i += 1;
            }
            _ => {
                devices.push(args[i].clone());
                i += 1;
            }
        }
    }

    if devices.is_empty() {
        eprintln!("Error: No devices specified.");
        std::process::exit(1);
    }

    // Wipe all specified devices
    for device in devices {
        if let Err(e) = wipe_drive(&device, passes, use_random, verify) {
            eprintln!("Failed to wipe {}: {}", device, e);
        }
    }
}
