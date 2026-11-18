//! ABOUTME: One briefcase's record set in a transport folder, and the import fold that judges it.
//! ABOUTME: Write-once records, local ciphertext copies, restore names; membership at each record's admin_head.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use age::x25519;
use rand_core::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::sync_crypto::{
    self, Body, DeviceFields, DeviceKeys, Epoch, Hash, Header, KeyFields, Link, Member, Membership, PublicKey,
    RecordError, RequestFields, VersionFields,
};

/// Opens the plaintext genesis file, which holds a nonce and the creator's admin key and nothing else.
const GENESIS_MAGIC: &[u8] = b"rumble-genesis-v1";
const NONCE_BYTES: usize = 32;
const RECORD_SUFFIX: &str = ".rumble";
const UUID_CHARS: usize = 36;
/// Hex digits of the ciphertext hash in a restore name, `<uuid>-<hash8>.rumble`.
const RESTORE_HASH_HEX: usize = 8;
const FIRST_EPOCH: u64 = 1;
const READ_CHUNK_BYTES: usize = 64 * 1024;
/// An invite stops admitting requests this long after its admin issued it, by that admin's clock.
const INVITE_TTL_DAYS: i64 = 7;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// The signed link this device answered, kept locally to compute its side of the safety number.
const LINK_FILE: &str = "link";
/// Present once this device's user tapped "Numbers match" (or created the matter): writes allowed.
const CONFIRMED_FILE: &str = "confirmed";

/// A verified record in this device's store: its signature, uuid and matter have been checked.
struct Record {
    uuid: Uuid,
    ciphertext: Hash,
    path: PathBuf,
    header: Header,
    /// For a version: the recipients of every held epoch carrying its signed number that opens it.
    signed_epoch_opens: Vec<String>,
    /// The identity a `key` record carries.
    epoch: Option<Epoch>,
}

struct Genesis {
    uuid: Uuid,
    path: PathBuf,
    admin: PublicKey,
}

#[derive(Default)]
struct Opened {
    genesis: Option<Genesis>,
    records: Vec<Record>,
    /// Every epoch a verified `key` record carries, accepted or not: enough to decrypt, not to write.
    epochs: Vec<Epoch>,
    seen: HashSet<(Uuid, Hash)>,
}

/// The admin chain as it stood at some heads.
#[derive(Default)]
struct View {
    membership: Membership,
    invites: HashMap<Uuid, Vec<u8>>,
    closed: HashSet<Uuid>,
    named: HashSet<(u64, String)>,
    /// Admin files that count for nothing here: a removal whose signer another removal removed past it
    /// (two admins removing each other concurrently both lose), and anything else an effectively
    /// removed admin signed past its cutoff (no backdated branch).
    void: HashSet<Hash>,
}

/// One file's version graph. Identical versions (same parents, same content) count once.
#[derive(Debug, Default, PartialEq)]
pub struct FileHistory {
    pub versions: BTreeSet<Hash>,
    pub heads: BTreeSet<Hash>,
    /// Versions that two or more distinct versions name as a parent.
    pub forks: BTreeSet<Hash>,
}

/// Why admins are told about a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    /// Two different records with one signer and seq: the device is compromised.
    Equivocation,
    /// It and another admin removed each other concurrently; both removals are void.
    MutualRemoval,
}

/// What this device makes of the briefcase after one fold. Derived from the store alone, every time.
#[derive(Default)]
pub struct State {
    id: Hash,
    genesis_admin: Option<PublicKey>,
    /// Accepted admin-chain (`member`) records by record hash.
    admin: HashMap<Hash, Header>,
    /// Membership at this device's own head: every accepted approval, with removal cutoffs.
    pub roll: Membership,
    pub epochs: Vec<Epoch>,
    /// Device → (seq, fields) of its newest self-description.
    pub devices: HashMap<PublicKey, (u64, DeviceFields)>,
    /// The admins' inbox: device → (the open invite it answered, its newest request). Never matter state.
    pub requests: HashMap<PublicKey, (Uuid, DeviceFields)>,
    /// Open invites that two devices answered: "Two people answered this invite". Neither is approvable.
    pub contested: BTreeSet<Uuid>,
    /// Every invite issued in the admin chain → its signed link bytes.
    invites: HashMap<Uuid, Vec<u8>>,
    /// Invites approved or revoked.
    closed: HashSet<Uuid>,
    pub versions: HashMap<Hash, Header>,
    pub files: BTreeMap<Uuid, FileHistory>,
    pub held: Vec<Uuid>,
    pub rejected: Vec<(Uuid, RecordError)>,
    /// Transport files that did not verify this time; retried on the next sync.
    pub unverified: Vec<(String, RecordError)>,
    /// An admin file a member's record names that this device hasn't seen: new writes wait for it.
    pub write_block: Option<Hash>,
    pub flags: BTreeSet<(PublicKey, Flag)>,
    own: Option<(u64, Hash)>,
    own_signer: Option<PublicKey>,
    accepted: HashMap<Hash, (Uuid, Hash, PathBuf)>,
    /// Every accepted record's header, for walking a signer's own chain.
    signed: HashMap<Hash, Header>,
    /// Every verified record's header, accepted or not: what a removal cutoff's `prev` chain walks.
    verified: HashMap<Hash, Header>,
    /// Accepted admin files with no effect in the merged chain (see `View::void`).
    void: HashSet<Hash>,
    /// Every epoch an admin file names: (number, public recipient).
    named: HashSet<(u64, String)>,
    restorable_genesis: Option<(Uuid, PathBuf)>,
}

impl State {
    pub fn is_member(&self, device: &PublicKey) -> bool {
        self.roll.members.contains(device) && !self.roll.removed.contains_key(device)
    }

    pub fn current_epoch(&self) -> Option<&Epoch> {
        self.epochs.iter().max_by_key(|e| (e.number, e.recipient().to_string()))
    }

    fn tips(&self) -> Vec<Hash> {
        let named: HashSet<&Hash> = self.admin.values().flat_map(|h| &h.parents).collect();
        let mut tips: Vec<Hash> = self.admin.keys().filter(|h| !named.contains(h)).copied().collect();
        tips.sort();
        if tips.is_empty() {
            tips.push(self.id);
        }
        tips
    }

    /// The newest admin file this device has seen at which `signer` is a member, as its `admin_head`.
    fn admin_head(&self, signer: &PublicKey) -> Hash {
        let tips = self.tips();
        tips.iter().find(|t| self.membership_at(&[**t]).members.contains(signer)).copied().unwrap_or(tips[0])
    }

    /// Membership as the admin chain stood at `heads`: their ancestors' approvals, plus every cutoff.
    fn membership_at(&self, heads: &[Hash]) -> Membership {
        Membership { removed: self.roll.removed.clone(), ..self.view(heads).membership }
    }

    /// Every accepted admin file `heads` reach, `heads` included.
    fn ancestors(&self, heads: &[Hash]) -> Vec<(Hash, &Header)> {
        let mut stack = heads.to_vec();
        let mut visited = HashSet::new();
        let mut chain = Vec::new();
        while let Some(h) = stack.pop() {
            let Some(header) = self.admin.get(&h).filter(|_| visited.insert(h)) else { continue };
            chain.push((h, header));
            stack.extend(&header.parents);
        }
        chain
    }

    /// The admin chain as it stood at `heads`: who is an admin or a member, and which invites exist.
    /// Files void in the whole merged chain count for nothing in any part of it.
    fn view(&self, heads: &[Hash]) -> View {
        let mut v = View::default();
        let chain = self.ancestors(heads);
        let removals: HashMap<Hash, Removal> = chain
            .iter()
            .filter_map(|(hash, h)| match h.body {
                Body::Member(Member::Remove { device, cutoff }) => {
                    let seen = self.ancestors(&[*hash]).into_iter().map(|(a, _)| a).collect();
                    Some((*hash, Removal { signer: h.signer, device, kept: kept_chain(&self.verified, device, cutoff), seen }))
                }
                _ => None,
            })
            .collect();
        v.void.extend(removals.iter().filter(|(hash, r)| removed_past(&removals, hash, &r.signer)).map(|(h, _)| *h));
        v.void.extend(&self.void);
        let effective: HashMap<Hash, Removal> =
            removals.iter().filter(|(hash, _)| !v.void.contains(*hash)).map(|(h, r)| (*h, r.clone())).collect();
        for (hash, h) in &chain {
            if !removals.contains_key(hash) && removed_past(&effective, hash, &h.signer) {
                v.void.insert(*hash);
            }
        }
        // An approval and a revoke of one invite can both stand only on concurrent branches: the revoke wins.
        let revoked: HashSet<Uuid> = chain
            .iter()
            .filter(|(hash, _)| !v.void.contains(hash))
            .filter_map(|(_, h)| match h.body {
                Body::Member(Member::Revoke { invite }) => Some(invite),
                _ => None,
            })
            .collect();
        for (hash, h) in &chain {
            if matches!(h.body, Body::Member(Member::Approve { invite: Some(i), .. }) if revoked.contains(&i)) {
                v.void.insert(*hash);
            }
        }
        for r in effective.values() {
            v.membership.removed.entry(r.device).and_modify(|k| k.retain(|h| r.kept.contains(h))).or_insert_with(|| r.kept.clone());
        }
        v.membership.admins.extend(self.genesis_admin);
        v.membership.members.extend(self.genesis_admin);
        for (_, header) in chain.iter().filter(|(hash, _)| !v.void.contains(hash)) {
            match &header.body {
                Body::Member(Member::Approve { device, admin, invite }) => {
                    v.membership.members.insert(*device);
                    if *admin {
                        v.membership.admins.insert(*device);
                    }
                    v.closed.extend(invite);
                }
                Body::Member(Member::Invite { link }) => {
                    let invite = Link::verify(link).expect("accepted invites verify").invite;
                    v.invites.insert(invite, link.clone());
                }
                Body::Member(Member::Revoke { invite }) => {
                    v.closed.insert(*invite);
                }
                Body::Member(Member::Rotate { epoch, recipient }) => {
                    v.named.insert((*epoch, recipient.clone()));
                }
                _ => {}
            }
        }
        let removed = &v.membership.removed;
        v.membership.admins.retain(|a| !removed.contains_key(a));
        v
    }

    /// The admin's side of the safety number for a request: computed from its own stored `invite`
    /// record, never from the link the request carries.
    pub fn safety_number(&self, device: &PublicKey) -> Option<String> {
        let (invite, _) = self.requests.get(device)?;
        Some(sync_crypto::safety_number(self.invites.get(invite)?, device))
    }

    /// The receiving keys of every other device approved and not removed, as their own records say.
    fn member_recipients(&self) -> Result<Vec<x25519::Recipient>, RecordError> {
        self.devices
            .iter()
            .filter(|(signer, _)| self.is_member(signer) && Some(**signer) != self.own_signer)
            .map(|(_, (_, fields))| fields.recipient.parse().map_err(|_| RecordError::Malformed("recipient")))
            .collect()
    }

    fn knows_admin_file(&self, h: &Hash) -> bool {
        *h == self.id || self.admin.contains_key(h)
    }

    /// The end of a device's accepted chain walked from seq 1 with no gap: what a removal names as its
    /// cutoff. A record whose predecessor this device never accepted is not part of it.
    fn last_accepted(&self, device: &PublicKey) -> Option<Hash> {
        let next: HashMap<Option<Hash>, Hash> = self
            .signed
            .iter()
            .filter(|(_, h)| h.signer == *device && !matches!(h.body, Body::Key(_)))
            .map(|(hash, h)| (h.prev, *hash))
            .collect();
        let mut tip = next.get(&None).copied();
        while let Some(child) = tip.and_then(|t| next.get(&Some(t))) {
            tip = Some(*child);
        }
        tip
    }
}

