use std::env;
use std::fs;
use flate2::read::ZlibDecoder;
use std::io::Read;
use sha1::Digest;
use std::io::Write;

fn get(url: &str) -> Result<Vec<u8>, std::io::Error> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .send()
        .map_err(std::io::Error::other)?;

    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(std::io::Error::other)
}

fn post(url: &str, body: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let response = reqwest::blocking::Client::new()
        .post(url)
        .header("Content-Type", "application/x-git-upload-pack-request")
        .body(body.to_vec())
        .send()
        .map_err(std::io::Error::other)?;

    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(std::io::Error::other)
}

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

fn get_hash_and_compressed_object(file_path: &str) -> Result<(String, Vec<u8>), std::io::Error> {
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
    Ok((format!("{:x}", hash), compressed_object))
}

fn hash_object(args: Vec<String>) -> Result<(), std::io::Error> {
    let file_path = &args[3];
    let (hash, compressed_object) = get_hash_and_compressed_object(file_path)?;
    println!("{}", hash);
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

fn get_hash_for_tree(dir_path: &str) -> Result<String, std::io::Error> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if name == ".git" {
            continue;
        }
        if path.is_dir() {
            let hash = get_hash_for_tree(path.to_str().unwrap())?;
            entries.push(format!("40000 {}\0{}", name, hash));
        } else if path.is_file() {
            let (hash, _) = get_hash_and_compressed_object(path.to_str().unwrap())?;
            entries.push(format!("100644 {}\0{}", name, hash));
        }
    }
    
    let mut content: Vec<u8> = Vec::new();
    entries.sort_by(|a, b| {
        a.split_once(' ').unwrap().1
            .cmp(b.split_once(' ').unwrap().1)
    });
    // println!("Entries: {:?}", entries);
    for entry in &entries {
        let (metadata, hash) = entry.split_once('\0').unwrap();
        content.extend_from_slice(metadata.as_bytes());
        content.push(0);

        // hash is 40-char hex → convert to 20 raw bytes
        for i in (0..hash.len()).step_by(2) {
            let byte = u8::from_str_radix(&hash[i..i + 2], 16).unwrap();
            content.push(byte);
        }

    }
    let header = format!("tree {}\0", content.len());
    let mut object = header.into_bytes();
    object.extend_from_slice(&content);
    let mut hasher = sha1::Sha1::new();
    hasher.update(&object);
    let hash = hasher.finalize();
    let hash = format!("{:x}", hash);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&object)?;
    let compressed_object = encoder.finish()?;
    fs::create_dir(format!(".git/objects/{}", &hash[0..2]))?;
    let path = format!(".git/objects/{}/{}",&hash[0..2],&hash[2..]);
    fs::write(path, compressed_object)?;
    Ok(hash)
}

fn write_tree(_args: Vec<String>) -> Result<(), std::io::Error> {
    let dir_path = std::env::current_dir()?;
    let hash = get_hash_for_tree(dir_path.to_str().unwrap())?;
    println!("{}", hash);
    Ok(())
}

fn commit_tree(args: Vec<String>) -> Result<(), std::io::Error> {
    let tree_hash = &args[2];
    let message = &args[args.len() - 1];
    // commit 177\0tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904
    // parent 3b18e512dba79e4c8300dd08aeb37f8e728b8dad
    // author John Doe <john@example.com> 1234567890 +0000
    // committer John Doe <john@example.com> 1234567890 +0000

    // Initial commit

    let mut content_body = format!("tree {}", tree_hash);

    if args.len() == 7 {
        // git commit-tree 5b825dc642cb6eb9a060e54bf8d69288fbee4904 -p 3b18e512dba79e4c8300dd08aeb37f8e728b8dad -m "Second commit"
        content_body.push_str(&format!("\nparent {}", &args[4]));
    }
    content_body.push_str(&format!("\nauthor John Doe <john@example.com> 1234567890 +0000"));
    content_body.push_str(&format!("\ncommitter John Doe <john@example.com> 1234567890 +0000"));
    content_body.push_str(&format!("\n\n{}\n", message));
    let header = format!("commit {}\0", content_body.len());
    let mut object = header.into_bytes();
    object.extend_from_slice(content_body.as_bytes());
    let mut hasher = sha1::Sha1::new();
    hasher.update(&object);
    let hash = hasher.finalize();
    let hash = format!("{:x}", hash);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&object)?;
    let compressed_object = encoder.finish()?;
    fs::create_dir(format!(".git/objects/{}", &hash[0..2]))?;
    let path = format!(".git/objects/{}/{}",&hash[0..2],&hash[2..]);
    fs::write(path, compressed_object)?;
    println!("{}", hash);
    Ok(())
}

fn clone(args: Vec<String>) -> Result<(), std::io::Error> {
    let url = &args[2];
    let repo_name = url.split('/').last().unwrap();
    fs::create_dir(repo_name)?;
    env::set_current_dir(repo_name)?;

    let get_url = format!("{}.git/info/refs?service=git-upload-pack", url);
    let body = get(&get_url)?;
    let text = String::from_utf8(body).map_err(std::io::Error::other)?;
    let lines = text.lines();
    for line in lines {
        if line.starts_with("003f") {
            let hash = &line[4..44];

            let body = format!(
                "{:04x}want {}\n0009done\n0000",
                4 + 5 + hash.len() + 1,
                hash
            );

            let response = post(
                &format!("{}.git/git-upload-pack", url),
                body.as_bytes()
            )?;
            println!("Request: {:?}", body);
            println!("Response: {:?}", response);
        }
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
        "write-tree" => write_tree(args)?,
        "commit-tree" => commit_tree(args)?,
        "clone" => clone(args)?,
        _ => println!("unknown command: {}", args[1]),
    }
    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    eprintln!("Logs from your program will appear here!");

    parse_command()?;
    Ok(())
}