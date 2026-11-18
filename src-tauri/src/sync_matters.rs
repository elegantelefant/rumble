//! ABOUTME: The matter screens' host side: matters opened this session, files with numbered versions and authors.
//! ABOUTME: Invites and safety numbers, working copies opened in Word, and fork resolution, over sync_briefcase.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::sync_briefcase::{Briefcase, State};
use crate::sync_crypto::{Body, DeviceKeys, Hash, Header, Link, PublicKey, RecordError, VersionFields};

/// Types Rumble opens in their own app: macro-free formats only. Anything else is "Show in folder".
const OPENABLE_EXTENSIONS: [&str; 6] = ["docx", "xlsx", "pptx", "pdf", "txt", "md"];
const MAX_BASENAME_CHARS: usize = 100;
const MAX_EXTENSION_CHARS: usize = 10;
const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
    "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];
/// Beside each working-copy folder: the version written into it, then any further parents its next save names.
const OPENED_SUFFIX: &str = ".opened";

#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    Admin,
    Member,
    /// Asked to join; in only once an admin approves and this side confirms the number.
    Waiting,
    /// Its folder or local store can't be read right now.
    Unavailable,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MatterSummary {
    pub id: String,
    /// The matter folder's own name: the matter's name record is not built yet.
    pub name: String,
    pub folder: String,
    pub standing: Standing,
    /// For a waiting joiner: the admin who issued the link.
    pub waiting_for: Option<String>,
    /// Why an unavailable matter can't be read.
    pub unavailable: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VersionView {
    pub id: String,
    /// Depth in the parent graph; with the author's initials where two versions share a depth.
    pub label: String,
    pub author: String,
    pub mine: bool,
    pub saved_at_ms: i64,
    pub head: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub id: String,
    pub name: String,
    pub openable: bool,
    /// Newest first.
    pub versions: Vec<VersionView>,
    /// The label of the version two people each edited, when the file has more than one head.
    pub forked_from: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RequestView {
    pub device: String,
    pub name: String,
    pub safety_number: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MemberView {
    pub device: String,
    pub name: String,
    pub admin: bool,
    pub mine: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MatterDetail {
    pub matter: MatterSummary,
    pub files: Vec<FileView>,
    pub members: Vec<MemberView>,
    pub requests: Vec<RequestView>,
    /// This side's number while waiting to join.
    pub safety_number: Option<String>,
    pub confirmed: bool,
    pub needs_second_admin: bool,
    /// Invites two people answered (by invite id): neither can be let in, so the admin withdraws them.
    pub contested: Vec<String>,
    /// Working copies saved in their app whose save couldn't be recorded as a version, by file name.
    pub not_recorded: Vec<String>,
}

/// One launch's sync session: device keys held in memory until the hardware-wrapped key store lands,
/// so its data lives under a folder named for those keys and two launches never share a store.
pub struct Session {
    keys: DeviceKeys,
    root: PathBuf,
    folders: BTreeMap<Hash, PathBuf>,
}

impl Session {
    pub fn new(base: &Path) -> Self {
        let keys = DeviceKeys::generate();
        let root = base.join(hex::encode(keys.signer()));
        Session { keys, root, folders: BTreeMap::new() }
    }

    pub fn create(&mut self, folder: &Path, your_name: &str) -> Result<MatterSummary, RecordError> {
        let id = Briefcase::create(&self.keys, folder, &self.root, your_name)?.id;
        self.folders.insert(id, folder.into());
        self.summary(id)
    }

    /// Answer an invite link: write a join request into the matter folder the link belongs to.
    pub fn join(&mut self, folder: &Path, link: &[u8], your_name: &str) -> Result<MatterSummary, RecordError> {
        let id = Link::verify(link).map_err(|_| io::Error::other("That invite link isn't valid. Ask for a fresh one."))?.briefcase_id;
        let b = Briefcase::open(id, &self.keys, folder, &self.root)?;
        if !b.sync()?.has_genesis() {
            return Err(io::Error::other("That folder doesn't hold this matter. Pick the folder your colleague shared with you.").into());
        }
        b.request(link, your_name)?;
        self.folders.insert(id, folder.into());
        self.summary(id)
    }

    /// Every matter open in this launch; one that can't be read is listed as unavailable, not dropped.
    pub fn matters(&self) -> Result<Vec<MatterSummary>, RecordError> {
        Ok(self
            .folders
            .keys()
            .map(|id| {
                self.summary(*id).unwrap_or_else(|e| {
                    let folder = &self.folders[id];
                    MatterSummary {
                        id: hex::encode(id),
                        name: folder_name(folder),
                        folder: folder.display().to_string(),
                        standing: Standing::Unavailable,
                        waiting_for: None,
                        unavailable: Some(say(e)),
                    }
                })
            })
            .collect())
    }

    /// At quit: record every saved working copy, then remove this launch's store and working copies,
    /// which no later launch could open. Kept if any save couldn't be recorded.
    pub fn close(&self) -> Result<(), RecordError> {
        let mut not_recorded = vec![];
        for id in self.folders.keys() {
            not_recorded.extend(self.synced(*id)?.2);
        }
        if !not_recorded.is_empty() {
            return Err(io::Error::other(format!("saves not recorded, working copies kept: {}", not_recorded.join(", "))).into());
        }
        match fs::remove_dir_all(&self.root) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    pub fn withdraw(&self, id: Hash, invite: Uuid) -> Result<Hash, RecordError> {
        self.briefcase(id)?.revoke(invite)
    }

    pub fn detail(&self, id: Hash) -> Result<MatterDetail, RecordError> {
        let (b, state, not_recorded) = self.synced(id)?;
        let me = self.keys.signer();
        let matter = self.summarise(&b, &state)?;
        let name = |d: &PublicKey| state.devices.get(d).map_or("Someone".to_string(), |(_, f)| f.display_name.clone());
        let mut members: Vec<MemberView> = state
            .roll
            .members
            .iter()
            .filter(|d| state.is_member(d))
            .map(|d| MemberView { device: hex::encode(d), name: name(d), admin: state.roll.admins.contains(d), mine: *d == me })
            .collect();
        members.sort_by(|a, b| a.name.cmp(&b.name));
        let admin = matter.standing == Standing::Admin;
        let requests = match admin {
            true => state
                .requests
                .iter()
                .map(|(d, (_, fields))| RequestView {
                    device: hex::encode(d),
                    name: fields.display_name.clone(),
                    safety_number: state.safety_number(d),
                })
                .collect(),
            false => vec![],
        };
        Ok(MatterDetail {
            safety_number: (matter.standing == Standing::Waiting).then(|| b.safety_number().ok()).flatten(),
            confirmed: b.confirmed(),
            needs_second_admin: admin && members.len() >= 2 && members.iter().filter(|m| m.admin).count() < 2,
            contested: if admin { state.contested.iter().map(Uuid::to_string).collect() } else { vec![] },
            not_recorded,
            files: files(&state, &me),
            members,
            requests,
            matter,
        })
    }

    pub fn add_file(&self, id: Hash, path: &Path) -> Result<Hash, RecordError> {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.briefcase(id)?.save_version(Uuid::new_v4(), &name, media_type(&name), path, vec![])
    }

    pub fn invite(&self, id: Hash, invitee_name: &str) -> Result<Vec<u8>, RecordError> {
        self.briefcase(id)?.invite(invitee_name)
    }

    pub fn confirm(&self, id: Hash, number: &str) -> Result<(), RecordError> {
        self.briefcase(id)?.confirm(number)
    }

    pub fn approve(&self, id: Hash, device: PublicKey, number: &str) -> Result<Hash, RecordError> {
        self.briefcase(id)?.approve(device, false, number)
    }

    pub fn promote(&self, id: Hash, device: PublicKey) -> Result<Hash, RecordError> {
        self.briefcase(id)?.promote(device)
    }

    /// The file's working copy at its one head, written there if older. Unrecorded saves are kept first.
    pub fn check_out(&self, id: Hash, file: Uuid) -> Result<PathBuf, RecordError> {
        let (b, state, _) = self.synced(id)?;
        let heads = heads(&state, file)?;
        if heads.len() != 1 {
            return Err(io::Error::other("Two versions of this file need combining first.").into());
        }
        self.put(&b, &state, file, heads[0], &heads)
    }

    /// My head as the working copy, naming every head as the next save's parents, with each other head
    /// released beside it. Returns the working copy first.
    pub fn combine(&self, id: Hash, file: Uuid) -> Result<Vec<PathBuf>, RecordError> {
        let (b, state, _) = self.synced(id)?;
        let mut heads = heads(&state, file)?;
        let me = self.keys.signer();
        heads.sort_by_key(|h| state.versions[h].signer != me);
        let mut paths = vec![self.put(&b, &state, file, heads[0], &heads)?];
        for h in &heads[1..] {
            let header = &state.versions[h];
            let fields = version_fields(header);
            let author = state.devices.get(&header.signer).map_or("a colleague", |(_, f)| f.display_name.as_str());
            let (stem, ext) = fields.file_name.rsplit_once('.').unwrap_or((&fields.file_name, ""));
            let dest = paths[0].with_file_name(safe_basename(&format!("{stem} (from {author}).{ext}")));
            b.release(&state, h, &dest)?;
            mark_from_colleague(&dest)?;
            paths.push(dest);
        }
        Ok(paths)
    }

    /// "Use mine" or "Use Ana's": a new version with `chosen`'s content and every head as a parent.
    pub fn resolve(&self, id: Hash, file: Uuid, chosen: Hash) -> Result<Hash, RecordError> {
        let (b, state, _) = self.synced(id)?;
        let heads = heads(&state, file)?;
        if !heads.contains(&chosen) {
            return Err(io::Error::other("That version is no longer one of the two to choose from.").into());
        }
        let fields = version_fields(&state.versions[&chosen]);
        let temp = self.root.join("tmp");
        fs::create_dir_all(&temp)?;
        let content = temp.join(Uuid::new_v4().to_string());
        b.release(&state, &chosen, &content)?;
        let saved = b.save_version(file, &fields.file_name, &fields.media_type, &content, heads);
        fs::remove_file(&content)?;
        saved
    }

    fn briefcase(&self, id: Hash) -> Result<Briefcase<'_>, RecordError> {
        let folder = self.folders.get(&id).ok_or_else(|| io::Error::other("This matter isn't open in Rumble."))?;
        Ok(Briefcase::open(id, &self.keys, folder, &self.root)?)
    }

    /// The briefcase after one sync, with any working copy saved since its last version recorded first.
    /// A save that can't be recorded (removed, or waiting for a membership change) is named, not fatal.
    fn synced(&self, id: Hash) -> Result<(Briefcase<'_>, State, Vec<String>), RecordError> {
        let b = self.briefcase(id)?;
        let state = b.sync()?;
        let (saved, not_recorded) = self.sweep(&b, &state)?;
        let state = if saved { b.sync()? } else { state };
        Ok((b, state, not_recorded))
    }

    fn summary(&self, id: Hash) -> Result<MatterSummary, RecordError> {
        let b = self.briefcase(id)?;
        let state = b.sync()?;
        self.summarise(&b, &state)
    }

    fn summarise(&self, b: &Briefcase, state: &State) -> Result<MatterSummary, RecordError> {
        let me = self.keys.signer();
        let folder = &self.folders[&b.id];
        let standing = match (state.is_member(&me) && b.confirmed(), state.roll.admins.contains(&me)) {
            (false, _) => Standing::Waiting,
            (true, true) => Standing::Admin,
            (true, false) => Standing::Member,
        };
        Ok(MatterSummary {
            id: hex::encode(b.id),
            name: folder_name(folder),
            folder: folder.display().to_string(),
            standing,
            waiting_for: (standing == Standing::Waiting).then(|| b.answered().map(|l| l.admin_name)).flatten(),
            unavailable: None,
        })
    }

    fn work(&self, id: Hash) -> PathBuf {
        self.root.join("work").join(hex::encode(id))
    }

    fn copy_path(&self, id: Hash, file: Uuid, file_name: &str) -> PathBuf {
        self.work(id).join(file.to_string()).join(safe_basename(file_name))
    }

    fn marker(&self, id: Hash, file: Uuid) -> PathBuf {
        self.work(id).join(format!("{file}{OPENED_SUFFIX}"))
    }

    /// Write `content` into the file's working copy unless it already holds it; the next save names `parents`.
    fn put(&self, b: &Briefcase, state: &State, file: Uuid, content: Hash, parents: &[Hash]) -> Result<PathBuf, RecordError> {
        let header = &state.versions[&content];
        let path = self.copy_path(b.id, file, &version_fields(header).file_name);
        let marker = self.marker(b.id, file);
        if read_marker(&marker).first() != Some(&content) || !path.exists() {
            fs::create_dir_all(path.parent().expect("working copies sit in a folder"))?;
            b.release(state, &content, &path)?;
            if header.signer != self.keys.signer() {
                mark_from_colleague(&path)?;
            }
        }
        fs::write(&marker, parents.iter().map(hex::encode).collect::<Vec<_>>().join("\n"))?;
        Ok(path)
    }

    /// Record a version for every working copy whose content differs from the version last written into it.
    /// Returns whether any was recorded, and the file names whose save the matter refused.
    fn sweep(&self, b: &Briefcase, state: &State) -> Result<(bool, Vec<String>), RecordError> {
        let Ok(entries) = fs::read_dir(self.work(b.id)) else { return Ok((false, vec![])) };
        let (mut saved, mut refused) = (false, vec![]);
        for entry in entries {
            let marker = entry?.path();
            let Some(file) = marker.file_name().and_then(|n| n.to_str()?.strip_suffix(OPENED_SUFFIX)?.parse::<Uuid>().ok()) else {
                continue;
            };
            let parents = read_marker(&marker);
            let Some(fields) = parents.first().and_then(|p| state.versions.get(p)).map(version_fields) else { continue };
            let path = self.copy_path(b.id, file, &fields.file_name);
            if !path.exists() || sha256_file(&path)? == fields.content_sha256 {
                continue;
            }
            match b.save_version(file, &fields.file_name, &fields.media_type, &path, parents) {
                Ok(version) => {
                    fs::write(&marker, hex::encode(version))?;
                    saved = true;
                }
                Err(RecordError::NotMember | RecordError::NotConfirmed | RecordError::WaitingForMembershipUpdate) => {
                    refused.push(fields.file_name)
                }
                Err(e) => return Err(e),
            }
        }
        Ok((saved, refused))
    }
}

fn files(state: &State, me: &PublicKey) -> Vec<FileView> {
    let mut depths = HashMap::new();
    let author = |h: &Header| state.devices.get(&h.signer).map_or("Someone".to_string(), |(_, f)| f.display_name.clone());
    state
        .files
        .iter()
        .map(|(id, history)| {
            let depth_of: HashMap<Hash, u64> = history.versions.iter().map(|v| (*v, depth(state, *v, &mut depths))).collect();
            let mut at_depth: HashMap<u64, usize> = HashMap::new();
            for d in depth_of.values() {
                *at_depth.entry(*d).or_default() += 1;
            }
            let label = |v: &Hash, d: u64| match at_depth.get(&d).copied().unwrap_or(1) {
                1 => d.to_string(),
                _ => format!("{d}-{}", initials(&author(&state.versions[v]))),
            };
            let mut versions: Vec<VersionView> = history
                .versions
                .iter()
                .map(|v| {
                    let h = &state.versions[v];
                    VersionView {
                        id: hex::encode(v),
                        label: label(v, depth_of[v]),
                        author: author(h),
                        mine: h.signer == *me,
                        saved_at_ms: version_fields(h).created_at_ms,
                        head: history.heads.contains(v),
                    }
                })
                .collect();
            versions.sort_by_key(|v| std::cmp::Reverse((v.label.split('-').next().and_then(|d| d.parse::<u64>().ok()), v.saved_at_ms)));
            let newest = history.heads.iter().map(|h| &state.versions[h]).max_by_key(|h| version_fields(h).created_at_ms);
            let name = newest.map(|h| version_fields(h).file_name).unwrap_or_default();
            FileView {
                id: id.to_string(),
                openable: openable(&name),
                name,
                versions,
                forked_from: (history.heads.len() > 1)
                    .then(|| diverged_at(state, &history.heads.iter().copied().collect::<Vec<_>>()))
                    .flatten()
                    .map(|v| label(&v, depth(state, v, &mut HashMap::new()))),
            }
        })
        .collect()
}

/// A version's number: one past its deepest parent, so every laptop numbers it the same. Walked with
/// an explicit stack: every save is a version, so a history can be thousands deep.
fn depth(state: &State, v: Hash, memo: &mut HashMap<Hash, u64>) -> u64 {
    let parents = |h: &Hash| -> Vec<Hash> {
        state.versions.get(h).map(|h| h.parents.iter().filter(|p| state.versions.contains_key(*p)).copied().collect()).unwrap_or_default()
    };
    let mut stack = vec![v];
    while let Some(&top) = stack.last() {
        if memo.contains_key(&top) {
            stack.pop();
            continue;
        }
        let pending: Vec<Hash> = parents(&top).into_iter().filter(|p| !memo.contains_key(p)).collect();
        if pending.is_empty() {
            let d = 1 + parents(&top).iter().map(|p| memo[p]).max().unwrap_or(0);
            memo.insert(top, d);
            stack.pop();
        } else {
            stack.extend(pending);
        }
    }
    memo[&v]
}

fn ancestors(state: &State, v: Hash) -> HashSet<Hash> {
    let mut seen = HashSet::new();
    let mut stack = vec![v];
    while let Some(h) = stack.pop() {
        if seen.insert(h) {
            stack.extend(state.versions.get(&h).map(|h| h.parents.clone()).unwrap_or_default());
        }
    }
    seen
}

/// The deepest version every head descends from: the one two people each edited.
fn diverged_at(state: &State, heads: &[Hash]) -> Option<Hash> {
    let mut common = ancestors(state, *heads.first()?);
    for h in &heads[1..] {
        let theirs = ancestors(state, *h);
        common.retain(|a| theirs.contains(a));
    }
    let mut memo = HashMap::new();
    common.into_iter().filter(|a| state.versions.contains_key(a)).max_by_key(|a| (depth(state, *a, &mut memo), *a))
}

fn initials(name: &str) -> String {
    let got: String = name.split_whitespace().filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect();
    if got.is_empty() { "?".into() } else { got }
}

fn heads(state: &State, file: Uuid) -> Result<Vec<Hash>, RecordError> {
    let history = state.files.get(&file).ok_or_else(|| io::Error::other("That file isn't in this matter."))?;
    Ok(history.heads.iter().copied().collect())
}

fn version_fields(h: &Header) -> VersionFields {
    match &h.body {
        Body::Version(v) => v.clone(),
        _ => unreachable!("state.versions holds versions only"),
    }
}

fn folder_name(folder: &Path) -> String {
    folder.file_name().map_or_else(|| folder.display().to_string(), |n| n.to_string_lossy().into_owned())
}

fn read_marker(path: &Path) -> Vec<Hash> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| hex::decode(l).ok()?.try_into().ok()).collect()
}

fn sha256_file(path: &Path) -> io::Result<Hash> {
    let mut hasher = Sha256::new();
    io::copy(&mut File::open(path)?, &mut hasher)?;
    Ok(hasher.finalize().into())
}

/// Windows: the Mark of the Web, so Office treats a colleague's file as from the internet (macros blocked).
fn mark_from_colleague(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    fs::write(format!("{}:Zone.Identifier", path.display()), "[ZoneTransfer]\r\nZoneId=3\r\n")?;
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

fn extension(name: &str) -> Option<String> {
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())
}

fn media_type(name: &str) -> &'static str {
    match extension(name).as_deref() {
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        _ => "application/octet-stream",
    }
}

/// What the matter screens tell the person when a sync step refuses.
pub fn say(e: RecordError) -> String {
    match e {
        RecordError::Io(e) => e.to_string(),
        RecordError::NeedsSecondAdmin => "Make a colleague a second admin before inviting a third person. If this laptop is lost, nobody else could add or remove people.".into(),
        RecordError::SafetyNumberMismatch => "The numbers don't match. Don't let them in: send a fresh invite instead.".into(),
        RecordError::NotMember | RecordError::NotConfirmed => "You're not in this matter yet.".into(),
        RecordError::NotAdmin => "Only an admin of this matter can do that.".into(),
        RecordError::WaitingForMembershipUpdate => "Waiting for the latest change to who's in this matter to arrive in the folder. Try again shortly.".into(),
        RecordError::InviteNotOpen => "This invite has already been used or withdrawn. Ask for a fresh one.".into(),
        RecordError::ForeignMatter => "This invite is for a different matter folder.".into(),
        other => other.to_string(),
    }
}

/// A working-copy file name built from allowed characters only, so a member's file name can't reach
/// outside the working folder, an NTFS stream, or a Windows device name.
pub fn safe_basename(name: &str) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !e.is_empty() => (s, e),
        _ => (name, ""),
    };
    let keep = |c: char| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '(' | ')');
    let mut stem: String = stem.chars().map(|c| if keep(c) { c } else { '_' }).collect::<String>().trim().to_string();
    if stem.is_empty() {
        stem = "file".into();
    }
    if WINDOWS_RESERVED.contains(&stem.to_ascii_uppercase().as_str()) {
        stem.insert(0, '_');
    }
    let ext: String = ext.chars().filter(char::is_ascii_alphanumeric).take(MAX_EXTENSION_CHARS).collect();
    let ext = if ext.is_empty() { ext } else { format!(".{ext}") };
    let room = MAX_BASENAME_CHARS - ext.chars().count();
    stem.chars().take(room).collect::<String>() + &ext
}