/// This device's handle on one briefcase: records in `transport`, its own ciphertext copies under `data`.
pub struct Briefcase<'k> {
    pub id: Hash,
    keys: &'k DeviceKeys,
    transport: PathBuf,
    root: PathBuf,
    store: PathBuf,
    temp: PathBuf,
}

impl<'k> Briefcase<'k> {
    pub fn open(id: Hash, keys: &'k DeviceKeys, transport: &Path, data: &Path) -> io::Result<Self> {
        let root = data.join("briefcases").join(hex::encode(id));
        let (store, temp) = (root.join("store"), root.join("tmp"));
        fs::create_dir_all(&store)?;
        fs::create_dir_all(&temp)?;
        Ok(Briefcase { id, keys, transport: transport.into(), root, store, temp })
    }

    /// Start a briefcase: the genesis file, the first epoch's `key` file, and this device's own record.
    pub fn create(keys: &'k DeviceKeys, transport: &Path, data: &Path, display_name: &str) -> Result<Self, RecordError> {
        let mut nonce = [0u8; NONCE_BYTES];
        rand_core::OsRng.fill_bytes(&mut nonce);
        let genesis = [GENESIS_MAGIC, &nonce, &keys.signer()].concat();
        let briefcase = Briefcase::open(Sha256::digest(&genesis).into(), keys, transport, data)?;
        fs::write(briefcase.root.join(CONFIRMED_FILE), b"")?;
        briefcase.publish(Uuid::new_v4(), |out| Ok(io::Write::write_all(out, &genesis)?))?;
        let mut state = briefcase.sync()?;
        briefcase.rotate(&mut state)?;
        let mut state = briefcase.sync()?;
        let epoch = state.current_epoch().ok_or(RecordError::NotMember)?.recipient();
        briefcase.describe(&mut state, &epoch, display_name)?;
        Ok(briefcase)
    }

    /// Issue an invite in the admin chain; returns the signed link bytes, which carry no key.
    pub fn invite(&self, invitee_name: &str) -> Result<Vec<u8>, RecordError> {
        let mut state = self.writable_as_admin()?;
        if !state.invites.is_empty() && state.roll.admins.len() < 2 {
            return Err(RecordError::NeedsSecondAdmin);
        }
        let me = self.keys.signer();
        let link = Link {
            invite: Uuid::new_v4(),
            briefcase_id: self.id,
            epoch_recipient: state.current_epoch().ok_or(RecordError::NotMember)?.recipient().to_string(),
            admin: me,
            admin_name: state.devices.get(&me).map(|(_, d)| d.display_name.clone()).unwrap_or_default(),
            invitee_name: invitee_name.into(),
            issued_at_ms: chrono::Utc::now().timestamp_millis(),
        }
        .sign(self.keys)?;
        self.admin_record(&mut state, Member::Invite { link: link.clone() })?;
        Ok(link)
    }

    pub fn revoke(&self, invite: Uuid) -> Result<Hash, RecordError> {
        let mut state = self.writable_as_admin()?;
        self.admin_record(&mut state, Member::Revoke { invite })
    }

    /// A join request answering `link`: this device's own record and the link, readable by members only
    /// (through the link's epoch recipient). The link is kept locally for this side's safety number.
    pub fn request(&self, link: &[u8], display_name: &str) -> Result<Hash, RecordError> {
        let parsed = Link::verify(link)?;
        if parsed.briefcase_id != self.id {
            return Err(RecordError::ForeignMatter);
        }
        let epoch: x25519::Recipient = parsed.epoch_recipient.parse().map_err(|_| RecordError::Malformed("recipient"))?;
        fs::write(self.root.join(LINK_FILE), link)?;
        let mut state = self.sync()?;
        let me = self.keys.recipient();
        let device = DeviceFields { display_name: display_name.into(), recipient: me.to_string() };
        let body = Body::Request(RequestFields { device, link: link.into() });
        let head = Some(state.admin_head(&self.keys.signer()));
        self.seal_record(&mut state, vec![], head, body, |header, out| {
            sync_crypto::seal(self.keys, header, io::empty(), &[&epoch, &me], out)
        })
    }

    /// The joiner's side of the safety number, from the link it holds.
    pub fn safety_number(&self) -> Result<String, RecordError> {
        Ok(sync_crypto::safety_number(&fs::read(self.root.join(LINK_FILE))?, &self.keys.signer()))
    }

    /// The joiner taps "Numbers match": only then does this device write to the matter.
    pub fn confirm(&self, number: &str) -> Result<(), RecordError> {
        if number != self.safety_number()? {
            return Err(RecordError::SafetyNumberMismatch);
        }
        Ok(fs::write(self.root.join(CONFIRMED_FILE), b"")?)
    }

    /// The admin taps "Numbers match": approve the device, closing its invite, and wrap every epoch to it.
    pub fn approve(&self, device: PublicKey, admin: bool, number: &str) -> Result<Hash, RecordError> {
        let mut state = self.writable_as_admin()?;
        if state.safety_number(&device).as_deref() != Some(number) {
            return Err(RecordError::SafetyNumberMismatch);
        }
        let (invite, fields) = &state.requests[&device];
        let invite = Some(*invite);
        let joiner: x25519::Recipient = fields.recipient.parse().map_err(|_| RecordError::Malformed("recipient"))?;
        let devices = [vec![joiner, self.keys.recipient()], state.member_recipients()?].concat();
        let approval = self.admin_record(&mut state, Member::Approve { device, admin, invite })?;
        for epoch in state.epochs.clone() {
            self.write_key_file(&mut state, vec![approval], &epoch, &devices)?;
        }
        Ok(approval)
    }

    /// Make a member an admin.
    pub fn promote(&self, device: PublicKey) -> Result<Hash, RecordError> {
        let mut state = self.writable_as_admin()?;
        self.admin_record(&mut state, Member::Approve { device, admin: true, invite: None })
    }

    /// Remove a device after its last record this device accepted, then move the matter to a new epoch.
    pub fn remove(&self, device: PublicKey) -> Result<Hash, RecordError> {
        let mut state = self.writable_as_admin()?;
        let cutoff = state.last_accepted(&device);
        self.admin_record(&mut state, Member::Remove { device, cutoff })
    }

    /// A fresh epoch, named in the admin chain on every tip and wrapped only to devices approved and not
    /// removed. Leaves `state` freshly synced.
    fn rotate(&self, state: &mut State) -> Result<Hash, RecordError> {
        *state = self.sync()?;
        let tips = state.tips();
        let epoch = Epoch::generate(next_epoch(&state.view(&tips)));
        let body = Body::Member(Member::Rotate { epoch: epoch.number, recipient: epoch.recipient().to_string() });
        let rotation = self.seal_record(state, tips, None, body, |header, out| {
            sync_crypto::seal(self.keys, header, io::empty(), &[&epoch.recipient()], out)
        })?;
        let devices = [vec![self.keys.recipient()], state.member_recipients()?].concat();
        self.write_key_file(state, vec![rotation], &epoch, &devices)
    }

    pub fn save_version(
        &self,
        file_id: Uuid,
        file_name: &str,
        media_type: &str,
        content: &Path,
        parents: Vec<Hash>,
    ) -> Result<Hash, RecordError> {
        let mut state = self.writable()?;
        let epoch = state.current_epoch().ok_or(RecordError::NotMember)?.clone();
        let (size, content_sha256) = hash_file(content)?;
        let body = Body::Version(VersionFields {
            file_id,
            version_id: Uuid::new_v4(),
            epoch: epoch.number,
            file_name: file_name.into(),
            media_type: media_type.into(),
            size,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
            content_sha256,
        });
        let head = Some(state.admin_head(&self.keys.signer()));
        self.seal_record(&mut state, parents, head, body, |header, out| {
            sync_crypto::seal(self.keys, header, File::open(content)?, &[&epoch.recipient()], out)
        })
    }

    /// Verify a version against the fold, then release its content to `dest` through local temp space.
    pub fn release(&self, state: &State, version: &Hash, dest: &Path) -> Result<Header, RecordError> {
        let header = state.versions.get(version).ok_or(RecordError::Malformed("unknown version"))?;
        let (uuid, _, path) = &state.accepted[version];
        let membership = state.membership_at(&[header.admin_head.expect("accepted versions name an admin_head")]);
        sync_crypto::release_version(File::open(path)?, *uuid, &self.id, &state.epochs, &membership, &self.temp, dest)
    }

    /// Import every new record from the transport, fold the store, then write back what vanished.
    pub fn sync(&self) -> Result<State, RecordError> {
        loop {
            let opened = self.open_store()?;
            let (imported, present, unverified) = self.import(&opened)?;
            if imported == 0 {
                let now_ms = chrono::Utc::now().timestamp_millis();
                let state = fold(self.id, self.keys.signer(), now_ms, opened, unverified);
                self.write_back(&state, &present)?;
                return Ok(state);
            }
        }
    }

    fn writable(&self) -> Result<State, RecordError> {
        let state = self.sync()?;
        if state.write_block.is_some() {
            return Err(RecordError::WaitingForMembershipUpdate);
        }
        if !state.is_member(&self.keys.signer()) {
            return Err(RecordError::NotMember);
        }
        if !self.root.join(CONFIRMED_FILE).exists() {
            return Err(RecordError::NotConfirmed);
        }
        Ok(state)
    }

    fn writable_as_admin(&self) -> Result<State, RecordError> {
        let state = self.writable()?;
        match state.roll.admins.contains(&self.keys.signer()) {
            true => Ok(state),
            false => Err(RecordError::NotAdmin),
        }
    }

    fn describe(&self, state: &mut State, epoch: &x25519::Recipient, display_name: &str) -> Result<Hash, RecordError> {
        let me = self.keys.recipient();
        let body = Body::Device(DeviceFields { display_name: display_name.into(), recipient: me.to_string() });
        let head = Some(state.admin_head(&self.keys.signer()));
        self.seal_record(state, vec![], head, body, |header, out| {
            sync_crypto::seal(self.keys, header, io::empty(), &[epoch, &me], out)
        })
    }

    /// Write an admin file on every tip. A removal, or a file that merges admin branches, then moves the
    /// matter to a fresh epoch, so no device removed in any branch reads what follows.
    fn admin_record(&self, state: &mut State, member: Member) -> Result<Hash, RecordError> {
        let epoch = state.current_epoch().ok_or(RecordError::NotMember)?.recipient();
        let parents = state.tips();
        let rotates = parents.len() > 1 || matches!(member, Member::Remove { .. });
        let hash = self.seal_record(state, parents, None, Body::Member(member), |header, out| {
            sync_crypto::seal(self.keys, header, io::empty(), &[&epoch], out)
        })?;
        if rotates {
            self.rotate(state)?;
        }
        Ok(hash)
    }

    fn write_key_file(
        &self,
        state: &mut State,
        parents: Vec<Hash>,
        epoch: &Epoch,
        devices: &[x25519::Recipient],
    ) -> Result<Hash, RecordError> {
        let body = Body::Key(KeyFields { epoch: epoch.number, epoch_recipient: epoch.recipient().to_string() });
        self.seal_record(state, parents, None, body, |header, out| {
            sync_crypto::seal_key_file(self.keys, header, epoch, devices, out)
        })
    }

