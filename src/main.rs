use std::env;
use std::fs;
use flate2::read::ZlibDecoder;
use std::io::Read;
use sha1::Digest;
use std::io::Write;

fn get_file_content_from_hash(hash: &str) -> Result<Vec<u8>, std::io::Error> {
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
    Ok(content.to_vec())
}

fn cat_file(args: Vec<String>) -> Result<(),std::io::Error> {
    let hash = &args[3];
    let content = get_file_content_from_hash(hash)?;
    let content = String::from_utf8_lossy(&content);
    print!("{}", content);
    Ok(())
}

fn hash_object(args: Vec<String>) -> Result<(), std::io::Error> {
    let file_path = &args[3];
    let content = fs::read(file_path)?;
    let header = format!("blob {}\0", content.len());
    let mut object = header.into_bytes();
    object.extend_from_slice(&content);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&object)?;
    let compressed_object = encoder.finish()?;
    let mut hasher = sha1::Sha1::new();
    hasher.update(&object);
    let hash = hasher.finalize();
    println!("{:x}", hash);
    let hash = format!("{:x}", hash);
    fs::create_dir(format!(".git/objects/{}", &hash[0..2]))?;
    let path = format!(".git/objects/{}/{}",&hash[0..2],&hash[2..]);
    fs::write(path, compressed_object)?;
    Ok(())
}

fn init() {
    fs::create_dir(".git").unwrap();
    fs::create_dir(".git/objects").unwrap();
    fs::create_dir(".git/refs").unwrap();
    fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();

    println!("Initialized git directory");
}

fn ls_tree(args: Vec<String>) -> Result<(), std::io::Error> {
    let hash = &args[3];
    let content = get_file_content_from_hash(hash)?;
    let mut i = 0;
    let mut names = Vec::new();
    let mut last_space_index = 0;
    while i < content.len() {
        if content[i] == 0 {
            let part = &content[last_space_index..i];
            let part_str = String::from_utf8_lossy(part);
            names.push(part_str.to_string());
            i+=20
        }
        if content[i] == b' ' {
            last_space_index = i + 1;
        }
        i+=1;
    }
   
    names.sort();
    for name in names {
        println!("{}", name);
    }
    Ok(())
}

fn parse_command() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();

    match args[1].as_str() {
        "init" => init(),
        "cat-file" => cat_file(args)?,
        "hash-object" => hash_object(args)?,
        "ls-tree" => ls_tree(args)?,
        _ => println!("unknown command: {}", args[1]),
    }
    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    eprintln!("Logs from your program will appear here!");

    parse_command()?;
    Ok(())
}