pub fn openable(name: &str) -> bool {
    extension(name).is_some_and(|e| OPENABLE_EXTENSIONS.contains(&e.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCX: &[u8] = b"v1 content";

    struct Laptop {
        session: Session,
        _base: tempfile::TempDir,
    }

    fn laptop() -> Laptop {
        let base = tempfile::tempdir().unwrap();
        Laptop { session: Session::new(base.path()), _base: base }
    }

    fn write(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    /// Ana creates a matter with nda.docx at v1.
    fn matter(ana: &mut Laptop, folder: &Path) -> (Hash, Uuid) {
        let id = hex_id(&ana.session.create(folder, "Ana Lim").unwrap().id);
        let source = tempfile::tempdir().unwrap();
        ana.session.add_file(id, &write(source.path(), "nda.docx", DOCX)).unwrap();
        let file = ana.session.detail(id).unwrap().files[0].id.parse().unwrap();
        (id, file)
    }

    fn hex_id(s: &str) -> Hash {
        hex::decode(s).unwrap().try_into().unwrap()
    }

    /// Ben answers Ana's invite; both tap "Numbers match" on the number each side computed.
    fn admit(ana: &Laptop, ben: &mut Laptop, id: Hash, folder: &Path) {
        let link = ana.session.invite(id, "Ben Tan").unwrap();
        ben.session.join(folder, &link, "Ben Tan").unwrap();
        let request = ana.session.detail(id).unwrap().requests.remove(0);
        let device = hex_id(&request.device);
        ana.session.approve(id, device, &request.safety_number.unwrap()).unwrap();
        let number = ben.session.detail(id).unwrap().safety_number.unwrap();
        ben.session.confirm(id, &number).unwrap();
    }

    fn labels(detail: &MatterDetail) -> Vec<(String, String)> {
        detail.files[0].versions.iter().map(|v| (v.label.clone(), v.author.clone())).collect()
    }

    fn save_in_word(path: &Path, content: &[u8]) {
        fs::write(path, content).unwrap();
    }

    #[test]
    fn a_dropped_file_is_version_one_by_its_author() {
        let (mut ana, folder) = (laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        assert_eq!(labels(&ana.session.detail(id).unwrap()), vec![("1".to_string(), "Ana Lim".to_string())]);
    }

    #[test]
    fn a_save_in_word_becomes_the_next_version_on_the_next_look() {
        let (mut ana, folder) = (laptop(), tempfile::tempdir().unwrap());
        let (id, file) = matter(&mut ana, folder.path());
        save_in_word(&ana.session.check_out(id, file).unwrap(), b"v2 content");
        let got: Vec<String> = ana.session.detail(id).unwrap().files[0].versions.iter().map(|v| v.label.clone()).collect();
        assert_eq!(got, vec!["2", "1"]);
    }

    #[test]
    fn a_working_copy_refreshed_from_the_head_is_not_a_new_version() {
        let (mut ana, folder) = (laptop(), tempfile::tempdir().unwrap());
        let (id, file) = matter(&mut ana, folder.path());
        ana.session.check_out(id, file).unwrap();
        ana.session.check_out(id, file).unwrap();
        assert_eq!(ana.session.detail(id).unwrap().files[0].versions.len(), 1);
    }

    #[test]
    fn a_colleagues_version_shows_with_their_name() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = matter(&mut ana, folder.path());
        admit(&ana, &mut ben, id, folder.path());
        save_in_word(&ben.session.check_out(id, file).unwrap(), b"ben's v2");
        ben.session.detail(id).unwrap();
        assert_eq!(labels(&ana.session.detail(id).unwrap())[0], ("2".to_string(), "Ben Tan".to_string()));
    }

    #[test]
    fn the_joiner_and_the_admin_see_the_same_safety_number() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        let link = ana.session.invite(id, "Ben Tan").unwrap();
        ben.session.join(folder.path(), &link, "Ben Tan").unwrap();
        let admin_side = ana.session.detail(id).unwrap().requests.remove(0).safety_number;
        assert_eq!(admin_side, ben.session.detail(id).unwrap().safety_number);
    }

    #[test]
    fn a_joiner_waits_for_the_admin_who_issued_the_link() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        let joined = ben.session.join(folder.path(), &ana.session.invite(id, "Ben Tan").unwrap(), "Ben Tan").unwrap();
        assert_eq!((joined.standing, joined.waiting_for.as_deref()), (Standing::Waiting, Some("Ana Lim")));
    }

    #[test]
    fn a_joiner_is_a_member_once_approved_and_confirmed() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        admit(&ana, &mut ben, id, folder.path());
        assert_eq!(ben.session.matters().unwrap()[0].standing, Standing::Member);
    }

    /// Ana and Ben each save over v2 before seeing the other's save.
    fn fork(ana: &mut Laptop, ben: &mut Laptop, folder: &Path) -> (Hash, Uuid) {
        let (id, file) = matter(ana, folder);
        admit(ana, ben, id, folder);
        let (a, b) = (ana.session.check_out(id, file).unwrap(), ben.session.check_out(id, file).unwrap());
        save_in_word(&a, b"ana's v2");
        save_in_word(&b, b"ben's v2");
        ana.session.detail(id).unwrap();
        ben.session.detail(id).unwrap();
        (id, file)
    }

    #[test]
    fn two_edits_of_one_version_are_labelled_with_depth_and_initials() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = fork(&mut ana, &mut ben, folder.path());
        let mut got: Vec<String> = ana.session.detail(id).unwrap().files[0].versions.iter().map(|v| v.label.clone()).collect();
        got.sort();
        assert_eq!(got, vec!["1", "2-AL", "2-BT"]);
    }

    #[test]
    fn a_forked_file_names_the_version_both_edited() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = fork(&mut ana, &mut ben, folder.path());
        assert_eq!(ana.session.detail(id).unwrap().files[0].forked_from.as_deref(), Some("1"));
    }

    #[test]
    fn a_forked_file_does_not_open_until_combined() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = fork(&mut ana, &mut ben, folder.path());
        assert!(ana.session.check_out(id, file).is_err());
    }

    fn version_by(detail: &MatterDetail, author: &str) -> Hash {
        hex_id(&detail.files[0].versions.iter().find(|v| v.head && v.author == author).unwrap().id)
    }

    #[test]
    fn using_one_side_makes_one_head_with_that_content() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = fork(&mut ana, &mut ben, folder.path());
        let bens = version_by(&ana.session.detail(id).unwrap(), "Ben Tan");
        ana.session.resolve(id, file, bens).unwrap();
        assert_eq!(fs::read(ana.session.check_out(id, file).unwrap()).unwrap(), b"ben's v2");
    }

    #[test]
    fn the_same_choice_on_two_laptops_collapses_into_one_version() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = fork(&mut ana, &mut ben, folder.path());
        let bens = version_by(&ana.session.detail(id).unwrap(), "Ben Tan");
        // Ana's choice has not reached the folder yet when Ben makes the same one.
        let before: Vec<_> = fs::read_dir(folder.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        ana.session.resolve(id, file, bens).unwrap();
        let aside = tempfile::tempdir().unwrap();
        for e in fs::read_dir(folder.path()).unwrap().map(|e| e.unwrap()).filter(|e| !before.contains(&e.file_name())) {
            fs::rename(e.path(), aside.path().join(e.file_name())).unwrap();
        }
        ben.session.resolve(id, file, bens).unwrap();
        for e in fs::read_dir(aside.path()).unwrap().map(|e| e.unwrap()) {
            fs::rename(e.path(), folder.path().join(e.file_name())).unwrap();
        }
        let heads = ana.session.detail(id).unwrap().files[0].versions.iter().filter(|v| v.head).count();
        assert_eq!(heads, 1);
    }

    #[test]
    fn combine_opens_mine_with_theirs_beside_it() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = fork(&mut ana, &mut ben, folder.path());
        let paths = ana.session.combine(id, file).unwrap();
        let got: Vec<Vec<u8>> = paths.iter().map(|p| fs::read(p).unwrap()).collect();
        assert_eq!(got, vec![b"ana's v2".to_vec(), b"ben's v2".to_vec()]);
    }

    #[test]
    fn saving_a_combined_copy_joins_both_sides() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = fork(&mut ana, &mut ben, folder.path());
        save_in_word(&ana.session.combine(id, file).unwrap()[0], b"combined v3");
        let detail = ana.session.detail(id).unwrap();
        let heads: Vec<&str> = detail.files[0].versions.iter().filter(|v| v.head).map(|v| v.label.as_str()).collect();
        assert_eq!(heads, vec!["3"]);
    }

    #[test]
    fn a_third_invite_waits_for_a_second_admin() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        admit(&ana, &mut ben, id, folder.path());
        assert!(ana.session.detail(id).unwrap().needs_second_admin);
    }

    #[test]
    fn basenames_keep_only_allowed_characters() {
        assert_eq!(safe_basename(r"..\..\evil:Zone.Identifier.docx"), "______evil_Zone_Identifier.docx");
    }

    #[test]
    fn windows_device_names_are_prefixed() {
        assert_eq!(safe_basename("con.docx"), "_con.docx");
    }

    #[test]
    fn long_names_are_capped_with_their_extension() {
        let name = safe_basename(&format!("{}.docx", "a".repeat(300)));
        assert_eq!((name.chars().count(), name.ends_with(".docx")), (MAX_BASENAME_CHARS, true));
    }

    #[test]
    fn only_macro_free_types_open() {
        let got: Vec<bool> = ["nda.docx", "nda.DOCX", "nda.docm", "x.lnk", "x.html", "x.exe"].iter().map(|n| openable(n)).collect();
        assert_eq!(got, vec![true, true, false, false, false, false]);
    }
        #[test]
    fn two_answers_to_one_invite_show_as_contested() {
        let (mut ana, mut ben, mut mal, folder) = (laptop(), laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        let link = ana.session.invite(id, "Ben Tan").unwrap();
        ben.session.join(folder.path(), &link, "Ben Tan").unwrap();
        mal.session.join(folder.path(), &link, "Mal").unwrap();
        assert_eq!(ana.session.detail(id).unwrap().contested.len(), 1);
    }

    #[test]
    fn withdrawing_a_contested_invite_clears_it() {
        let (mut ana, mut ben, mut mal, folder) = (laptop(), laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        let link = ana.session.invite(id, "Ben Tan").unwrap();
        ben.session.join(folder.path(), &link, "Ben Tan").unwrap();
        mal.session.join(folder.path(), &link, "Mal").unwrap();
        let invite = ana.session.detail(id).unwrap().contested.remove(0).parse().unwrap();
        ana.session.withdraw(id, invite).unwrap();
        assert!(ana.session.detail(id).unwrap().contested.is_empty());
    }

    #[test]
    fn a_save_that_cannot_be_recorded_still_shows_the_matter() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = matter(&mut ana, folder.path());
        admit(&ana, &mut ben, id, folder.path());
        let copy = ben.session.check_out(id, file).unwrap();
        ana.session.briefcase(id).unwrap().remove(ben.session.keys.signer()).unwrap();
        save_in_word(&copy, b"after removal");
        assert_eq!(ben.session.detail(id).unwrap().not_recorded, vec!["nda.docx".to_string()]);
    }

    #[test]
    fn joining_from_a_folder_without_the_matter_is_refused() {
        let (mut ana, mut ben, folder, elsewhere) = (laptop(), laptop(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        let link = ana.session.invite(id, "Ben Tan").unwrap();
        assert!(ben.session.join(elsewhere.path(), &link, "Ben Tan").is_err());
    }

    #[test]
    fn answering_a_second_link_needs_its_number_confirmed_again() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, _) = matter(&mut ana, folder.path());
        ben.session.join(folder.path(), &ana.session.invite(id, "Ben Tan").unwrap(), "Ben Tan").unwrap();
        let number = ben.session.detail(id).unwrap().safety_number.unwrap();
        ben.session.confirm(id, &number).unwrap();
        ben.session.join(folder.path(), &ana.session.invite(id, "Ben Tan").unwrap(), "Ben Tan").unwrap();
        assert!(!ben.session.detail(id).unwrap().confirmed);
    }

    #[test]
    fn an_unreachable_matter_is_listed_as_unavailable() {
        let (mut ana, folder) = (laptop(), tempfile::tempdir().unwrap());
        matter(&mut ana, folder.path());
        fs::remove_dir_all(ana.session.root.join("briefcases")).unwrap();
        fs::write(ana.session.root.join("briefcases"), b"not a folder").unwrap();
        assert!(ana.session.matters().unwrap()[0].unavailable.is_some());
    }

    #[test]
    fn closing_records_saved_copies_then_removes_this_launchs_files() {
        let (mut ana, mut ben, folder) = (laptop(), laptop(), tempfile::tempdir().unwrap());
        let (id, file) = matter(&mut ana, folder.path());
        admit(&ana, &mut ben, id, folder.path());
        save_in_word(&ben.session.check_out(id, file).unwrap(), b"saved before quit");
        ben.session.close().unwrap();
        let versions = ana.session.detail(id).unwrap().files[0].versions.len();
        assert_eq!((versions, ben.session.root.exists()), (2, false));
    }

    #[test]
    fn a_long_history_numbers_without_overflowing_the_stack() {
        let mut state = State::default();
        let mut parent: Option<Hash> = None;
        for i in 0..20_000u32 {
            let mut hash = [0u8; 32];
            hash[..4].copy_from_slice(&i.to_be_bytes());
            state.versions.insert(hash, version_header(parent));
            parent = Some(hash);
        }
        assert_eq!(depth(&state, parent.unwrap(), &mut HashMap::new()), 20_000);
    }

    #[test]
    fn the_banner_names_where_the_heads_diverged_not_an_earlier_merged_fork() {
        // v1 → a, b; a + b → m (merged); m → x, y: the heads x and y diverged at m.
        let mut state = State::default();
        let h = |n: u8| [n; 32];
        state.versions.insert(h(1), version_header(None));
        state.versions.insert(h(2), version_header(Some(h(1))));
        state.versions.insert(h(3), version_header(Some(h(1))));
        let mut merge = version_header(Some(h(2)));
        merge.parents.push(h(3));
        state.versions.insert(h(4), merge);
        state.versions.insert(h(5), version_header(Some(h(4))));
        state.versions.insert(h(6), version_header(Some(h(4))));
        let heads = [h(5), h(6)];
        assert_eq!(diverged_at(&state, &heads), Some(h(4)));
    }

    fn version_header(parent: Option<Hash>) -> Header {
        Header {
            briefcase_id: [0; 32],
            uuid: Uuid::nil(),
            signer: [0; 32],
            seq: 1,
            prev: None,
            parents: parent.into_iter().collect(),
            admin_head: None,
            body: Body::Version(VersionFields {
                file_id: Uuid::nil(),
                version_id: Uuid::nil(),
                epoch: 1,
                file_name: "nda.docx".into(),
                media_type: String::new(),
                size: 0,
                created_at_ms: 0,
                content_sha256: [0; 32],
            }),
        }
    }
}
