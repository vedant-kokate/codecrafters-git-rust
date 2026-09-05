use std::env;
use std::fs;
use flate2::read::ZlibDecoder;
use std::io::Read;
use sha1::Digest;
use std::io::Write;
use std::collections::HashMap;

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


fn read_size_encoding(data: &[u8], pos: &mut usize) -> usize {
    let mut size = 0usize;
    let mut shift = 0;
    loop {
        let byte = data[*pos];
        *pos += 1;
        size |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
    }
    size
}

fn resolve_delta(base_data: &[u8], delta_data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut pos = 0;
    let _base_size = read_size_encoding(delta_data, &mut pos);
    let result_size = read_size_encoding(delta_data, &mut pos);

    let mut result = Vec::with_capacity(result_size);

    while pos < delta_data.len() {
        let instruction = delta_data[pos];
        pos += 1;

        if instruction & 0x80 != 0 {
            // copy instruction
            let mut offset: usize = 0;
            let mut size: usize = 0;

            if instruction & 0x01 != 0 { offset |= (delta_data[pos] as usize) << 0;  pos += 1; }
            if instruction & 0x02 != 0 { offset |= (delta_data[pos] as usize) << 8;  pos += 1; }
            if instruction & 0x04 != 0 { offset |= (delta_data[pos] as usize) << 16; pos += 1; }
            if instruction & 0x08 != 0 { offset |= (delta_data[pos] as usize) << 24; pos += 1; }

            if instruction & 0x10 != 0 { size |= (delta_data[pos] as usize) << 0;  pos += 1; }
            if instruction & 0x20 != 0 { size |= (delta_data[pos] as usize) << 8;  pos += 1; }
            if instruction & 0x40 != 0 { size |= (delta_data[pos] as usize) << 16; pos += 1; }

            if size == 0 {
                size = 0x10000;
            }

            result.extend_from_slice(&base_data[offset..offset + size]);
        } else if instruction != 0 {
            // insert instruction: literal bytes follow in the delta stream
            let len = instruction as usize;
            result.extend_from_slice(&delta_data[pos..pos + len]);
            pos += len;
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid delta instruction (0x00)",
            ));
        }
    }

    Ok(result)
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

fn hash_and_compress(object: &[u8]) -> Result<(String, Vec<u8>), std::io::Error> {
    let mut hasher = sha1::Sha1::new();
    hasher.update(object);
    let hash = format!("{:x}", hasher.finalize());

    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());

    encoder.write_all(object)?;
    let compressed = encoder.finish()?;

    Ok((hash, compressed))
}