    /// Sign the next record in this device's own chain and publish it.
    fn seal_record(
        &self,
        state: &mut State,
        parents: Vec<Hash>,
        admin_head: Option<Hash>,
        body: Body,
        seal: impl FnOnce(&Header, &mut File) -> Result<(), RecordError>,
    ) -> Result<Hash, RecordError> {
        // `key` files stand outside the signer's chain: they are wrapped to some devices only, so a chain
        // through them would break for everyone else. The admin file that names their epoch orders them.
        let chained = !matches!(body, Body::Key(_));
        let header = Header {
            briefcase_id: self.id,
            uuid: Uuid::new_v4(),
            signer: self.keys.signer(),
            seq: if chained { state.own.map_or(1, |(seq, _)| seq + 1) } else { 0 },
            prev: state.own.filter(|_| chained).map(|(_, h)| h),
            parents,
            admin_head,
            body,
        };
        self.publish(header.uuid, |out| seal(&header, out))?;
        let hash = header.record_hash();
        if chained {
            state.own = Some((header.seq, hash));
        }
        Ok(hash)
    }

    /// Write once: into the local store, then into the transport as `<uuid>.rumble`.
    fn publish(&self, uuid: Uuid, write: impl FnOnce(&mut File) -> Result<(), RecordError>) -> Result<(), RecordError> {
        let temp = self.temp_path();
        let written = File::create_new(&temp).map_err(RecordError::from).and_then(|mut out| {
            write(&mut out)?;
            out.sync_all()?;
            Ok(hash_file(&temp)?.1)
        });
        let ciphertext = written.inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })?;
        let stored = self.store.join(store_name(uuid, &ciphertext));
        fs::rename(&temp, &stored)?;
        Ok(self.place(&stored, &format!("{uuid}{RECORD_SUFFIX}"))?)
    }

    /// Copy a stored file into the transport under `name` unless that name exists: `.tmp`, then rename.
    fn place(&self, from: &Path, name: &str) -> io::Result<()> {
        let dest = self.transport.join(name);
        if dest.exists() {
            return Ok(());
        }
        let temp = self.transport.join(format!(".{}.tmp", Uuid::new_v4()));
        fs::copy(from, &temp)?;
        File::open(&temp)?.sync_all()?;
        fs::rename(&temp, &dest)
    }

    fn temp_path(&self) -> PathBuf {
        self.temp.join(format!("{}.tmp", Uuid::new_v4()))
    }

    fn identities<'e>(&'e self, epochs: &'e [Epoch]) -> Vec<&'e dyn age::Identity> {
        let mut ids: Vec<&dyn age::Identity> = vec![self.keys.identity()];
        ids.extend(epochs.iter().map(|e| &e.identity as &dyn age::Identity));
        ids
    }

    /// Open one stored or incoming file with this device's key and the epochs held so far.
    fn read(&self, path: &Path, uuid: Uuid, ciphertext: Hash, epochs: &[Epoch]) -> Result<Opening, RecordError> {
        if ciphertext == self.id {
            let bytes = fs::read(path)?;
            let admin = genesis_admin(&bytes).ok_or(RecordError::Malformed("genesis"))?;
            return Ok(Opening::Genesis(Genesis { uuid, path: path.into(), admin }));
        }
        let mut file = File::open(path)?;
        let (header, rest, _) = sync_crypto::open(&mut file, uuid, &self.id, &self.identities(epochs))?;
        let epoch = match &header.body {
            Body::Key(fields) => Some(sync_crypto::epoch_from(fields, rest)?),
            _ => None,
        };
        Ok(Opening::Record(Record { uuid, ciphertext, path: path.into(), header, signed_epoch_opens: vec![], epoch }))
    }

    /// A half-synced file can open and verify its first chunk; only the whole payload proves a version.
    fn check_payload(&self, r: &Record, epochs: &[Epoch]) -> Result<(), RecordError> {
        let Body::Version(fields) = &r.header.body else { return Ok(()) };
        let mut file = File::open(&r.path)?;
        let (_, rest, _) = sync_crypto::open(&mut file, r.uuid, &self.id, &self.identities(epochs))?;
        let mut hasher = Sha256::new();
        let size = io::copy(&mut rest.take(fields.size.saturating_add(1)), &mut hasher)?;
        match size == fields.size && <[u8; 32]>::from(hasher.finalize()) == fields.content_sha256 {
            true => Ok(()),
            false => Err(RecordError::ContentMismatch),
        }
    }

    /// Every stored copy that opens with what this device holds; repeated as `key` records yield epochs.
    fn open_store(&self) -> Result<Opened, RecordError> {
        let mut opened = Opened::default();
        let mut pending = Vec::new();
        for entry in fs::read_dir(&self.store)? {
            let path = entry?.path();
            if let Some((uuid, hash)) = path.file_name().and_then(|n| n.to_str()).and_then(parse_store_name) {
                opened.seen.insert((uuid, hash));
                pending.push((uuid, hash, path));
            }
        }
        loop {
            let before = pending.len();
            let mut still = Vec::new();
            for (uuid, hash, path) in pending {
                match self.read(&path, uuid, hash, &opened.epochs) {
                    Ok(item) => opened.add(item),
                    Err(_) => still.push((uuid, hash, path)),
                }
            }
            pending = still;
            if pending.len() == before {
                break;
            }
        }
        // Bind each version to its signed epoch, not to whichever held identity opened it first; only
        // now, with every epoch in hand, so the answer can't depend on the order the store was read in.
        for r in &mut opened.records {
            if let Body::Version(fields) = &r.header.body {
                for e in opened.epochs.iter().filter(|e| e.number == fields.epoch) {
                    if opens_with(&r.path, &e.identity)? {
                        r.signed_epoch_opens.push(e.recipient().to_string());
                    }
                }
            }
        }
        Ok(opened)
    }

    /// Copy in every transport file not seen before that verifies. Returns how many were imported,
    /// the ciphertext hashes present in the transport, and what did not verify.
    fn import(&self, opened: &Opened) -> Result<(usize, HashSet<Hash>, Vec<(String, RecordError)>), RecordError> {
        let (mut imported, mut present, mut unverified) = (0, HashSet::new(), Vec::new());
        for entry in fs::read_dir(&self.transport)? {
            let entry = entry?;
            let Some((name, uuid)) = entry.file_name().to_str().and_then(|n| Some((n.to_owned(), claimed_uuid(n)?)))
            else {
                continue;
            };
            match self.import_one(&entry.path(), uuid, opened) {
                Ok((hash, new)) => {
                    present.insert(hash);
                    imported += usize::from(new);
                }
                Err(e) => unverified.push((name, e)),
            }
        }
        Ok((imported, present, unverified))
    }

    fn import_one(&self, path: &Path, uuid: Uuid, opened: &Opened) -> Result<(Hash, bool), RecordError> {
        let (_, hash) = hash_file(path)?;
        if opened.seen.contains(&(uuid, hash)) {
            return Ok((hash, false));
        }
        let temp = self.temp_path();
        let verified = fs::copy(path, &temp).map_err(RecordError::from).and_then(|_| {
            let (_, copied) = hash_file(&temp)?;
            if let Opening::Record(r) = self.read(&temp, uuid, copied, &opened.epochs)? {
                self.check_payload(&r, &opened.epochs)?;
            }
            Ok(copied)
        });
        match verified {
            Ok(copied) => {
                fs::rename(&temp, self.store.join(store_name(uuid, &copied)))?;
                Ok((copied, true))
            }
            Err(e) => {
                let _ = fs::remove_file(&temp);
                Err(e)
            }
        }
    }

    /// Restore every accepted record missing from the transport under its write-once restore name.
    fn write_back(&self, state: &State, present: &HashSet<Hash>) -> io::Result<()> {
        let genesis = state.restorable_genesis.iter().map(|(uuid, path)| (uuid, &self.id, path));
        let records = state.accepted.values().map(|(uuid, hash, path)| (uuid, hash, path));
        for (uuid, hash, path) in genesis.chain(records) {
            let name = format!("{uuid}-{}{RECORD_SUFFIX}", &hex::encode(hash)[..RESTORE_HASH_HEX]);
            // Anyone can read hash8 off the original, so a file already squatting the name is replaced.
            if !present.contains(hash) {
                let _ = fs::remove_file(self.transport.join(&name));
                self.place(path, &name)?;
            }
        }
        Ok(())
    }
}

enum Opening {
    Genesis(Genesis),
    Record(Record),
}

impl Opened {
    fn add(&mut self, item: Opening) {
        match item {
            Opening::Genesis(g) => self.genesis = Some(g),
            Opening::Record(r) => {
                if let Some(e) = &r.epoch {
                    self.epochs.push(e.clone());
                }
                self.records.push(r);
            }
        }
    }
}

