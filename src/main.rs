use std::env;
use std::fs;
use object::read::pe::PeFile;
use object::{File, Object, ObjectSection};

/// A simple PE file parser that reads a file path from the command line
/// and prints its architecture and sections.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Get the file path from command-line arguments.
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path-to-pe-file>", args[0]);
        return Err("Invalid arguments".into());
    }
    let path = &args[1];

    // 2. Read the entire file into a byte buffer.
    println!("Reading file: {}", path);
    let buffer = fs::read(path)?;

    // 3. Parse the byte buffer directly as a PE file.
    // This will fail if the file is not a valid PE file.
    println!("Passing file...");
    let file = File::parse(&*buffer)?;

    match file{
        File::Pe32(pe) => {
            println!("Ok. File is 32-bit pe file.");
            print_pe_info(&pe);
        }

        File::Pe64(pe) => {
            println!("Ok. File is 64-bit pe file.");
        }

        _ => {
            return Err("This is a not valid PE file.".into());
        }

    }

    // 5. Return Ok to indicate success.
    Ok(())
}

fn print_pe_info<'data, O: Object<'data, 'data>>(pe_file: &'data O) {

    println!("Arch: {:?}" ,  pe_file.architecture());
    println!("Sections:");
    for section in pe_file.sections() {
        println!(" -Name: {:<8} Size: {:<8} Address: 0x{:x}",
        section.name().unwrap_or("?"),
        section.size(),
        section.address());
    }
}