fn get_hash_and_compressed_object(file_path: &str) -> Result<(String, Vec<u8>), std::io::Error> {
    let content = fs::read(file_path)?;

    let header = format!("blob {}\0", content.len());
    let mut object = header.into_bytes();
    object.extend_from_slice(&content);

    hash_and_compress(&object)
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

fn parse_header(pack: &[u8], pos: &mut usize) -> (u8, usize) {
    let mut byte = pack[*pos];
    *pos += 1;

    let object_type = (byte >> 4) & 7;
    let mut size = (byte & 15) as usize;
    let mut shift = 4;

    while byte & 0x80 != 0 {
        byte = pack[*pos];
        *pos += 1;
        size |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
    }

    (object_type, size)
}

fn unpack_object(pack: &[u8], pos: &mut usize,) -> Result<(u8, usize, Vec<u8>, Option<String>), std::io::Error> {
    let (object_type, size) = parse_header(pack, pos);
    let mut base_hash = None;
    if object_type == 7 {
        let hash = &pack[*pos..*pos + 20];
        *pos += 20;

        base_hash = Some(hash.iter().map(|b| format!("{:02x}", b)).collect());
    }

    let mut decoder = ZlibDecoder::new(&pack[*pos..]);
    let mut object_data = Vec::new();

    decoder.read_to_end(&mut object_data)?;

    *pos += decoder.total_in() as usize;

    Ok((object_type, size, object_data, base_hash))
}

fn store_object(object_type: u8,size: usize,object_data: &[u8],) -> Result<(), std::io::Error> {
    let object_type = match object_type {
        1 => "commit",
        2 => "tree",
        3 => "blob",
        4 => "tag",
        _ => panic!("unknown type"),
    };

    let header = format!("{} {}\0", object_type, size);

    let mut object = header.into_bytes();
    object.extend_from_slice(object_data);

    let (hash, compressed) = hash_and_compress(&object)?;

    fs::create_dir_all(format!(".git/objects/{}", &hash[..2]))?;

    fs::write(
        format!(".git/objects/{}/{}", &hash[..2], &hash[2..]),
        compressed,
    )?;

    Ok(())
}

fn get_pack(url: &str, hash: &str) -> Result<Vec<u8>, std::io::Error> {
    let body = format!(
        "0032want {}\n00000009done\n0000",
        hash
    );

    let response = post(
        &format!("{}.git/git-upload-pack", url),
        body.as_bytes(),
    )?;

    let pack_start = response
        .windows(4)
        .position(|w| w == b"PACK")
        .unwrap();

    Ok(response[pack_start..].to_vec())
}

fn get_hash_for_clone(url: &str) -> Result<String, std::io::Error> {
    let get_url = format!("{}.git/info/refs?service=git-upload-pack", url);
    let body = get(&get_url)?;
    let text = String::from_utf8(body).map_err(std::io::Error::other)?;

    for line in text.lines() {
        if line.starts_with("003f") {
            return Ok(line[4..44].to_string());
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "No hash found",
    ))
}

fn parse_tree_entries(content: &[u8]) -> Vec<(String, String, [u8; 20])> {
    let mut entries = Vec::new();
    let mut i = 0;
    while i < content.len() {
        let space = content[i..].iter().position(|&b| b == b' ').unwrap() + i;
        let mode = String::from_utf8_lossy(&content[i..space]).to_string();

        let null = content[space..].iter().position(|&b| b == 0).unwrap() + space;
        let name = String::from_utf8_lossy(&content[space + 1..null]).to_string();

        let mut sha = [0u8; 20];
        sha.copy_from_slice(&content[null + 1..null + 21]);

        entries.push((mode, name, sha));
        i = null + 21;
    }
    entries
}

fn checkout_tree(tree_hash: &str, dir: &std::path::Path) -> Result<(), std::io::Error> {
    let (_, content) = read_object(tree_hash)?;

    for (mode, name, sha) in parse_tree_entries(&content) {
        let sha_hex = sha.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        let path = dir.join(&name);

        if mode == "40000" {
            fs::create_dir_all(&path)?;
            checkout_tree(&sha_hex, &path)?;
        } else {
            let (_, blob_content) = read_object(&sha_hex)?;
            fs::write(&path, blob_content)?;

            #[cfg(unix)]
            if mode == "100755" {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    Ok(())
}

fn read_object(hash: &str) -> Result<(String, Vec<u8>), std::io::Error> {
    let path = format!(".git/objects/{}/{}", &hash[0..2], &hash[2..]);
    let content = fs::read(path)?;
    let mut decoder = ZlibDecoder::new(&content[..]);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded)?;

    let header_end = decoded.iter().position(|&b| b == 0).unwrap();
    let header = String::from_utf8_lossy(&decoded[..header_end]).to_string();
    let obj_type = header.split(' ').next().unwrap().to_string();

    Ok((obj_type, decoded[header_end + 1..].to_vec()))
}


fn checkout_commit(hash: &str, dir: &std::path::Path) -> Result<(), std::io::Error> {
    let (_, content) = read_object(commit_hash)?;
    let content_str = String::from_utf8_lossy(&content);
    let tree_line = content_str.lines().next().unwrap(); // "tree <hash>"
    let tree_hash = tree_line.split(' ').nth(1).unwrap();
    println!("Checking out tree: {}", tree_hash);
    checkout_tree(tree_hash, dir)
}

fn clone(args: Vec<String>) -> Result<(), std::io::Error> {
    let url = &args[2];
    let repo_name = &args[3];

    fs::create_dir(repo_name)?;
    env::set_current_dir(repo_name)?;
    init();

    let hash = get_hash_for_clone(url)?;
    let pack = get_pack(url, &hash)?;

    let object_count = u32::from_be_bytes(pack[8..12].try_into().unwrap());
    let mut pos = 12;

    // hash -> (type_num, content)
    let mut objects: HashMap<String, (u8, Vec<u8>)> = HashMap::new();
    let mut deltas = Vec::new();

    for _ in 0..object_count {
        let (object_type, size, object_data, base_hash) = unpack_object(&pack, &mut pos)?;

        if let Some(base_hash) = base_hash {
            deltas.push((base_hash, object_data));
        } else {
            store_object(object_type, size, &object_data)?;
            let (hash, _) = hash_and_compress(&{
                let mut o = format!("{} {}\0", type_name(object_type), size).into_bytes();
                o.extend_from_slice(&object_data);
                o
            })?;
            objects.insert(hash, (object_type, object_data));
        }
    }

    let mut pending = deltas;
    while !pending.is_empty() {
        let mut still_pending = Vec::new();
        let mut progressed = false;

        for (base_hash, delta_data) in pending {
            if let Some((obj_type, base_content)) = objects.get(&base_hash) {
                let resolved = resolve_delta(base_content, &delta_data)?;
                store_object(*obj_type, resolved.len(), &resolved)?;

                let mut o = format!("{} {}\0", type_name(*obj_type), resolved.len()).into_bytes();
                o.extend_from_slice(&resolved);
                let (resolved_hash, _) = hash_and_compress(&o)?;

                objects.insert(resolved_hash, (*obj_type, resolved));
                progressed = true;
            } else {
                still_pending.push((base_hash, delta_data));
            }
        }

        if !progressed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "couldn't resolve all deltas — missing base object",
            ));
        }
        pending = still_pending;
    }
    checkout_commit(&hash, &env::current_dir()?)?;
    Ok(())
}

fn type_name(t: u8) -> &'static str {
    match t {
        1 => "commit",
        2 => "tree",
        3 => "blob",
        4 => "tag",
        _ => panic!("unknown type"),
    }
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