/// Judge every verified record: the admin chain from genesis, removal cutoffs, `key` records, then
/// device and version records at their own `admin_head`. Order of arrival never matters.
fn fold(id: Hash, me: PublicKey, now_ms: i64, opened: Opened, unverified: Vec<(String, RecordError)>) -> State {
    let mut state = State { id, unverified, own_signer: Some(me), ..State::default() };
    state.own = opened.records.iter().filter(|r| r.header.signer == me && !matches!(r.header.body, Body::Key(_))).max_by_key(|r| r.header.seq).map(|r| (r.header.seq, r.header.record_hash()));
    let Some(genesis) = opened.genesis else { return state };
    state.genesis_admin = Some(genesis.admin);
    state.restorable_genesis = Some((genesis.uuid, genesis.path));
    let mut records: HashMap<Hash, Record> = HashMap::new();
    for r in opened.records {
        records.entry(r.header.record_hash()).or_insert(r);
    }
    // Each signer's records form one chain: seq 1 has no prev, and every prev sits exactly one seq below.
    let broken: Vec<Hash> = records
        .iter()
        .filter(|(_, r)| !matches!(r.header.body, Body::Key(_)))
        .filter(|(_, r)| match r.header.prev {
            None => r.header.seq != 1,
            Some(p) => records.get(&p).is_some_and(|q| q.header.signer != r.header.signer || q.header.seq + 1 != r.header.seq),
        })
        .map(|(h, _)| *h)
        .collect();
    for hash in broken {
        let r = records.remove(&hash).expect("collected from records");
        state.rejected.push((r.uuid, RecordError::Malformed("seq")));
    }
    // Two different records with one signer and seq mean the device is compromised: everything it
    // signed from that seq on is held until an admin decides. Admin files are held here; the rest
    // below, unless the device is removed, when its cutoff already decides.
    let mut slots: HashMap<(PublicKey, u64), Hash> = HashMap::new();
    let mut twinned: HashMap<PublicKey, u64> = HashMap::new();
    for (hash, r) in records.iter().filter(|(_, r)| !matches!(r.header.body, Body::Key(_))) {
        let (signer, seq) = (r.header.signer, r.header.seq);
        if slots.insert((signer, seq), *hash).is_some() {
            twinned.entry(signer).and_modify(|s| *s = (*s).min(seq)).or_insert(seq);
        }
    }
    let held_from = |twinned: &HashMap<PublicKey, u64>, h: &Header| {
        !matches!(h.body, Body::Key(_)) && twinned.get(&h.signer).is_some_and(|seq| h.seq >= *seq)
    };
    records.retain(|_, r| match matches!(r.header.body, Body::Member(_)) && held_from(&twinned, &r.header) {
        true => {
            state.held.push(r.uuid);
            false
        }
        false => true,
    });
    state.flags.extend(twinned.keys().map(|d| (*d, Flag::Equivocation)));
    state.verified = records.iter().map(|(hash, r)| (*hash, r.header.clone())).collect();
    fn accept(state: &mut State, hash: Hash, r: &Record) {
        state.accepted.insert(hash, (r.uuid, r.ciphertext, r.path.clone()));
        state.signed.insert(hash, r.header.clone());
    }

    // The admin chain: a `member` record counts once its parents do and its signer is an admin there.
    let mut pending: Vec<Hash> =
        records.iter().filter(|(_, r)| matches!(r.header.body, Body::Member(_))).map(|(h, _)| *h).collect();
    loop {
        let before = pending.len();
        pending.retain(|hash| {
            let r = &records[hash];
            let parents = &r.header.parents;
            if parents.is_empty() || !parents.iter().all(|p| state.knows_admin_file(p)) {
                return true;
            }
            let verdict = judge_admin_file(&state.view(parents), &r.header);
            match verdict {
                Ok(()) => {
                    state.admin.insert(*hash, r.header.clone());
                    accept(&mut state, *hash, r);
                }
                Err(e) => state.rejected.push((r.uuid, e)),
            }
            false
        });
        if pending.len() == before {
            break;
        }
    }
    state.held.extend(pending.iter().map(|h| records[h].uuid));

    // The merged chain: every branch's removals, void files set aside for every later judgment.
    let all: Vec<Hash> = state.admin.keys().copied().collect();
    let View { membership, invites, closed, named, void } = state.view(&all);
    for hash in &void {
        if let Body::Member(Member::Remove { .. }) = state.admin[hash].body {
            state.flags.insert((state.admin[hash].signer, Flag::MutualRemoval));
        }
    }
    (state.roll, state.invites, state.closed, state.named, state.void) = (membership, invites, closed, named, void);
    let mut answers: HashMap<Uuid, HashMap<PublicKey, (u64, DeviceFields)>> = HashMap::new();

    let mut by_kind: Vec<(&Hash, &Record)> = records.iter().filter(|(_, r)| !matches!(r.header.body, Body::Member(_))).collect();
    // `key` records first: versions need the accepted epochs.
    by_kind.sort_by_key(|(_, r)| !matches!(r.header.body, Body::Key(_)));
    for (hash, r) in by_kind {
        let h = &r.header;
        if held_from(&twinned, h) && !state.roll.removed.contains_key(&h.signer) {
            state.held.push(r.uuid);
            continue;
        }
        let verdict = match &h.body {
            Body::Key(_) => {
                if h.parents.is_empty() || !h.parents.iter().all(|p| state.knows_admin_file(p)) {
                    state.held.push(r.uuid);
                    continue;
                }
                let Body::Key(fields) = &h.body else { unreachable!("matched above") };
                let named = state.named.contains(&(fields.epoch, fields.epoch_recipient.clone()));
                match state.membership_at(&h.parents).admins.contains(&h.signer) && named {
                    true => {
                        let epoch = r.epoch.clone().expect("key records carry an epoch");
                        let recipient = epoch.recipient().to_string();
                        if !state.epochs.iter().any(|e| e.recipient().to_string() == recipient) {
                            state.epochs.push(epoch);
                        }
                        Ok(())
                    }
                    false => Err(RecordError::NotAdmin),
                }
            }
            _ => {
                let Some(head) = h.admin_head else {
                    state.rejected.push((r.uuid, RecordError::Malformed("admin_head")));
                    continue;
                };
                if !state.knows_admin_file(&head) {
                    // Only a member at this device's own head can block it; anyone with a link can write.
                    if state.is_member(&h.signer) {
                        state.write_block.get_or_insert(head);
                    }
                    state.held.push(r.uuid);
                    continue;
                }
                match &h.body {
                    // A request from a device not yet approved goes to the inbox, and only for an open invite.
                    Body::Request(RequestFields { device, link }) if !state.roll.members.contains(&h.signer) => {
                        if let Some(invite) = open_invite(&state, link, now_ms) {
                            insert_newest(answers.entry(invite).or_default(), h, device);
                        }
                        continue;
                    }
                    // An approved device's request is its first self-description, judged at this device's head.
                    Body::Request(RequestFields { device: fields, .. }) => {
                        let verdict = state.roll.admit(h);
                        if verdict.is_ok() {
                            insert_newest(&mut state.devices, h, fields);
                        }
                        verdict
                    }
                    // A device describes itself before it is approved, so it is judged at this device's head.
                    Body::Device(fields) => {
                        if !state.roll.members.contains(&h.signer) && !state.roll.removed.contains_key(&h.signer) {
                            continue;
                        }
                        let verdict = state.roll.admit(h);
                        if verdict.is_ok() {
                            insert_newest(&mut state.devices, h, fields);
                        }
                        verdict
                    }
                    Body::Version(fields) => state.membership_at(&[head]).admit(h).and_then(|()| {
                        let epoch_held = state.epochs.iter().any(|e| {
                            e.number == fields.epoch && r.signed_epoch_opens.contains(&e.recipient().to_string())
                        });
                        match epoch_held {
                            true => {
                                state.versions.insert(*hash, h.clone());
                                Ok(())
                            }
                            false => Err(RecordError::WrongEpoch),
                        }
                    }),
                    Body::Key(_) | Body::Member(_) => unreachable!("sorted out above"),
                }
            }
        };
        match verdict {
            Ok(()) => accept(&mut state, *hash, r),
            Err(e) => state.rejected.push((r.uuid, e)),
        }
    }
    // Two devices answering one invite means someone else holds the link: neither can be approved.
    for (invite, devices) in answers {
        match devices.len() {
            1 => state.requests.extend(devices.into_iter().map(|(d, (_, fields))| (d, (invite, fields)))),
            _ => {
                state.contested.insert(invite);
            }
        }
    }
    state.files = histories(&state.versions);
    state
}

