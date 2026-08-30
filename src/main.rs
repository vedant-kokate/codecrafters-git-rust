use std::env;
use std::fs;
use flate2::read::ZlibDecoder;
use std::io::Read;

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
    let content = String::from_utf8_lossy(fs::read(file_path)?);
    print!("This is the content: {}", &content);
    // let header = format!("blob {}\0", content.len());
    // let mut store = Vec::new();
    // store.extend_from_slice(header.as_bytes());
    // store.extend_from_slice(&content);
    // let hash = sha1::Sha1::from(&store).hexdigest();
    // let dir = &hash[0..2];
    // let file = &hash[2..];
    // fs::create_dir_all(format!(".git/objects/{}", dir))?;
    // let compressed_content = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default())
    //     .write_all(&store)
    //     .unwrap()
    //     .finish()
    //     .unwrap();
    // fs::write(format!(".git/objects/{}/{}", dir, file), compressed_content)?;
    // println!("{}", hash);
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