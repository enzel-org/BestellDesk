use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use semver::Version;
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use tar::Archive;

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub tag: String,
    pub notes: String,
    pub asset_url: String,
    pub asset_name: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

fn desired_target_and_ext() -> (&'static str, &'static str) {
    #[cfg(target_os = "windows")]
    {
        ("pc-windows-msvc", ".zip")
    }
    #[cfg(target_os = "linux")]
    {
        ("unknown-linux-gnu", ".tar.gz")
    }
}

fn arch_tag() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(not(any(target_arch = "x86_64")))]
    {
        std::env::consts::ARCH
    }
}

fn binary_name() -> String {
    let base = "BestellDesk";
    if cfg!(target_os = "windows") {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn find_binary(dir: &Path) -> Option<PathBuf> {
    let wanted = binary_name();
    fn walk(base: &Path, wanted: &str, out: &mut Option<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(base)? {
            let e = entry?;
            let p = e.path();
            if p.is_dir() {
                walk(&p, wanted, out)?;
            } else if p
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s == wanted)
                .unwrap_or(false)
            {
                *out = Some(p);
                return Ok(());
            }
        }
        Ok(())
    }
    let mut found = None;
    let _ = walk(dir, &wanted, &mut found);
    found
}

pub async fn check_latest(
    owner: &str,
    repo: &str,
    current_ver: &str,
) -> Result<Option<UpdateInfo>> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::new();

    let resp = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, "BestellDesk-updater/1.0")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?
        .error_for_status()?;

    let rel: Release = resp.json().await?;

    fn normalize_tag(s: &str) -> &str {
        s.strip_prefix('v').unwrap_or(s)
    }
    let remote = Version::parse(normalize_tag(&rel.tag_name)).unwrap_or_else(|_| Version::new(0, 0, 0));
    let local = Version::parse(normalize_tag(current_ver)).unwrap_or_else(|_| Version::new(0, 0, 0));
    if remote <= local {
        return Ok(None);
    }

    let (target_os_tag, _ext) = desired_target_and_ext();
    let arch = arch_tag();
    let needle = format!("{arch}-{target_os_tag}");

    if let Some(a) = rel.assets.iter().find(|a| a.name.contains(&needle)) {
        Ok(Some(UpdateInfo {
            tag: rel.tag_name,
            notes: rel.body.unwrap_or_default(),
            asset_url: a.browser_download_url.clone(),
            asset_name: a.name.clone(),
        }))
    } else {
        bail!("No matching asset for target {needle}");
    }
}

pub async fn download_and_extract(info: &UpdateInfo) -> Result<PathBuf> {
    let client = reqwest::Client::new();
    let bytes = client
        .get(&info.asset_url)
        .header(reqwest::header::USER_AGENT, "BestellDesk-updater/1.0")
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let tmp = std::env::temp_dir();
    let archive_path = tmp.join(&info.asset_name);
    fs::write(&archive_path, &bytes)?;

    let extract_dir = archive_path.with_extension("extract");
    if extract_dir.exists() {
        let _ = fs::remove_dir_all(&extract_dir);
    }
    fs::create_dir_all(&extract_dir)?;

    if info.asset_name.ends_with(".zip") {
        let reader = std::io::Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(reader)?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let outpath = extract_dir.join(file.mangled_name());
            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p)?;
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if file.unix_mode().unwrap_or(0) & 0o111 != 0 {
                        let mut perm = outfile.metadata()?.permissions();
                        perm.set_mode(0o755);
                        fs::set_permissions(&outpath, perm)?;
                    }
                }
            }
        }
    } else if info.asset_name.ends_with(".tar.gz") || info.asset_name.ends_with(".tgz") {
        let reader = std::io::Cursor::new(bytes);
        let gz = GzDecoder::new(reader);
        let mut ar = Archive::new(gz);
        ar.unpack(&extract_dir)?;
    } else {
        let bin = extract_dir.join(binary_name());
        fs::write(&bin, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = fs::metadata(&bin)?.permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&bin, perm)?;
        }
    }

    let new_bin = find_binary(&extract_dir).context("Cannot locate new binary in extracted files")?;
    Ok(new_bin)
}

pub fn spawn_replacer_and_exit(new_exe: &Path) -> Result<()> {
    let current_exe = std::env::current_exe()?;

    #[cfg(target_os = "windows")]
    {
        let script = std::env::temp_dir().join("BestellDesk_update.bat");
        fs::write(
            &script,
            format!(
                "@echo off\r\n\
                 ping 127.0.0.1 -n 2 > nul\r\n\
                 copy /Y \"{new}\" \"{old}\"\r\n\
                 start \"\" \"{old}\"\r\n",
                new = new_exe.display(),
                old = current_exe.display(),
            ),
        )?;
        std::process::Command::new("cmd")
            .args(["/C", script.to_str().unwrap()])
            .spawn()?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let script = std::env::temp_dir().join("BestellDesk_update.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 sleep 1\n\
                 mv \"{new}\" \"{old}\"\n\
                 exec \"{old}\"\n",
                new = new_exe.display(),
                old = current_exe.display(),
            ),
        )?;
        let _ = std::process::Command::new("sh").arg(&script).spawn()?;
    }
    std::process::exit(0);
}
