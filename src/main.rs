use rand::{thread_rng, Rng};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;
use std::thread;

fn is_drive_in_use(device: &str) -> bool {
    let output = Command::new("lsof")
        .arg(device)
        .output()
        .expect("Failed to execute lsof");

    !output.stdout.is_empty()
}

fn is_drive_mounted(device: &str) -> bool {
    let path = Path::new("/proc/mounts");
    let file = File::open(path).expect("Unable to open /proc/mounts");

    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line.unwrap();
        if line.contains(device) {
            return true;
        }
    }
    false
}

fn unmount_drive(device: &str) -> std::io::Result<()> {
    // Logic to unmount the drive
    // You can use std::process::Command to call `umount` here
    std::process::Command::new("sudo")
        .arg("umount")
        .arg(device)
        .status()?;

    Ok(())
}

fn verify_wipe(device: &str, drive_size: u64, use_random: bool) -> io::Result<()> {
    println!("Verifying wipe on {}", device);

    // Reopen the device for reading
    let mut file = OpenOptions::new()
        .read(true)
        .open(device)
        .expect("Failed to open device for verification");

    file.seek(SeekFrom::Start(0))?; // Reset file cursor to the start
    let mut read_buffer = vec![0u8; 1024 * 1024]; // 1MB buffer for reading
    let mut read_bytes: u64 = 0;

    while read_bytes < drive_size {
        file.read_exact(&mut read_buffer)?;

        if use_random {
            // Skip verification for random data since we can't predict the pattern
            eprintln!("Warning: Verification of random data is not supported.");
            break;
        } else {
            // For zero wipe, ensure all bytes are zero
            if read_buffer.iter().any(|&byte| byte != 0) {
                eprintln!("Verification failed on {}", device);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Verification failed: Non-zero byte found",
                ));
            }
        }

        read_bytes += read_buffer.len() as u64;
    }

    println!("Verification successful for {}", device);
    Ok(())
}

fn wipe_drive(device: &str, passes: u32, use_random: bool, verify: bool) -> std::io::Result<()> {
    // Open the device for writing
    let mut file = OpenOptions::new().write(true).open(device)?;

    // Get the drive size
    let output = Command::new("blockdev")
        .arg("--getsize64")
        .arg(device)
        .output()?;

    let drive_size: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap();

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
        if let Err(e) = verify_wipe(device, drive_size, use_random) {
            eprintln!("Verification failed: {}", e);
            std::process::exit(1); // Exit if verification fails
        }
    }

    println!("Drive wipe complete on {}", device);
    Ok(())
}

fn main() {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!(
            "Usage: {} [--zero|--random] [--passes <n>] [--verify] </dev/disk0> </dev/disk1> ...",
            args[0]
        );
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

    // Check if the device is mounted or in use before proceeding
    for device in &devices {
        if is_drive_mounted(device) || is_drive_in_use(device) {
            println!("The drive {} is currently mounted or in use.", device);
            print!("Would you like to unmount the drive now? (y/n): ");
            io::stdout().flush()?; // Ensure the prompt is printed

            let mut response = String::new();
            io::stdin().read_line(&mut response)?;

            if response.trim().eq_ignore_ascii_case("y") {
                unmount_drive(device)?;
                println!("Drive {} unmounted successfully.", device);
            } else {
                eprintln!("Please unmount the drive manually and try again.");
                std::process::exit(1); // Exit if the user declines
            }
        }
    }

    // Create a vector to hold the thread handles
    let mut handles = vec![];

    // launch a separate thread for each device
    for device in devices {
        let device_clone = device.clone();
        let handle = thread::spawn(move || {
            if let Err(e) = wipe_drive(&device_clone, passes, use_random, verify) {
                eprintln!("Failed to wipe {}: {}", device_clone, e);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Thread failed: {:?}", e);
        }
    }
}
