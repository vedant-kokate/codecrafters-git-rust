use std::env;
use std::fs;
use flate2::read::ZlibDecoder;
use std::io::Read;
use sha1::Digest;

fn cat_file(args: Vec<String>) -> Result<(),std::io::Error> {
    let hash = &args[3];
    let dir = &hash[0..2];
    let file = &hash[2..];
    let path = format!(".git/objects/{}/{}", dir, file);
    let content = fs::read(path)?;
    let mut decoder = ZlibDecoder::new(&content[..]);
    let mut decoded_content = Vec::new();
    decoder.read_to_end(&mut decoded_content)?;
    let mut header_end_index = 0;
    for (i, byte) in decoded_content.iter().enumerate() {
        if *byte == 0 {
            header_end_index = i;
            break;
        }
    }
    let content = &decoded_content[header_end_index + 1..];
    let content = String::from_utf8_lossy(content);

    print!("{}", content);
    Ok(())
}

fn hash_object(args: Vec<String>) -> Result<(), std::io::Error> {
    let file_path = &args[3];
    let binding = fs::read(file_path)?;
    let content = String::from_utf8_lossy(&binding);
    let header = format!("blob {}\0", content.len());
    let mut object = header.into_bytes();
    object.extend_from_slice(&content);
    print!("This is the content: {}", &content);
    let mut hasher = sha1::Sha1::new();
    hasher.update(&object);
    let hash = hasher.finalize();
    print("{}", hash);
    Ok(())
}

fn init() {
    fs::create_dir(".git").unwrap();
    fs::create_dir(".git/objects").unwrap();
    fs::create_dir(".git/refs").unwrap();
    fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();

    println!("Initialized git directory");
}

fn parse_command() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();

    match args[1].as_str() {
        "init" => init(),
        "cat-file" => cat_file(args)?,
        "hash-object" => hash_object(args)?,
        _ => println!("unknown command: {}", args[1]),
    }
    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    eprintln!("Logs from your program will appear here!");

    parse_command()?;
    Ok(())
}