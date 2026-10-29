//! Update check-in, as Hokum does it: fetches this build's channel manifest from the Anomaly
//! release service (`/release/hokum/sound/<channel>.json`) with User-Agent
//! `hokum/<channel>/<version>/sound` — which is also how active installs are counted — shortly
//! after start and then hourly with jitter. When the published executable for this machine
//! differs from the running one (by sha256; versions are only shown), it reports an
//! [`Available`] to whoever asked (the UI shows the affordance). Nothing installs on its own.
//! Failures are silent; `check-for-updates #false` in config.kdl's `behaviour` turns it off.
//!
//! `sound update` checks now and prints the result; `sound update --install` replaces a plain
//! writable install (`~/.local/bin/sound`) in place.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

const HOST: &str = "https://anoma.ly";
/// The release service's app and artifact.
const APP: &str = "hokum";
const ARTIFACT: &str = "sound";
const SITE: &str = "https://github.com/anomalyco/hokum-sound";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRST: Duration = Duration::from_secs(10);
const EVERY: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Channel {
    Dev,
    Stable,
}

impl Channel {
    pub fn name(self) -> &'static str {
        match self {
            Channel::Dev => "dev",
            Channel::Stable => "stable",
        }
    }
}

/// Dev builds are numbered `0.0.0-dev-N`; everything else is stable.
pub fn channel(version: &str) -> Channel {
    if version.starts_with("0.0.0-dev-") { Channel::Dev } else { Channel::Stable }
}

pub fn user_agent() -> String {
    format!("{APP}/{}/{VERSION}/{ARTIFACT}", channel(VERSION).name())
}

#[derive(Deserialize)]
struct Manifest {
    release: Release,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub version: String,
    #[serde(default)]
    targets: HashMap<String, Target>,
    #[serde(default)]
    distributions: Vec<Distribution>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Target {
    url: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Distribution {
    kind: String,
    command: Option<String>,
    package: Option<String>,
}

fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" }
}

/// The build to offer: this machine's target, unless it is the one running.
pub fn newer<'a>(release: &'a Release, running: &str) -> Option<&'a Target> {
    release.targets.get(&format!("linux-{}", arch())).filter(|t| !t.sha256.eq_ignore_ascii_case(running))
}

/// How this executable was installed, which decides how it updates.
#[derive(Clone, Debug, PartialEq)]
pub enum Install {
    /// Replace the file in place (`~/.local/bin/sound`).
    Direct(PathBuf),
    /// A package manager owns it: pacman, dpkg or rpm, and the package's name.
    Package { manager: &'static str, package: String },
    /// Not writable and no package claims it.
    Unknown,
    /// A cargo build: never checks in.
    Source,
}

pub fn classify(path: &Path, owner: Option<(&'static str, String)>, writable: bool) -> Install {
    let s = path.to_string_lossy();
    if s.contains("/target/release/") || s.contains("/target/debug/") {
        return Install::Source;
    }
    if let Some((manager, package)) = owner {
        return Install::Package { manager, package };
    }
    if writable && !s.starts_with("/usr/") { Install::Direct(path.to_owned()) } else { Install::Unknown }
}

fn quiet(cmd: &mut Command) -> Option<String> {
    let out = cmd.stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn has(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

fn owner(path: &Path) -> Option<(&'static str, String)> {
    if let Some(p) = quiet(Command::new("pacman").arg("-Qqo").arg(path)) {
        return Some(("pacman", p.lines().next()?.to_owned()));
    }
    if let Some(p) = quiet(Command::new("dpkg").arg("-S").arg(path)) {
        return Some(("dpkg", p.split(':').next()?.trim().to_owned()));
    }
    quiet(Command::new("rpm").args(["-qf", "--qf", "%{NAME}"]).arg(path)).map(|p| ("rpm", p))
}

fn writable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let ok = |p: &Path| std::ffi::CString::new(p.as_os_str().as_bytes()).is_ok_and(|c| unsafe { libc::access(c.as_ptr(), libc::W_OK) } == 0);
    ok(path) && path.parent().is_some_and(ok)
}

/// The executable's path, read before an update can replace (and unlink) it.
pub fn exe() -> &'static Path {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        let p = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/bin/sound"));
        let s = p.to_string_lossy();
        s.strip_suffix(" (deleted)").map(PathBuf::from).unwrap_or(p)
    })
}

pub fn install() -> &'static Install {
    static INSTALL: OnceLock<Install> = OnceLock::new();
    INSTALL.get_or_init(|| {
        let path = exe();
        if classify(path, None, false) == Install::Source { Install::Source } else { classify(path, owner(path), writable(path)) }
    })
}