/// Whether an admin file is valid where its parents stand: an admin signs it, and it acts on what exists.
fn judge_admin_file(at: &View, h: &Header) -> Result<(), RecordError> {
    let m = &at.membership;
    let open = |invite: &Uuid| at.invites.contains_key(invite) && !at.closed.contains(invite);
    if !m.admins.contains(&h.signer) {
        return Err(RecordError::NotAdmin);
    }
    match &h.body {
        // With concurrent counter-removals void, this keeps at least one admin: never zero admins.
        Body::Member(Member::Remove { device, .. }) if *device == h.signer => Err(RecordError::RemovesItself),
        Body::Member(Member::Approve { invite: Some(invite), .. }) if !open(invite) => Err(RecordError::InviteNotOpen),
        Body::Member(Member::Approve { device, invite: None, .. }) if !m.members.contains(device) || m.removed.contains_key(device) => {
            Err(RecordError::NotMember)
        }
        Body::Member(Member::Revoke { invite }) if !open(invite) => Err(RecordError::InviteNotOpen),
        Body::Member(Member::Invite { link }) => {
            let parsed = Link::verify(link)?;
            if parsed.admin != h.signer || parsed.briefcase_id != h.briefcase_id || at.invites.contains_key(&parsed.invite) {
                return Err(RecordError::Malformed("invite"));
            }
            // A second admin before the second invite: one lost laptop must not strand the matter.
            match !at.invites.is_empty() && m.admins.len() < 2 {
                true => Err(RecordError::NeedsSecondAdmin),
                false => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

/// The number the next epoch takes where `at` stands: one past every epoch named there.
fn next_epoch(at: &View) -> u64 {
    at.named.iter().map(|(n, _)| n + 1).max().unwrap_or(FIRST_EPOCH)
}

/// The invite a request answers: only if its link is exactly one an admin of this matter issued in the
/// admin chain, still open, and younger than `INVITE_TTL_DAYS` by this device's clock.
fn open_invite(state: &State, link: &[u8], now_ms: i64) -> Option<Uuid> {
    let parsed = Link::verify(link).ok()?;
    let issued = state.invites.get(&parsed.invite).filter(|issued| issued.as_slice() == link)?;
    let fresh = now_ms.saturating_sub(Link::verify(issued).ok()?.issued_at_ms) <= INVITE_TTL_DAYS * DAY_MS;
    (!state.closed.contains(&parsed.invite) && fresh).then_some(parsed.invite)
}

/// A device may describe itself more than once; its highest-seq record wins, whatever the read order.
fn insert_newest(map: &mut HashMap<PublicKey, (u64, DeviceFields)>, h: &Header, fields: &DeviceFields) {
    if map.get(&h.signer).is_none_or(|(seq, _)| *seq < h.seq) {
        map.insert(h.signer, (h.seq, fields.clone()));
    }
}

#[derive(Clone)]
struct Removal {
    signer: PublicKey,
    device: PublicKey,
    /// The device's records that stay accepted: its cutoff's `prev` chain.
    kept: HashSet<Hash>,
    /// The admin files the remover had seen: a removed admin's admin files stand only if among them.
    seen: HashSet<Hash>,
}

/// Whether a removal other than admin file `hash` removes its `signer` without having seen `hash`.
fn removed_past(removals: &HashMap<Hash, Removal>, hash: &Hash, signer: &PublicKey) -> bool {
    removals.iter().any(|(r, removal)| r != hash && removal.device == *signer && !removal.seen.contains(hash))
}

/// The records of a removed device that stay accepted: its cutoff and what the cutoff's `prev` chain
/// reaches (each link one seq below the last, checked at import). A record the device signs later can't
/// join that chain, whatever `seq` it claims, because the cutoff hash already fixes every link. The chain
/// ends at the first link this device doesn't hold, which is not kept: what nobody saw before stays out.
fn kept_chain(records: &HashMap<Hash, Header>, device: PublicKey, cutoff: Option<Hash>) -> HashSet<Hash> {
    let mut kept = HashSet::new();
    let mut next = cutoff;
    while let Some(h) = next.and_then(|hash| records.get(&hash)).filter(|h| h.signer == device) {
        kept.insert(next.expect("walked from a hash"));
        next = h.prev;
    }
    kept
}

fn histories(versions: &HashMap<Hash, Header>) -> BTreeMap<Uuid, FileHistory> {
    let fields = |h: &Header| match &h.body {
        Body::Version(v) => v.clone(),
        _ => unreachable!("only versions"),
    };
    // Same file, same parents, same content: one version, named by its smallest record hash.
    let mut class: HashMap<(Uuid, BTreeSet<Hash>, Hash), Hash> = HashMap::new();
    for (hash, h) in versions {
        let v = fields(h);
        let key = (v.file_id, h.parents.iter().copied().collect(), v.content_sha256);
        class.entry(key).and_modify(|c| *c = (*c).min(*hash)).or_insert(*hash);
    }
    let canon: HashMap<Hash, Hash> = versions
        .iter()
        .map(|(hash, h)| {
            let v = fields(h);
            (*hash, class[&(v.file_id, h.parents.iter().copied().collect(), v.content_sha256)])
        })
        .collect();
    let mut files: BTreeMap<Uuid, FileHistory> = BTreeMap::new();
    let mut children: HashMap<Hash, BTreeSet<Hash>> = HashMap::new();
    for (hash, h) in versions {
        let rep = canon[hash];
        let file = files.entry(fields(h).file_id).or_default();
        file.versions.insert(rep);
        for p in &h.parents {
            children.entry(*canon.get(p).unwrap_or(p)).or_default().insert(rep);
        }
    }
    for file in files.values_mut() {
        file.heads = file.versions.iter().filter(|v| !children.contains_key(*v)).copied().collect();
        file.forks = file.versions.iter().filter(|v| children.get(*v).is_some_and(|c| c.len() > 1)).copied().collect();
    }
    files
}

fn opens_with(path: &Path, identity: &dyn age::Identity) -> Result<bool, RecordError> {
    let decryptor = age::Decryptor::new(File::open(path)?).map_err(RecordError::Decrypt)?;
    Ok(decryptor.decrypt(std::iter::once(identity)).is_ok())
}

fn genesis_admin(bytes: &[u8]) -> Option<PublicKey> {
    let rest = bytes.strip_prefix(GENESIS_MAGIC)?;
    match rest.len() == NONCE_BYTES + 32 {
        true => rest[NONCE_BYTES..].try_into().ok(),
        false => None,
    }
}

/// The uuid a transport name claims: `<uuid>.rumble`, or a restore name `<uuid>-<hash8>.rumble`.
/// Everything else (conflict copies, `.tmp`, `desktop.ini`) is not ours.
fn claimed_uuid(name: &str) -> Option<Uuid> {
    let stem = name.strip_suffix(RECORD_SUFFIX).filter(|s| s.is_ascii())?;
    let (id, restore) = stem.split_at(stem.len().min(UUID_CHARS));
    let restore_ok = restore.is_empty()
        || restore.strip_prefix('-').is_some_and(|h| h.len() == RESTORE_HASH_HEX && is_lower_hex(h));
    Uuid::parse_str(id).ok().filter(|u| restore_ok && u.hyphenated().to_string() == id)
}

fn store_name(uuid: Uuid, ciphertext: &Hash) -> String {
    format!("{uuid}-{}{RECORD_SUFFIX}", hex::encode(ciphertext))
}

fn parse_store_name(name: &str) -> Option<(Uuid, Hash)> {
    let stem = name.strip_suffix(RECORD_SUFFIX).filter(|s| s.is_ascii() && s.len() > UUID_CHARS)?;
    let (id, hash) = stem.split_at(UUID_CHARS);
    let hash: Hash = hex::decode(hash.strip_prefix('-')?).ok()?.try_into().ok()?;
    Some((Uuid::parse_str(id).ok()?, hash))
}

fn is_lower_hex(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn hash_file(path: &Path) -> io::Result<(u64, Hash)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; READ_CHUNK_BYTES];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok((size, hasher.finalize().into()));
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: Uuid = Uuid::from_bytes([0x0f; 16]);
    const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

    /// One laptop: device keys (standing in for the hardware-wrapped blob) and its own data dir.
    struct Laptop {
        keys: DeviceKeys,
        data: tempfile::TempDir,
    }

    impl Laptop {
        fn new() -> Self {
            Laptop { keys: DeviceKeys::generate(), data: tempfile::tempdir().unwrap() }
        }
        /// Start Rumble on this laptop: nothing carries over but the data dir and the keys.
        fn on(&self, id: Hash, transport: &Path) -> Briefcase<'_> {
            Briefcase::open(id, &self.keys, transport, self.data.path()).unwrap()
        }
        /// A document saved by Word into this laptop's working copy (never the transport).
        fn doc(&self, content: &[u8]) -> PathBuf {
            let path = self.data.path().join(format!("{}.docx", Uuid::new_v4()));
            fs::write(&path, content).unwrap();
            path
        }
        fn save(&self, id: Hash, t: &Path, content: &[u8], parents: Vec<Hash>) -> Hash {
            self.on(id, t).save_version(FILE, "nda.docx", DOCX, &self.doc(content), parents).unwrap()
        }
    }

    /// An admin invites; the link goes to the joiner out of band; the joiner requests. Returns the
    /// joiner's safety number.
    fn ask(admin: &Laptop, joiner: &Laptop, id: Hash, t: &Path, name: &str) -> String {
        let link = admin.on(id, t).invite(name).unwrap();
        joiner.on(id, t).request(&link, name).unwrap();
        joiner.on(id, t).safety_number().unwrap()
    }

    /// Ask, then both sides tap "Numbers match".
    fn join_as(admin: &Laptop, joiner: &Laptop, id: Hash, t: &Path, name: &str, as_admin: bool) {
        let number = ask(admin, joiner, id, t, name);
        admin.on(id, t).approve(joiner.keys.signer(), as_admin, &number).unwrap();
        joiner.on(id, t).confirm(&number).unwrap();
    }

    fn join(admin: &Laptop, joiner: &Laptop, id: Hash, t: &Path, name: &str) {
        join_as(admin, joiner, id, t, name, false);
    }

    /// Ana creates a briefcase; every other laptop joins it. With a third, the second is made an admin
    /// first, as the second-admin rule asks.
    fn team(n: usize) -> (tempfile::TempDir, Hash, Vec<Laptop>) {
        let t = tempfile::tempdir().unwrap();
        let laptops: Vec<Laptop> = (0..n).map(|_| Laptop::new()).collect();
        let id = Briefcase::create(&laptops[0].keys, t.path(), laptops[0].data.path(), "Ana Lim").unwrap().id;
        for (i, joiner) in laptops.iter().enumerate().skip(1) {
            join(&laptops[0], joiner, id, t.path(), &format!("Member {i}"));
            if i == 1 && n > 2 {
                laptops[0].on(id, t.path()).promote(joiner.keys.signer()).unwrap();
            }
        }
        (t, id, laptops)
    }

    fn transport_names(t: &Path) -> BTreeSet<String> {
        fs::read_dir(t).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect()
    }

    /// Move files written since `before` out of the transport, as a sync client that hasn't uploaded yet.
    fn withhold(t: &Path, before: &BTreeSet<String>) -> tempfile::TempDir {
        let aside = tempfile::tempdir().unwrap();
        for name in transport_names(t).difference(before) {
            fs::rename(t.join(name), aside.path().join(name)).unwrap();
        }
        aside
    }

    fn deliver(aside: tempfile::TempDir, t: &Path) {
        for e in fs::read_dir(aside.path()).unwrap() {
            let e = e.unwrap();
            fs::rename(e.path(), t.join(e.file_name())).unwrap();
        }
    }

    fn file_of(t: &Path, uuid: Uuid) -> PathBuf {
        t.join(format!("{uuid}{RECORD_SUFFIX}"))
    }

    /// A record built by hand, as an attacker or a buggy client would, dropped into the transport.
    fn forge(t: &Path, keys: &DeviceKeys, id: Hash, seq: u64, prev: Option<Hash>, admin_head: Hash, epoch: &Epoch) -> Header {
        let content = b"forged";
        let header = Header {
            briefcase_id: id,
            uuid: Uuid::new_v4(),
            signer: keys.signer(),
            seq,
            prev,
            parents: vec![],
            admin_head: Some(admin_head),
            body: Body::Version(VersionFields {
                file_id: FILE,
                version_id: Uuid::new_v4(),
                epoch: epoch.number,
                file_name: "nda.docx".into(),
                media_type: DOCX.into(),
                size: content.len() as u64,
                created_at_ms: 0,
                content_sha256: Sha256::digest(content).into(),
            }),
        };
        let mut out = File::create(file_of(t, header.uuid)).unwrap();
        sync_crypto::seal(keys, &header, &content[..], &[&epoch.recipient()], &mut out).unwrap();
        header
    }

    fn rejected_for(state: &State, uuid: Uuid) -> Option<&RecordError> {
        state.rejected.iter().find(|(u, _)| *u == uuid).map(|(_, e)| e)
    }

    fn uuid_of(state: &State, version: &Hash) -> Uuid {
        state.accepted[version].0
    }

    // ---- the plan's MVP script: two laptops, one folder, never on together ----

    #[test]
    fn two_laptops_never_on_together_converge_on_one_history() {
        let t = tempfile::tempdir().unwrap();
        let t = t.path();
        let (ana, ben) = (Laptop::new(), Laptop::new());

        // Ana creates the matter, adds nda.docx as v1 and stops.
        let id = Briefcase::create(&ana.keys, t, ana.data.path(), "Ana Lim").unwrap().id;
        let v1 = ana.save(id, t, b"NDA v1 by Ana", vec![]);
        let link = ana.on(id, t).invite("Ben Tan").unwrap();

        // Ben opens the link, requests, sees the number and stops; he can open nothing yet.
        ben.on(id, t).request(&link, "Ben Tan").unwrap();
        let bens_number = ben.on(id, t).safety_number().unwrap();
        assert!(ben.on(id, t).sync().unwrap().versions.is_empty());

        // Ana starts, sees the request with the same number, taps "Numbers match" and stops.
        let state = ana.on(id, t).sync().unwrap();
        assert_eq!(state.requests[&ben.keys.signer()].1.display_name, "Ben Tan");
        assert_eq!(state.safety_number(&ben.keys.signer()).unwrap(), bens_number);
        ana.on(id, t).approve(ben.keys.signer(), false, &bens_number).unwrap();

        // Ben starts and taps "Numbers match" too.
        ben.on(id, t).confirm(&bens_number).unwrap();

        // Ben starts, is in, and saves v2 on top of v1.
        assert!(ben.on(id, t).sync().unwrap().is_member(&ben.keys.signer()));
        let v2 = ben.save(id, t, b"NDA v2 by Ben", vec![v1]);

        // Ana starts and sees v2 by Ben.
        let state = ana.on(id, t).sync().unwrap();
        assert_eq!(state.files[&FILE].heads, BTreeSet::from([v2]));
        assert_eq!(state.devices[&state.versions[&v2].signer].1.display_name, "Ben Tan");

        // Both edit v2 offline: neither sees the other's save until both have saved.
        let before = transport_names(t);
        let v3a = ana.save(id, t, b"NDA v3 by Ana", vec![v2]);
        let aside = withhold(t, &before);
        let v3b = ben.save(id, t, b"NDA v3 by Ben", vec![v2]);
        deliver(aside, t);
        for laptop in [&ana, &ben] {
            let file = &laptop.on(id, t).sync().unwrap().files[&FILE];
            assert_eq!((&file.heads, &file.forks), (&BTreeSet::from([v3a, v3b]), &BTreeSet::from([v2])));
        }

        // "Use Ana's" on both laptops, offline again: one v4, not a new fork.
        let before = transport_names(t);
        ana.save(id, t, b"NDA v3 by Ana", vec![v3a, v3b]);
        let aside = withhold(t, &before);
        ben.save(id, t, b"NDA v3 by Ana", vec![v3a, v3b]);
        deliver(aside, t);
        let (a, b) = (ana.on(id, t).sync().unwrap(), ben.on(id, t).sync().unwrap());
        assert_eq!(a.files, b.files);
        assert_eq!(a.files[&FILE].heads.len(), 1);

        // The head releases to the same bytes on both laptops.
        let head = *a.files[&FILE].heads.first().unwrap();
        for (laptop, state) in [(&ana, &a), (&ben, &b)] {
            let dest = laptop.data.path().join("nda.docx");
            laptop.on(id, t).release(state, &head, &dest).unwrap();
            assert_eq!(fs::read(dest).unwrap(), b"NDA v3 by Ana");
        }

        // The folder never holds plaintext: no content, no file name, no person's name.
        let secrets: [&[u8]; 7] =
            [b"NDA v1", b"NDA v2", b"NDA v3", b"nda.docx", b"Ana Lim", b"Ben Tan", b"wordprocessingml"];
        for name in transport_names(t) {
            let bytes = fs::read(t.join(&name)).unwrap();
            assert!(claimed_uuid(&name).is_some(), "{name} is not a record name");
            for s in secrets {
                assert!(!bytes.windows(s.len()).any(|w| w == s), "{name} holds {}", String::from_utf8_lossy(s));
            }
        }
    }

    #[test]
    fn syncing_again_imports_and_writes_nothing() {
        let (t, id, laptops) = team(2);
        laptops[1].save(id, t.path(), b"v1", vec![]);
        let store = |l: &Laptop| fs::read_dir(l.on(id, t.path()).store).unwrap().count();
        laptops[0].on(id, t.path()).sync().unwrap();
        let (names, stored) = (transport_names(t.path()), store(&laptops[0]));
        laptops[0].on(id, t.path()).sync().unwrap();
        assert_eq!((transport_names(t.path()), store(&laptops[0])), (names, stored));
    }

    #[test]
    fn two_versions_sharing_a_parent_are_a_fork() {
        let (t, id, laptops) = team(2);
        let v1 = laptops[0].save(id, t.path(), b"v1", vec![]);
        laptops[0].save(id, t.path(), b"v2 by Ana", vec![v1]);
        laptops[1].save(id, t.path(), b"v2 by Ben", vec![v1]);
        assert_eq!(laptops[0].on(id, t.path()).sync().unwrap().files[&FILE].forks, BTreeSet::from([v1]));
    }

    // ---- negative tests ----

    #[test]
    fn tampered_record_is_not_imported() {
        let (t, id, laptops) = team(2);
        let v1 = laptops[0].save(id, t.path(), b"v1", vec![]);
        let path = file_of(t.path(), uuid_of(&laptops[0].on(id, t.path()).sync().unwrap(), &v1));
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&path, bytes).unwrap();
        let state = laptops[1].on(id, t.path()).sync().unwrap();
        assert!(!state.versions.contains_key(&v1) && state.unverified.iter().any(|(_, e)| matches!(e, RecordError::Decrypt(_))));
    }

    #[test]
    fn deleted_version_is_restored_from_the_authors_copy() {
        let (t, id, laptops) = team(2);
        let v1 = laptops[0].save(id, t.path(), b"v1", vec![]);
        fs::remove_file(file_of(t.path(), uuid_of(&laptops[0].on(id, t.path()).sync().unwrap(), &v1))).unwrap();
        laptops[0].on(id, t.path()).sync().unwrap();
        assert!(laptops[1].on(id, t.path()).sync().unwrap().versions.contains_key(&v1));
    }

    #[test]
    fn record_replayed_under_another_name_is_rejected() {
        let (t, id, laptops) = team(2);
        let v1 = laptops[0].save(id, t.path(), b"v1", vec![]);
        let uuid = uuid_of(&laptops[0].on(id, t.path()).sync().unwrap(), &v1);
        fs::copy(file_of(t.path(), uuid), file_of(t.path(), Uuid::new_v4())).unwrap();
        let state = laptops[1].on(id, t.path()).sync().unwrap();
        assert!(state.unverified.iter().any(|(_, e)| matches!(e, RecordError::UuidMismatch)));
    }

    #[test]
    fn record_from_another_matter_is_rejected() {
        let (t, id, laptops) = team(2);
        let other_t = tempfile::tempdir().unwrap();
        let other = Briefcase::create(&laptops[0].keys, other_t.path(), laptops[0].data.path(), "Ana Lim").unwrap().id;
        let foreign = laptops[0].save(other, other_t.path(), b"other matter", vec![]);
        let other_state = laptops[0].on(other, other_t.path()).sync().unwrap();
        // Someone holding both epochs re-encrypts it for this matter, under its own uuid.
        let uuid = uuid_of(&other_state, &foreign);
        let plain = age::decrypt(&other_state.current_epoch().unwrap().identity, &fs::read(file_of(other_t.path(), uuid)).unwrap()).unwrap();
        let here = laptops[0].on(id, t.path()).sync().unwrap().current_epoch().unwrap().recipient();
        fs::write(file_of(t.path(), uuid), age::encrypt(&here, &plain).unwrap()).unwrap();
        let state = laptops[1].on(id, t.path()).sync().unwrap();
        assert!(state.unverified.iter().any(|(_, e)| matches!(e, RecordError::ForeignMatter)));
    }

    #[test]
    fn version_from_an_unapproved_device_is_rejected() {
        let (t, id, laptops) = team(1);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        let outsider = DeviceKeys::generate();
        // Anyone with the link can encrypt to the epoch; only a member's signature counts.
        let forged = forge(t.path(), &outsider, id, 1, None, state.admin_head(&outsider.signer()), state.current_epoch().unwrap());
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::NotMember)));
    }

    #[test]
    fn version_naming_a_head_from_before_its_signers_approval_is_rejected() {
        let (t, id, laptops) = team(2);
        let state = laptops[1].on(id, t.path()).sync().unwrap();
        let (seq, prev) = state.own.unwrap();
        let forged = forge(t.path(), &laptops[1].keys, id, seq + 1, Some(prev), id, state.current_epoch().unwrap());
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::NotMember)));
    }

    #[test]
    fn stranger_naming_an_unseen_admin_file_does_not_block_writes() {
        let (t, id, laptops) = team(1);
        let epoch = laptops[0].on(id, t.path()).sync().unwrap().current_epoch().unwrap().clone();
        forge(t.path(), &DeviceKeys::generate(), id, 1, None, [0xEE; 32], &epoch);
        assert!(laptops[0].on(id, t.path()).sync().unwrap().write_block.is_none());
    }

    /// Ben saves twice, then Ana removes him; Ben then mints a record numbered below his cutoff.
    #[test]
    fn removed_device_cannot_backdate_a_record_below_its_cutoff() {
        let (t, id, laptops) = team(2);
        let (ana, ben) = (&laptops[0], &laptops[1]);
        let v1 = ben.save(id, t.path(), b"v1 by Ben", vec![]);
        ben.save(id, t.path(), b"v2 by Ben", vec![v1]);
        let before_removal = ben.on(id, t.path()).sync().unwrap();
        ana.on(id, t.path()).remove(ben.keys.signer()).unwrap();
        // Ben's request is his first record; the backdated one claims to follow it, at v1's seq.
        let first = before_removal.versions[&v1].prev.unwrap();
        let forged = forge(t.path(), &ben.keys, id, before_removal.versions[&v1].seq, Some(first), before_removal.admin_head(&ben.keys.signer()), before_removal.current_epoch().unwrap());
        let state = ana.on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::AfterRemovalCutoff)));
    }

    /// Ana removes Dan and saves after it; Dan deletes the removal before Ben has seen it.
    fn removal_deleted_before_ben_sees_it() -> (tempfile::TempDir, Hash, Vec<Laptop>, Hash) {
        let (t, id, laptops) = team(3);
        let (ana, dan) = (&laptops[0], &laptops[2]);
        laptops[1].on(id, t.path()).sync().unwrap();
        let removal = ana.on(id, t.path()).remove(dan.keys.signer()).unwrap();
        ana.save(id, t.path(), b"after the removal", vec![]);
        let uuid = ana.on(id, t.path()).sync().unwrap().accepted[&removal].0;
        fs::remove_file(file_of(t.path(), uuid)).unwrap();
        (t, id, laptops, removal)
    }

    #[test]
    fn deleted_removal_file_blocks_new_writes_on_a_device_that_lacks_it() {
        let (t, id, laptops, _) = removal_deleted_before_ben_sees_it();
        let ben = &laptops[1];
        let saved = ben.on(id, t.path()).save_version(FILE, "nda.docx", DOCX, &ben.doc(b"x"), vec![]);
        assert!(matches!(saved, Err(RecordError::WaitingForMembershipUpdate)));
    }

    #[test]
    fn deleted_removal_file_is_restored_and_clears_the_block() {
        let (t, id, laptops, removal) = removal_deleted_before_ben_sees_it();
        laptops[0].on(id, t.path()).sync().unwrap();
        let state = laptops[1].on(id, t.path()).sync().unwrap();
        assert!(state.write_block.is_none() && state.admin.contains_key(&removal) && !state.is_member(&laptops[2].keys.signer()));
    }

    /// A hand-built record of any kind from `keys`, next in its own chain, sealed to `recipients`.
    fn forge_body(t: &Path, keys: &DeviceKeys, state: &State, parents: Vec<Hash>, admin_head: Option<Hash>, body: Body, payload: &[u8], recipients: &[&dyn age::Recipient]) -> Header {
        let header = Header {
            briefcase_id: state.id,
            uuid: Uuid::new_v4(),
            signer: keys.signer(),
            seq: state.own.map_or(1, |(s, _)| s + 1),
            prev: state.own.map(|(_, h)| h),
            parents,
            admin_head,
            body,
        };
        let mut out = File::create(file_of(t, header.uuid)).unwrap();
        sync_crypto::seal(keys, &header, payload, recipients, &mut out).unwrap();
        header
    }

    #[test]
    fn removed_device_records_before_its_cutoff_stay_accepted() {
        let (t, id, laptops) = team(2);
        let v1 = laptops[1].save(id, t.path(), b"v1 by Ben", vec![]);
        laptops[1].save(id, t.path(), b"v2 by Ben", vec![v1]);
        laptops[0].on(id, t.path()).remove(laptops[1].keys.signer()).unwrap();
        assert!(laptops[0].on(id, t.path()).sync().unwrap().versions.contains_key(&v1));
    }

    #[test]
    fn approval_signed_by_a_non_admin_member_is_rejected() {
        let (t, id, laptops) = team(2);
        let ben = &laptops[1];
        let state = ben.on(id, t.path()).sync().unwrap();
        let epoch = state.current_epoch().unwrap().recipient();
        let body = Body::Member(Member::Approve { device: DeviceKeys::generate().signer(), admin: true, invite: None });
        let forged = forge_body(t.path(), &ben.keys, &state, state.tips(), None, body, b"", &[&epoch]);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::NotAdmin)));
    }

    /// Ben, a member but no admin, issues his own epoch and seals a version to it.
    fn epoch_issued_by_a_non_admin() -> (tempfile::TempDir, Hash, Vec<Laptop>, Epoch, Header) {
        let (t, id, laptops) = team(2);
        let (ana, ben) = (&laptops[0], &laptops[1]);
        let state = ben.on(id, t.path()).sync().unwrap();
        let rogue = Epoch::generate(2);
        let body = Body::Key(KeyFields { epoch: 2, epoch_recipient: rogue.recipient().to_string() });
        let line = format!("{}\n", age::secrecy::ExposeSecret::expose_secret(&rogue.identity.to_string()));
        forge_body(t.path(), &ben.keys, &state, state.tips(), None, body, line.as_bytes(), &[&ana.keys.recipient(), &ben.keys.recipient()]);
        let state = ben.on(id, t.path()).sync().unwrap();
        let content = b"sealed to a rogue epoch";
        let body = Body::Version(VersionFields {
            file_id: FILE,
            version_id: Uuid::new_v4(),
            epoch: 2,
            file_name: "nda.docx".into(),
            media_type: DOCX.into(),
            size: content.len() as u64,
            created_at_ms: 0,
            content_sha256: Sha256::digest(content).into(),
        });
        let version = forge_body(t.path(), &ben.keys, &state, vec![], Some(state.admin_head(&ben.keys.signer())), body, content, &[&rogue.recipient()]);
        (t, id, laptops, rogue, version)
    }

    #[test]
    fn key_file_from_a_non_admin_member_is_not_an_epoch() {
        let (t, id, laptops, rogue, _) = epoch_issued_by_a_non_admin();
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(state.epochs.iter().all(|e| e.recipient().to_string() != rogue.recipient().to_string()));
    }

    #[test]
    fn version_sealed_to_an_epoch_no_admin_issued_is_rejected() {
        let (t, id, laptops, _, version) = epoch_issued_by_a_non_admin();
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, version.uuid), Some(RecordError::WrongEpoch)));
    }

    #[test]
    fn admin_removed_by_another_admin_is_no_longer_a_member() {
        let (t, id, laptops) = team(3);
        laptops[0].on(id, t.path()).remove(laptops[1].keys.signer()).unwrap();
        let state = laptops[2].on(id, t.path()).sync().unwrap();
        assert!(!state.is_member(&laptops[1].keys.signer()) && !state.roll.admins.contains(&laptops[1].keys.signer()));
    }

    /// Never zero admins: an admin is removed only by another admin.
    #[test]
    fn admin_cannot_remove_itself() {
        let (t, id, laptops) = team(2);
        let ana = &laptops[0];
        ana.on(id, t.path()).remove(ana.keys.signer()).unwrap();
        let state = ana.on(id, t.path()).sync().unwrap();
        assert!(state.roll.admins.contains(&ana.keys.signer()) && state.rejected.iter().any(|(_, e)| matches!(e, RecordError::RemovesItself)));
    }

    /// Ana and Ben, both admins, remove each other while offline; Dan sees both removals.
    fn mutual_removal() -> (tempfile::TempDir, Hash, Vec<Laptop>, State) {
        let (t, id, laptops) = team(3);
        let (ana, ben) = (&laptops[0], &laptops[1]);
        let before = transport_names(t.path());
        ana.on(id, t.path()).remove(ben.keys.signer()).unwrap();
        let aside = withhold(t.path(), &before);
        ben.on(id, t.path()).remove(ana.keys.signer()).unwrap();
        deliver(aside, t.path());
        let state = laptops[2].on(id, t.path()).sync().unwrap();
        (t, id, laptops, state)
    }

    #[test]
    fn concurrent_mutual_admin_removals_are_both_void() {
        let (_t, _, laptops, state) = mutual_removal();
        assert!(laptops[..2].iter().all(|l| state.is_member(&l.keys.signer()) && state.roll.admins.contains(&l.keys.signer())));
    }

    #[test]
    fn concurrent_mutual_admin_removals_flag_both_admins() {
        let (_t, _, laptops, state) = mutual_removal();
        assert!(laptops[..2].iter().all(|l| state.flags.contains(&(l.keys.signer(), Flag::MutualRemoval))));
    }

    /// Ana removes Cam while Ben, offline, removes Dan; then Ana writes the next admin file.
    #[test]
    fn merging_admin_branches_moves_to_an_epoch_no_device_removed_in_either_holds() {
        let (t, id, laptops) = team(4);
        let (ana, ben, cam, dan) = (&laptops[0], &laptops[1], &laptops[2], &laptops[3]);
        let before = transport_names(t.path());
        ana.on(id, t.path()).remove(cam.keys.signer()).unwrap();
        let aside = withhold(t.path(), &before);
        ben.on(id, t.path()).remove(dan.keys.signer()).unwrap();
        deliver(aside, t.path());
        ana.on(id, t.path()).invite("Eve").unwrap();
        let current = ana.on(id, t.path()).sync().unwrap().current_epoch().unwrap().recipient().to_string();
        let held = |l: &Laptop| l.on(id, t.path()).sync().unwrap().epochs.iter().any(|e| e.recipient().to_string() == current);
        assert!(!held(cam) && !held(dan) && held(ben));
    }

    /// Ana removes Ben, an admin; Ben then signs admin files on the chain as it stood before.
    fn removed_admin_backdates() -> (tempfile::TempDir, Hash, Vec<Laptop>, State) {
        let (t, id, laptops) = team(3);
        let (ana, ben) = (&laptops[0], &laptops[1]);
        let before = ben.on(id, t.path()).sync().unwrap();
        ana.on(id, t.path()).remove(ben.keys.signer()).unwrap();
        (t, id, laptops, before)
    }

    #[test]
    fn removed_admin_cannot_issue_an_epoch_below_its_cutoff() {
        let (t, id, laptops, before) = removed_admin_backdates();
        let (ana, ben, dan) = (&laptops[0], &laptops[1], &laptops[2]);
        let rogue = Epoch::generate(2);
        let recipient = rogue.recipient().to_string();
        let body = Body::Member(Member::Rotate { epoch: 2, recipient: recipient.clone() });
        let rotation = forge_body(t.path(), &ben.keys, &before, before.tips(), None, body, b"", &[&rogue.recipient()]).record_hash();
        let body = Body::Key(KeyFields { epoch: 2, epoch_recipient: recipient.clone() });
        let line = format!("{}\n", age::secrecy::ExposeSecret::expose_secret(&rogue.identity.to_string()));
        let to = [ana.keys.recipient(), ben.keys.recipient(), dan.keys.recipient()];
        forge_body(t.path(), &ben.keys, &before, vec![rotation], None, body, line.as_bytes(), &[&to[0], &to[1], &to[2]]);
        let state = ana.on(id, t.path()).sync().unwrap();
        assert!(state.epochs.iter().all(|e| e.recipient().to_string() != recipient));
    }

    #[test]
    fn removed_admin_cannot_make_an_admin_below_its_cutoff() {
        let (t, id, laptops, before) = removed_admin_backdates();
        let dan = laptops[2].keys.signer();
        let epoch = before.current_epoch().unwrap().recipient();
        let body = Body::Member(Member::Approve { device: dan, admin: true, invite: None });
        forge_body(t.path(), &laptops[1].keys, &before, before.tips(), None, body, b"", &[&epoch]);
        assert!(!laptops[0].on(id, t.path()).sync().unwrap().roll.admins.contains(&dan));
    }

    // ---- joining: no keys in the link, a safety number both sides confirm ----

    /// A link Ana signs for this matter but never records as an invite, issued `days_ago`.
    fn unrecorded_link(ana: &Laptop, state: &State, days_ago: i64) -> (Link, Vec<u8>) {
        let link = Link {
            invite: Uuid::new_v4(),
            briefcase_id: state.id,
            epoch_recipient: state.current_epoch().unwrap().recipient().to_string(),
            admin: ana.keys.signer(),
            admin_name: "Ana Lim".into(),
            invitee_name: "Ben Tan".into(),
            issued_at_ms: chrono::Utc::now().timestamp_millis() - days_ago * DAY_MS,
        };
        let signed = link.sign(&ana.keys).unwrap();
        (link, signed)
    }

    /// Ana records `link` as an invite in the admin chain, as a client without the API's checks would.
    fn record_invite(t: &Path, ana: &Laptop, state: &State, link: Vec<u8>) -> Header {
        let epoch = state.current_epoch().unwrap().recipient();
        forge_body(t, &ana.keys, state, state.tips(), None, Body::Member(Member::Invite { link }), b"", &[&epoch])
    }

    #[test]
    fn stolen_link_opens_nothing() {
        let (t, id, laptops) = team(1);
        laptops[0].save(id, t.path(), b"v1", vec![]);
        let thief = Laptop::new();
        let link = laptops[0].on(id, t.path()).invite("Ben Tan").unwrap();
        thief.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        let state = thief.on(id, t.path()).sync().unwrap();
        assert!(state.epochs.is_empty() && state.versions.is_empty() && state.admin.is_empty());
    }

    #[test]
    fn approved_joiner_cannot_write_before_tapping_numbers_match() {
        let (t, id, laptops) = team(1);
        let ben = Laptop::new();
        let number = ask(&laptops[0], &ben, id, t.path(), "Ben Tan");
        laptops[0].on(id, t.path()).approve(ben.keys.signer(), false, &number).unwrap();
        let saved = ben.on(id, t.path()).save_version(FILE, "nda.docx", DOCX, &ben.doc(b"x"), vec![]);
        assert!(matches!(saved, Err(RecordError::NotConfirmed)));
    }

    #[test]
    fn joiner_cannot_confirm_a_number_that_differs() {
        let (t, id, laptops) = team(1);
        let ben = Laptop::new();
        ask(&laptops[0], &ben, id, t.path(), "Ben Tan");
        let other = sync_crypto::safety_number(b"another link", &ben.keys.signer());
        assert!(matches!(ben.on(id, t.path()).confirm(&other), Err(RecordError::SafetyNumberMismatch)));
    }

    #[test]
    fn admin_cannot_approve_with_a_number_that_differs() {
        let (t, id, laptops) = team(1);
        let ben = Laptop::new();
        ask(&laptops[0], &ben, id, t.path(), "Ben Tan");
        let other = sync_crypto::safety_number(b"another link", &ben.keys.signer());
        assert!(matches!(laptops[0].on(id, t.path()).approve(ben.keys.signer(), false, &other), Err(RecordError::SafetyNumberMismatch)));
    }

    /// Someone re-signs Ben's link as its admin, keeping the invite id and the epoch recipient.
    #[test]
    fn request_answering_a_swapped_link_is_ignored() {
        let (t, id, laptops) = team(1);
        let (ana, ben, mal) = (&laptops[0], Laptop::new(), Laptop::new());
        let genuine = Link::verify(&ana.on(id, t.path()).invite("Ben Tan").unwrap()).unwrap();
        let swapped = Link { admin: mal.keys.signer(), ..genuine }.sign(&mal.keys).unwrap();
        ben.on(id, t.path()).request(&swapped, "Ben Tan").unwrap();
        assert!(ana.on(id, t.path()).sync().unwrap().requests.is_empty());
    }

    #[test]
    fn two_requests_on_one_invite_leave_neither_approvable() {
        let (t, id, laptops) = team(1);
        let (ana, ben, mal) = (&laptops[0], Laptop::new(), Laptop::new());
        let link = ana.on(id, t.path()).invite("Ben Tan").unwrap();
        ben.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        mal.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        let state = ana.on(id, t.path()).sync().unwrap();
        assert!(state.requests.is_empty() && state.contested.contains(&Link::verify(&link).unwrap().invite));
    }

    #[test]
    fn request_for_an_approved_invite_is_ignored() {
        let (t, id, laptops) = team(1);
        let (ana, ben, mal) = (&laptops[0], Laptop::new(), Laptop::new());
        let link = ana.on(id, t.path()).invite("Ben Tan").unwrap();
        ben.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        let number = ben.on(id, t.path()).safety_number().unwrap();
        ana.on(id, t.path()).approve(ben.keys.signer(), false, &number).unwrap();
        mal.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        let state = ana.on(id, t.path()).sync().unwrap();
        assert!(state.requests.is_empty() && state.contested.is_empty());
    }

    #[test]
    fn request_for_a_revoked_invite_is_ignored() {
        let (t, id, laptops) = team(1);
        let (ana, ben) = (&laptops[0], Laptop::new());
        let link = ana.on(id, t.path()).invite("Ben Tan").unwrap();
        ana.on(id, t.path()).revoke(Link::verify(&link).unwrap().invite).unwrap();
        ben.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        assert!(ana.on(id, t.path()).sync().unwrap().requests.is_empty());
    }

    #[test]
    fn request_for_an_expired_invite_is_ignored() {
        let (t, id, laptops) = team(1);
        let (ana, ben) = (&laptops[0], Laptop::new());
        let state = ana.on(id, t.path()).sync().unwrap();
        let (_, link) = unrecorded_link(ana, &state, INVITE_TTL_DAYS + 1);
        record_invite(t.path(), ana, &state, link.clone());
        ben.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        assert!(ana.on(id, t.path()).sync().unwrap().requests.is_empty());
    }

    #[test]
    fn request_for_an_invite_never_recorded_is_ignored() {
        let (t, id, laptops) = team(1);
        let (ana, ben) = (&laptops[0], Laptop::new());
        let (_, link) = unrecorded_link(ana, &ana.on(id, t.path()).sync().unwrap(), 0);
        ben.on(id, t.path()).request(&link, "Ben Tan").unwrap();
        assert!(ana.on(id, t.path()).sync().unwrap().requests.is_empty());
    }

    #[test]
    fn second_invite_needs_a_second_admin() {
        let (t, id, laptops) = team(2);
        assert!(matches!(laptops[0].on(id, t.path()).invite("Cam"), Err(RecordError::NeedsSecondAdmin)));
    }

    #[test]
    fn second_invite_is_issued_once_a_member_is_made_an_admin() {
        let (t, id, laptops) = team(2);
        laptops[0].on(id, t.path()).promote(laptops[1].keys.signer()).unwrap();
        assert!(laptops[0].on(id, t.path()).invite("Cam").is_ok());
    }

    #[test]
    fn second_invite_recorded_with_one_admin_is_rejected() {
        let (t, id, laptops) = team(2);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        let (_, link) = unrecorded_link(&laptops[0], &state, 0);
        let forged = record_invite(t.path(), &laptops[0], &state, link);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::NeedsSecondAdmin)));
    }

    #[test]
    fn approval_of_a_device_that_answered_no_invite_is_rejected() {
        let (t, id, laptops) = team(1);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        let epoch = state.current_epoch().unwrap().recipient();
        let body = Body::Member(Member::Approve { device: DeviceKeys::generate().signer(), admin: false, invite: None });
        let forged = forge_body(t.path(), &laptops[0].keys, &state, state.tips(), None, body, b"", &[&epoch]);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::NotMember)));
    }

    #[test]
    fn approval_naming_a_revoked_invite_is_rejected() {
        let (t, id, laptops) = team(1);
        let ana = &laptops[0];
        let invite = Link::verify(&ana.on(id, t.path()).invite("Ben Tan").unwrap()).unwrap().invite;
        ana.on(id, t.path()).revoke(invite).unwrap();
        let state = ana.on(id, t.path()).sync().unwrap();
        let epoch = state.current_epoch().unwrap().recipient();
        let body = Body::Member(Member::Approve { device: DeviceKeys::generate().signer(), admin: false, invite: Some(invite) });
        let forged = forge_body(t.path(), &ana.keys, &state, state.tips(), None, body, b"", &[&epoch]);
        let state = ana.on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::InviteNotOpen)));
    }

    /// Ana approves Cam while Ben, offline, revokes Cam's invite.
    #[test]
    fn revoke_in_one_admin_branch_wins_over_an_approval_in_the_other() {
        let (t, id, laptops) = team(3);
        let (ana, ben, cam) = (&laptops[0], &laptops[1], Laptop::new());
        let number = ask(ana, &cam, id, t.path(), "Cam");
        let invite = ben.on(id, t.path()).sync().unwrap().requests[&cam.keys.signer()].0;
        let before = transport_names(t.path());
        ana.on(id, t.path()).approve(cam.keys.signer(), false, &number).unwrap();
        let aside = withhold(t.path(), &before);
        ben.on(id, t.path()).revoke(invite).unwrap();
        deliver(aside, t.path());
        assert!(!laptops[2].on(id, t.path()).sync().unwrap().is_member(&cam.keys.signer()));
    }

    // ---- removal rotates the epoch ----

    /// Ana removes Dan, then Ben saves: what Ben writes is sealed to an epoch Dan was never given.
    fn saved_after_a_removal() -> (tempfile::TempDir, Hash, Vec<Laptop>, Hash) {
        let (t, id, laptops) = team(3);
        laptops[0].on(id, t.path()).remove(laptops[2].keys.signer()).unwrap();
        let v = laptops[1].save(id, t.path(), b"after Dan left", vec![]);
        (t, id, laptops, v)
    }

    #[test]
    fn removed_device_cannot_open_a_version_saved_after_its_removal() {
        let (t, id, laptops, v) = saved_after_a_removal();
        let uuid = uuid_of(&laptops[1].on(id, t.path()).sync().unwrap(), &v);
        let dan = laptops[2].on(id, t.path()).sync().unwrap();
        assert!(dan.epochs.iter().all(|e| !opens_with(&file_of(t.path(), uuid), &e.identity).unwrap()));
    }

    #[test]
    fn remaining_member_saves_under_the_epoch_issued_at_a_removal() {
        let (t, id, laptops, v) = saved_after_a_removal();
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        let Body::Version(fields) = &state.versions[&v].body else { unreachable!() };
        assert_eq!(fields.epoch, state.current_epoch().unwrap().number);
    }

    /// Ana's chain holds `key` files only she can open; her removal must still keep what she wrote.
    #[test]
    fn removed_admins_version_before_its_cutoff_stays_accepted() {
        let (t, id, laptops) = team(3);
        let v = laptops[0].save(id, t.path(), b"v1 by Ana", vec![]);
        laptops[1].on(id, t.path()).remove(laptops[0].keys.signer()).unwrap();
        let state = laptops[2].on(id, t.path()).sync().unwrap();
        assert!(!state.is_member(&laptops[0].keys.signer()) && state.versions.contains_key(&v));
    }

    // ---- equivocation ----

    /// Ben signs two different versions that both claim his next seq.
    fn equivocated() -> (tempfile::TempDir, Hash, Vec<Laptop>, [Header; 2]) {
        let (t, id, laptops) = team(2);
        let state = laptops[1].on(id, t.path()).sync().unwrap();
        let (seq, prev) = state.own.unwrap();
        let (epoch, head) = (state.current_epoch().unwrap(), state.admin_head(&laptops[1].keys.signer()));
        let twins = [0, 1].map(|_| forge(t.path(), &laptops[1].keys, id, seq + 1, Some(prev), head, epoch));
        (t, id, laptops, twins)
    }

    #[test]
    fn two_records_with_one_signer_and_seq_flag_the_device_compromised() {
        let (t, id, laptops, _) = equivocated();
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(state.flags.contains(&(laptops[1].keys.signer(), Flag::Equivocation)));
    }

    #[test]
    fn equivocating_records_are_held_not_accepted() {
        let (t, id, laptops, twins) = equivocated();
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(twins.iter().all(|h| !state.versions.contains_key(&h.record_hash()) && state.held.contains(&h.uuid)));
    }

    #[test]
    fn equivocating_admin_records_are_held_not_accepted() {
        let (t, id, laptops) = team(2);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        let (ben, epoch) = (laptops[1].keys.signer(), state.current_epoch().unwrap().recipient());
        for _ in 0..2 {
            let body = Body::Member(Member::Remove { device: ben, cutoff: state.last_accepted(&ben) });
            forge_body(t.path(), &laptops[0].keys, &state, state.tips(), None, body, b"", &[&epoch]);
        }
        assert!(laptops[0].on(id, t.path()).sync().unwrap().is_member(&ben));
    }

    #[test]
    fn honest_reencryption_of_one_record_is_not_equivocation() {
        let (t, id, laptops) = team(2);
        let v1 = laptops[1].save(id, t.path(), b"v1", vec![]);
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        let uuid = uuid_of(&state, &v1);
        let epoch = state.current_epoch().unwrap();
        let plain = age::decrypt(&epoch.identity, &fs::read(file_of(t.path(), uuid)).unwrap()).unwrap();
        let again = age::encrypt(&epoch.recipient(), &plain).unwrap();
        let hash8 = &hex::encode(Sha256::digest(&again))[..RESTORE_HASH_HEX];
        fs::write(t.path().join(format!("{uuid}-{hash8}.rumble")), again).unwrap();
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(state.flags.is_empty() && state.versions.contains_key(&v1));
    }

    // ---- regressions from the adversarial review ----

    #[test]
    fn squatted_restore_name_does_not_stop_a_restore() {
        let (t, id, laptops) = team(2);
        let v1 = laptops[0].save(id, t.path(), b"v1", vec![]);
        let (uuid, hash, _) = laptops[0].on(id, t.path()).sync().unwrap().accepted[&v1].clone();
        fs::remove_file(file_of(t.path(), uuid)).unwrap();
        fs::write(t.path().join(format!("{uuid}-{}.rumble", &hex::encode(hash)[..RESTORE_HASH_HEX])), b"junk").unwrap();
        laptops[0].on(id, t.path()).sync().unwrap();
        assert!(laptops[1].on(id, t.path()).sync().unwrap().versions.contains_key(&v1));
    }

    #[test]
    fn record_withheld_until_after_a_removal_is_not_kept() {
        let (t, id, laptops) = team(2);
        let (ana, ben) = (&laptops[0], &laptops[1]);
        let state = ben.on(id, t.path()).sync().unwrap();
        let (seq, prev) = state.own.unwrap();
        let (epoch, head) = (state.current_epoch().unwrap(), state.admin_head(&ben.keys.signer()));
        let before = transport_names(t.path());
        let withheld = forge(t.path(), &ben.keys, id, seq + 1, Some(prev), head, epoch);
        let aside = withhold(t.path(), &before);
        forge(t.path(), &ben.keys, id, seq + 2, Some(withheld.record_hash()), head, epoch);
        ana.on(id, t.path()).remove(ben.keys.signer()).unwrap();
        deliver(aside, t.path());
        let state = ana.on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, withheld.uuid), Some(RecordError::AfterRemovalCutoff)));
    }

    #[test]
    fn record_that_skips_seq_is_rejected() {
        let (t, id, laptops) = team(2);
        let state = laptops[1].on(id, t.path()).sync().unwrap();
        let (seq, prev) = state.own.unwrap();
        let head = state.admin_head(&laptops[1].keys.signer());
        let forged = forge(t.path(), &laptops[1].keys, id, seq + 1000, Some(prev), head, state.current_epoch().unwrap());
        let state = laptops[0].on(id, t.path()).sync().unwrap();
        assert!(matches!(rejected_for(&state, forged.uuid), Some(RecordError::Malformed("seq"))));
    }

    /// Two admins approve offline at once, so the admin chain has two tips; the joiner writes next.
    #[test]
    fn member_approved_on_one_admin_branch_can_write() {
        for _ in 0..6 {
            let (t, id, laptops) = team(1);
            let t = t.path();
            let (ana, bea, cam, dee) = (&laptops[0], Laptop::new(), Laptop::new(), Laptop::new());
            join_as(ana, &bea, id, t, "Bea", true);
            let cams = ask(ana, &cam, id, t, "Cam");
            let dees = ask(&bea, &dee, id, t, "Dee");
            let before = transport_names(t);
            ana.on(id, t).approve(cam.keys.signer(), false, &cams).unwrap();
            let aside = withhold(t, &before);
            bea.on(id, t).approve(dee.keys.signer(), false, &dees).unwrap();
            deliver(aside, t);
            dee.on(id, t).confirm(&dees).unwrap();
            let v = dee.save(id, t, b"by Dee", vec![]);
            assert!(ana.on(id, t).sync().unwrap().versions.contains_key(&v));
        }
    }

    #[test]
    fn version_is_judged_by_its_signed_epoch_whatever_else_it_is_sealed_to() {
        let (t, id, laptops) = team(2);
        let (ana, ben) = (&laptops[0], &laptops[1]);
        let a = ana.on(id, t.path());
        let mut state = a.sync().unwrap();
        a.rotate(&mut state).unwrap();
        let second = a.sync().unwrap().current_epoch().unwrap().clone();
        let state = ben.on(id, t.path()).sync().unwrap();
        let first = state.epochs.iter().find(|e| e.number == 1).unwrap().recipient();
        let content = b"sealed to both epochs";
        let body = Body::Version(VersionFields {
            file_id: FILE,
            version_id: Uuid::new_v4(),
            epoch: 2,
            file_name: "nda.docx".into(),
            media_type: DOCX.into(),
            size: content.len() as u64,
            created_at_ms: 0,
            content_sha256: Sha256::digest(content).into(),
        });
        let head = Some(state.admin_head(&ben.keys.signer()));
        let v = forge_body(t.path(), &ben.keys, &state, vec![], head, body, content, &[&first, &second.recipient()]);
        assert!(ana.on(id, t.path()).sync().unwrap().versions.contains_key(&v.record_hash()));
    }

    #[test]
    fn half_synced_version_is_not_imported() {
        let (t, id, laptops) = team(2);
        let big = vec![7u8; 400 * 1024];
        let v1 = laptops[0].save(id, t.path(), &big, vec![]);
        let path = file_of(t.path(), laptops[0].on(id, t.path()).sync().unwrap().accepted[&v1].0);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert!(!laptops[1].on(id, t.path()).sync().unwrap().versions.contains_key(&v1));
    }

    #[test]
    fn transport_names_that_are_not_records_are_ignored() {
        let ok = Uuid::new_v4().to_string();
        let names = [format!("{ok}.rumble"), format!("{ok}-0a1b2c3d.rumble")];
        let junk = [format!("{ok} (1).rumble"), format!("{ok}.rumble.tmp"), "desktop.ini".into(), format!("{ok}-0A1B2C3D.rumble"), format!("{}.rumble", ok.to_uppercase())];
        assert!(names.iter().all(|n| claimed_uuid(n).is_some()) && junk.iter().all(|n| claimed_uuid(n).is_none()));
    }
}
