use flate2::read::ZlibDecoder;
use ini::Ini;
use sha1::{Sha1, Digest};
use std::{
    fs,
    str,
    path::{Path, PathBuf},
};

trait GitObject {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(&mut self, data: Vec<u8>);
    fn fmt(&self) -> &[u8];
}

struct GitBlob {
    blobdata: Vec<u8>,
}

impl GitObject for GitBlob {
    fn serialize(&self) -> Vec<u8> {
        return self.blobdata.to_owned();
    }

    fn deserialize(&mut self, data: Vec<u8>) {
        self.blobdata = data;
    }

    fn fmt(&self) -> &[u8] {
        return b"blob";
    }
}

enum GitObjects {
    Commit(),
    Tree(),
    Tag(),
    Blob(),
}

fn object_read(repo: &GitRepository, sha: &str) -> Result<GitObjects, String> {
    let path = repo_file(repo, vec!["objects", &sha[0..2], &sha[2..]], false)?;

    let raw_data = fs::read(path).unwrap();

    let decoder = ZlibDecoder::new(raw_data.as_slice());
    let decoded_data = decoder.get_ref();

    let fmt_end = match decoded_data.iter().position(|&x| x == b' ') {
        Some(p) => p,
        None => return Err(format!("Malformed object {}: Cannot read 'fmt'", sha)),
};
    let fmt = &decoded_data[..fmt_end];

    let size_end = match decoded_data.iter().position(|&x| x == b'\x00') {
        Some(p) => p,
        None => return  Err(format!("Malformed object {}: Cannot read 'size'", sha)),
    };
    let size = str::from_utf8(&decoded_data[fmt_end..size_end]).unwrap();
    let size: usize = size.parse().unwrap();
    if size != decoded_data.len() - size_end - 1 {
        return Err(format!("Malformed object {}: bad length", sha));
    }

    match fmt {
        b"commit" => {}
        b"tree" => {}
        b"tag" => {}
        b"blob" => {}
    }

    return Err(format!("Unknown type {:?} for object {}", fmt, sha));
}

fn object_find<'a>(repo: &GitRepository, name: &'a str, fmt: &str, follow: bool) -> &'a str {
    return name
}

fn object_write(obj: &GitObject, actually_write: bool) {
    let data = obj.serialize();
    let result = format!("{:?} {}\x00{}", obj.fmt(), data.len(), data);
    let mut sha1 = Sha1::default();
    sha1.input(result);
    let sha = sha1.result();

    if actually_write {
        let path = repo_file(obj.repo, vec!["objects", sha[0..2], sha[2..]], actually_write);
    }
}

pub struct GitRepository<'a> {
    pub worktree: &'a Path,
    pub gitdir: PathBuf,
    pub conf: Ini,
}

impl<'a> GitRepository<'a> {
    pub fn new(path: &str, force: bool) -> Result<GitRepository, String> {
        let worktree = Path::new(path);
        let gitdir = worktree.join(".git");

        if !(force || gitdir.is_dir()) {
            return Err(format!("Not a Git repository {}", path));
        }

        let mut conf = Ini::new();

        let path = gitdir.join("config");
        if path.exists() {
            conf = Ini::load_from_file(path).unwrap();
        } else if !force {
            return Err("Configuration file missing".to_string());
        }

        if !force {
            let mut core = conf.with_section(Some("core"));
            let vers = core.get("repositoryformatversion").unwrap();
            let vers: u32 = vers.parse().unwrap();
            if vers != 0 {
                return Err(format!("Unsupported repositoryformatversion {}", vers));
            }
        }

        Ok(GitRepository {
            worktree,
            gitdir,
            conf,
        })
    }

    pub fn repo_create(path: &str) -> Result<GitRepository, String> {
        let repo = GitRepository::new(path, true)?;

        if repo.worktree.exists() {
            if !repo.worktree.is_dir() {
                return Err(format!("{} is not a directory!", path));
            }
            if fs::read_dir(repo.worktree).unwrap().count() > 0 {
                return Err(format!("{} is not empty!", path));
            }
        } else {
            fs::create_dir_all(repo.worktree).unwrap();
        }

        repo_dir(&repo, vec!["branches"], true)?;
        repo_dir(&repo, vec!["objects"], true)?;
        repo_dir(&repo, vec!["refs", "tags"], true)?;
        repo_dir(&repo, vec!["refs", "heads"], true)?;

        fs::write(
            repo_file(&repo, vec!["description"], false).unwrap(),
            "Unnamed repository; edit this file 'description' to name the repository.",
        )
        .unwrap();

        fs::write(
            repo_file(&repo, vec!["HEAD"], false).unwrap(),
            "ref: refs/heads/master\n",
        )
        .unwrap();

        GitRepository::repo_default_config()
            .write_to_file(repo_file(&repo, vec!["config"], false).unwrap())
            .unwrap();

        Ok(repo)
    }

    fn repo_default_config() -> Ini {
        let mut conf = Ini::new();

        conf.with_section(Some("core"))
            .set("repositoryformatversion", "0")
            .set("filemode", "false")
            .set("bare", "false");

        conf
    }
}

fn repo_path(repo: &GitRepository, paths: Vec<&str>) -> PathBuf {
    let mut p = repo.gitdir.to_path_buf();
    for path in paths {
        p = p.join(path);
    }
    p
}

fn repo_file(repo: &GitRepository, paths: Vec<&str>, mkdir: bool) -> Result<PathBuf, String> {
    match repo_dir(repo, paths[..paths.len() - 1].to_vec(), mkdir) {
        Ok(_) => Ok(repo_path(repo, paths)),
        Err(e) => Err(e),
    }
}

fn repo_dir(repo: &GitRepository, paths: Vec<&str>, mkdir: bool) -> Result<PathBuf, String> {
    let path = repo_path(repo, paths);

    if path.exists() {
        return if path.is_dir() {
            Ok(path)
        } else {
            Err(format!("Not a directory {}", path.display()))
        };
    }

    if mkdir {
        fs::create_dir_all(&path).unwrap();
        return Ok(path);
    }

    return Err(format!("Failed to create dir {}", path.display()));
}

fn repo_find(path: &str, required: bool) -> Result<GitRepository, String> {
    let abs_path = fs::canonicalize(Path::new(path)).unwrap();

    if abs_path.join(".git").is_dir() {
        let repo = GitRepository::new(path, false)?;
        return Ok(repo);
    }

    if let Some(p) = abs_path.parent() {
        return repo_find(path, required);
    }

    return Err("Not a git repository".to_string());
}