fn sha256(path: &Path) -> Option<String> {
    let out = quiet(Command::new("sha256sum").arg(path))?;
    out.split_whitespace().next().map(str::to_lowercase)
}

/// The running executable's sha256 (its inode, even after the file was replaced).
fn running() -> Option<&'static str> {
    static SHA: OnceLock<Option<String>> = OnceLock::new();
    // `/proc/self` in sha256sum would be sha256sum itself.
    SHA.get_or_init(|| sha256(&PathBuf::from(format!("/proc/{}/exe", std::process::id()))).or_else(|| sha256(exe()))).as_deref()
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(timeout)).user_agent(user_agent()).build().into()
}

fn fetch(channel: Channel) -> Result<Release, String> {
    let url = format!("{HOST}/release/{APP}/{ARTIFACT}/{}.json", channel.name());
    let body = agent(Duration::from_secs(10))
        .get(&url)
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
        .map_err(|e| format!("couldn't fetch {url}: {e}"))?;
    serde_json::from_str::<Manifest>(&body).map(|m| m.release).map_err(|e| format!("bad manifest at {url}: {e}"))
}

/// The command that updates a package manager's install, from the manifest when it names this
/// package (with the AUR helper that is actually installed).
pub fn command(release: &Release, manager: &str, package: &str, helper: impl Fn(&str) -> bool) -> Option<String> {
    let kind = match manager {
        "pacman" => "aur".to_owned(),
        "dpkg" => format!("deb-{}", arch()),
        "rpm" => format!("rpm-{}", arch()),
        _ => return None,
    };
    let d = release.distributions.iter().find(|d| d.kind == kind && d.package.as_deref() == Some(package))?;
    let cmd = d.command.clone()?;
    if manager == "pacman"
        && let Some(rest) = cmd.strip_prefix("yay ")
        && !helper("yay")
        && helper("paru")
    {
        return Some(format!("paru {rest}"));
    }
    Some(cmd)
}

/// A newer build is published for this machine. What the UI shows and acts on.
#[derive(Clone, Debug, PartialEq)]
pub struct Available {
    pub version: String,
    /// What updating means for this install.
    pub how: How,
    target: Target,
}

#[derive(Clone, Debug, PartialEq)]
pub enum How {
    /// A plain writable install: [`install_now`] replaces it; restart to use it.
    Replace,
    /// A package manager's install: run (or copy) this command.
    Command(String),
    /// Update it the way it was installed; point at the project page.
    Manual,
}

/// The project page, for [`How::Manual`].
pub fn site() -> &'static str {
    SITE
}

/// Checks now. `Ok(None)` when up to date or a cargo build.
pub fn check() -> Result<Option<Available>, String> {
    if *install() == Install::Source {
        return Ok(None);
    }
    let running = running().ok_or("couldn't read the running executable")?;
    let release = fetch(channel(VERSION))?;
    let Some(target) = newer(&release, running) else { return Ok(None) };
    let how = match install() {
        Install::Direct(_) => How::Replace,
        Install::Package { manager, package } => command(&release, manager, package, has).map_or(How::Manual, How::Command),
        _ => How::Manual,
    };
    Ok(Some(Available { version: release.version.clone(), how, target: target.clone() }))
}

/// Checks in shortly after start and then hourly (plus up to five minutes of jitter) until the
/// process exits, calling `found` whenever a newer build is out. Silent on failure; skipped
/// for cargo builds and while `check-for-updates` is off.
pub fn spawn(found: impl Fn(Available) + Send + 'static) {
    std::thread::Builder::new()
        .name("update".into())
        .spawn(move || {
            if *install() == Install::Source {
                return;
            }
            std::thread::sleep(FIRST);
            loop {
                if crate::config::load().check_for_updates
                    && let Ok(Some(a)) = check()
                {
                    found(a);
                }
                let jitter = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_millis() as u64) * 300;
                std::thread::sleep(EVERY + Duration::from_millis(jitter));
            }
        })
        .ok();
}

/// For [`How::Replace`]: downloads the build next to the executable, checks its size and
/// sha256 and renames it over the executable. The old one stays on any failure.
pub fn install_now(a: &Available) -> Result<(), String> {
    match install() {
        Install::Direct(path) => replace(path, &a.target),
        _ => Err("This install isn't a plain file this user can replace.".into()),
    }
}

fn replace(path: &Path, t: &Target) -> Result<(), String> {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    let dir = path.parent().ok_or("no folder for the executable")?;
    let tmp = dir.join(format!(".sound-update-{}", std::process::id()));
    let url = if t.url.starts_with("http") { t.url.clone() } else { format!("{HOST}{}", t.url) };
    let fail = |msg: String| {
        let _ = std::fs::remove_file(&tmp);
        Err(msg)
    };
    let download = || -> Result<(), String> {
        let mut r = agent(Duration::from_secs(15 * 60)).get(&url).call().map_err(|e| e.to_string())?;
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        std::io::copy(&mut r.body_mut().as_reader().take(t.size + 1), &mut file).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    };
    if let Err(e) = download() {
        return fail(format!("The download failed ({e})."));
    }
    if std::fs::metadata(&tmp).map(|m| m.len()).ok() != Some(t.size) {
        return fail("The download was the wrong size.".into());
    }
    if sha256(&tmp).as_deref() != Some(t.sha256.to_lowercase().as_str()) {
        return fail("The download didn't match its checksum.".into());
    }
    if let Err(e) = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)) {
        return fail(format!("Couldn't make it executable: {e}."));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        return fail(format!("Couldn't replace {}: {e}.", path.display()));
    }
    Ok(())
}

/// `sound update [--install]`.
pub fn cli(args: &[String]) -> i32 {
    println!("Hokum Sound {VERSION} ({} channel) at {}", channel(VERSION).name(), exe().display());
    if *install() == Install::Source {
        println!("This is a cargo build; update it from source.");
        return 0;
    }
    let a = match check() {
        Ok(Some(a)) => a,
        Ok(None) => {
            println!("Up to date.");
            return 0;
        }
        Err(e) => {
            eprintln!("sound: {e}");
            return 1;
        }
    };
    println!("Hokum Sound {} is available.", a.version);
    let now = args.iter().any(|a| a == "--install");
    match &a.how {
        How::Replace if now => match install_now(&a) {
            Ok(()) => {
                println!("Installed Hokum Sound {} at {}. Restart it to use it.", a.version, exe().display());
                0
            }
            Err(e) => {
                eprintln!("sound: {e}");
                1
            }
        },
        How::Replace => {
            println!("Run `sound update --install` to install it.");
            0
        }
        How::Command(cmd) => {
            println!("Update it with your package manager: {cmd}");
            i32::from(now)
        }
        How::Manual => {
            println!("Update Hokum Sound the way you installed it ({SITE}).");
            i32::from(now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(sha: &str) -> Release {
        let json = format!(
            r#"{{"version":"0.0.0-dev-23","targets":{{"linux-x64":{{"url":"/x","size":1,"sha256":"{sha}"}},"linux-arm64":{{"url":"/y","size":1,"sha256":"{sha}"}}}},
            "distributions":[{{"kind":"direct"}},{{"kind":"aur","command":"yay -Syu hokum-sound-bin","package":"hokum-sound-bin"}},
            {{"kind":"deb-{a}","command":"sudo apt install /tmp/hokum-sound.deb","package":"hokum-sound"}}]}}"#,
            a = arch()
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn the_channel_and_agent_come_from_the_version() {
        assert_eq!(channel("0.0.0-dev-22"), Channel::Dev);
        assert_eq!(channel("0.1.1"), Channel::Stable);
        assert!(user_agent().starts_with("hokum/") && user_agent().ends_with(&format!("/{VERSION}/sound")));
    }

    #[test]
    fn a_build_is_current_when_its_sha_matches() {
        let r = release("ABC");
        assert!(newer(&r, "abc").is_none());
        assert!(newer(&r, "def").is_some());
    }

    #[test]
    fn the_install_decides_how_to_update() {
        let home = Path::new("/home/d/.local/bin/sound");
        assert_eq!(classify(home, None, true), Install::Direct(home.to_owned()));
        assert_eq!(classify(home, None, false), Install::Unknown);
        assert_eq!(classify(Path::new("/usr/bin/sound"), None, true), Install::Unknown);
        assert_eq!(
            classify(Path::new("/usr/bin/sound"), Some(("pacman", "hokum-sound-bin".into())), false),
            Install::Package { manager: "pacman", package: "hokum-sound-bin".into() }
        );
        assert_eq!(classify(Path::new("/home/d/hokum-sound/target/release/sound"), None, true), Install::Source);
    }

    #[test]
    fn package_commands_come_from_the_manifest() {
        let r = release("abc");
        assert_eq!(command(&r, "pacman", "hokum-sound-bin", |h| h == "paru").as_deref(), Some("paru -Syu hokum-sound-bin"));
        assert_eq!(command(&r, "pacman", "hokum-sound-bin", |_| true).as_deref(), Some("yay -Syu hokum-sound-bin"));
        assert_eq!(command(&r, "dpkg", "hokum-sound", |_| true).as_deref(), Some("sudo apt install /tmp/hokum-sound.deb"));
        assert_eq!(command(&r, "rpm", "hokum-sound", |_| true), None);
    }
}
