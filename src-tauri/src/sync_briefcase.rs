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
    self, Body, DeviceFields, DeviceKeys, Epoch, Hash, Header, KeyFields, Member, Membership, PublicKey,
    RecordError, VersionFields,
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

/// A verified record in this device's store: its signature, uuid and matter have been checked.
struct Record {
    uuid: Uuid,
    ciphertext: Hash,
    path: PathBuf,
    header: Header,
    /// The epoch (number, recipient) whose identity opened it; `None` if this device's own key did.
    opened_by: Option<(u64, String)>,
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

/// One file's version graph. Identical versions (same parents, same content) count once.
#[derive(Debug, Default, PartialEq)]
pub struct FileHistory {
    pub versions: BTreeSet<Hash>,
    pub heads: BTreeSet<Hash>,
    /// Versions that two or more distinct versions name as a parent.
    pub forks: BTreeSet<Hash>,
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
    pub devices: HashMap<PublicKey, DeviceFields>,
    pub requests: HashMap<PublicKey, DeviceFields>,
    pub versions: HashMap<Hash, Header>,
    pub files: BTreeMap<Uuid, FileHistory>,
    pub held: Vec<Uuid>,
    pub rejected: Vec<(Uuid, RecordError)>,
    /// Transport files that did not verify this time; retried on the next sync.
    pub unverified: Vec<(String, RecordError)>,
    /// An admin file a member's record names that this device hasn't seen: new writes wait for it.
    pub write_block: Option<Hash>,
    own: Option<(u64, Hash)>,
    accepted: HashMap<Hash, (Uuid, Hash, PathBuf)>,
    restorable_genesis: Option<(Uuid, PathBuf)>,
}

impl State {
    pub fn is_member(&self, device: &PublicKey) -> bool {
        self.roll.members.contains(device) && !self.roll.removed.contains_key(device)
    }

    pub fn current_epoch(&self) -> Option<&Epoch> {
        self.epochs.iter().max_by_key(|e| e.number)
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

    /// The newest admin file this device has seen, as a record's `admin_head`.
    fn admin_head(&self) -> Hash {
        self.tips()[0]
    }

    /// Membership as the admin chain stood at `heads`: their ancestors' approvals, plus every cutoff.
    fn membership_at(&self, heads: &[Hash]) -> Membership {
        let mut m = Membership { removed: self.roll.removed.clone(), ..Membership::default() };
        m.admins.extend(self.genesis_admin);
        m.members.extend(self.genesis_admin);
        let mut stack = heads.to_vec();
        let mut visited = HashSet::new();
        while let Some(h) = stack.pop() {
            let Some(header) = self.admin.get(&h).filter(|_| visited.insert(h)) else { continue };
            if let Body::Member(Member::Approve { device, admin }) = &header.body {
                m.members.insert(*device);
                if *admin {
                    m.admins.insert(*device);
                }
            }
            stack.extend(&header.parents);
        }
        m
    }

    fn knows_admin_file(&self, h: &Hash) -> bool {
        *h == self.id || self.admin.contains_key(h)
    }

    /// The highest-seq accepted record a device signed: what a removal names as its cutoff.
    fn last_accepted(&self, device: &PublicKey) -> Option<Hash> {
        let all = self.admin.iter().chain(&self.versions);
        all.filter(|(_, h)| h.signer == *device).max_by_key(|(_, h)| h.seq).map(|(hash, _)| *hash)
    }
}

/// This device's handle on one briefcase: records in `transport`, its own ciphertext copies under `data`.
pub struct Briefcase<'k> {
    pub id: Hash,
    keys: &'k DeviceKeys,
    transport: PathBuf,
    store: PathBuf,
    temp: PathBuf,
}

impl<'k> Briefcase<'k> {
    pub fn open(id: Hash, keys: &'k DeviceKeys, transport: &Path, data: &Path) -> io::Result<Self> {
        let root = data.join("briefcases").join(hex::encode(id));
        let (store, temp) = (root.join("store"), root.join("tmp"));
        fs::create_dir_all(&store)?;
        fs::create_dir_all(&temp)?;
        Ok(Briefcase { id, keys, transport: transport.into(), store, temp })
    }

    /// Start a briefcase: the genesis file, the first epoch's `key` file, and this device's own record.
    pub fn create(keys: &'k DeviceKeys, transport: &Path, data: &Path, display_name: &str) -> Result<Self, RecordError> {
        let mut nonce = [0u8; NONCE_BYTES];
        rand_core::OsRng.fill_bytes(&mut nonce);
        let genesis = [GENESIS_MAGIC, &nonce, &keys.signer()].concat();
        let briefcase = Briefcase::open(Sha256::digest(&genesis).into(), keys, transport, data)?;
        briefcase.publish(Uuid::new_v4(), |out| Ok(io::Write::write_all(out, &genesis)?))?;
        let mut state = briefcase.sync()?;
        let epoch = Epoch::generate(FIRST_EPOCH);
        briefcase.write_key_file(&mut state, vec![briefcase.id], &epoch, &[keys.recipient()])?;
        let mut state = briefcase.sync()?;
        let epoch = state.current_epoch().ok_or(RecordError::NotMember)?.recipient();
        briefcase.describe(&mut state, &epoch, display_name)?;
        Ok(briefcase)
    }

    /// A join request: this device's own record, readable by members through the link's epoch recipient.
    pub fn request(&self, epoch: &x25519::Recipient, display_name: &str) -> Result<Hash, RecordError> {
        let mut state = self.sync()?;
        self.describe(&mut state, epoch, display_name)
    }

    pub fn approve(&self, device: PublicKey, admin: bool) -> Result<Hash, RecordError> {
        let mut state = self.writable_as_admin()?;
        let fields = state.requests.get(&device).ok_or(RecordError::Malformed("no request from that device"))?;
        let joiner: x25519::Recipient = fields.recipient.parse().map_err(|_| RecordError::Malformed("recipient"))?;
        let approval = self.admin_record(&mut state, Member::Approve { device, admin })?;
        let mut devices = vec![joiner];
        for (signer, fields) in &state.devices {
            if state.is_member(signer) {
                devices.push(fields.recipient.parse().map_err(|_| RecordError::Malformed("recipient"))?);
            }
        }
        for epoch in state.epochs.clone() {
            self.write_key_file(&mut state, vec![approval], &epoch, &devices)?;
        }
        Ok(approval)
    }

    /// Remove a device after its last record this device accepted. Rotating the epoch is a later slice.
    pub fn remove(&self, device: PublicKey) -> Result<Hash, RecordError> {
        let mut state = self.writable_as_admin()?;
        let cutoff = state.last_accepted(&device);
        self.admin_record(&mut state, Member::Remove { device, cutoff })
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
        let head = Some(state.admin_head());
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
                let state = fold(self.id, self.keys.signer(), opened, unverified);
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
        let head = Some(state.admin_head());
        self.seal_record(state, vec![], head, body, |header, out| {
            sync_crypto::seal(self.keys, header, io::empty(), &[epoch, &me], out)
        })
    }

    fn admin_record(&self, state: &mut State, member: Member) -> Result<Hash, RecordError> {
        let epoch = state.current_epoch().ok_or(RecordError::NotMember)?.recipient();
        let parents = state.tips();
        self.seal_record(state, parents, None, Body::Member(member), |header, out| {
            sync_crypto::seal(self.keys, header, io::empty(), &[&epoch], out)
        })
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
        let header = Header {
            briefcase_id: self.id,
            uuid: Uuid::new_v4(),
            signer: self.keys.signer(),
            seq: state.own.map_or(1, |(seq, _)| seq + 1),
            prev: state.own.map(|(_, h)| h),
            parents,
            admin_head,
            body,
        };
        self.publish(header.uuid, |out| seal(&header, out))?;
        let hash = header.record_hash();
        state.own = Some((header.seq, hash));
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
        let (header, rest, index) = sync_crypto::open(&mut file, uuid, &self.id, &self.identities(epochs))?;
        let epoch = match &header.body {
            Body::Key(fields) => Some(sync_crypto::epoch_from(fields, rest)?),
            _ => None,
        };
        let opened_by = index.checked_sub(1).map(|i| (epochs[i].number, epochs[i].recipient().to_string()));
        Ok(Opening::Record(Record { uuid, ciphertext, path: path.into(), header, opened_by, epoch }))
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
                return Ok(opened);
            }
        }
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
            self.read(&temp, uuid, copied, &opened.epochs)?;
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
            if !present.contains(hash) {
                self.place(path, &format!("{uuid}-{}{RECORD_SUFFIX}", &hex::encode(hash)[..RESTORE_HASH_HEX]))?;
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
fn fold(id: Hash, me: PublicKey, opened: Opened, unverified: Vec<(String, RecordError)>) -> State {
    let mut state = State { id, unverified, ..State::default() };
    state.own = opened.records.iter().filter(|r| r.header.signer == me).max_by_key(|r| r.header.seq).map(|r| (r.header.seq, r.header.record_hash()));
    let Some(genesis) = opened.genesis else { return state };
    state.genesis_admin = Some(genesis.admin);
    state.restorable_genesis = Some((genesis.uuid, genesis.path));
    let mut records: HashMap<Hash, Record> = HashMap::new();
    for r in opened.records {
        records.entry(r.header.record_hash()).or_insert(r);
    }
    fn accept(state: &mut State, hash: Hash, r: &Record) {
        state.accepted.insert(hash, (r.uuid, r.ciphertext, r.path.clone()));
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
            let at = state.membership_at(parents);
            let verdict = match &r.header.body {
                _ if !at.admins.contains(&r.header.signer) => Err(RecordError::NotAdmin),
                Body::Member(Member::Remove { device, .. }) if at.admins.contains(device) => Err(RecordError::RemovesAnAdmin),
                _ => Ok(()),
            };
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

    let removals: Vec<(PublicKey, Option<Hash>)> = state
        .admin
        .values()
        .filter_map(|h| match h.body {
            Body::Member(Member::Remove { device, cutoff }) => Some((device, cutoff)),
            _ => None,
        })
        .collect();
    for (device, cutoff) in removals {
        let kept = kept_chain(&records, device, cutoff);
        state.roll.removed.entry(device).and_modify(|k| k.retain(|h| kept.contains(h))).or_insert(kept);
    }
    let all: Vec<Hash> = state.admin.keys().copied().collect();
    let Membership { admins, members, .. } = state.membership_at(&all);
    (state.roll.admins, state.roll.members) = (admins, members);

    let mut by_kind: Vec<(&Hash, &Record)> = records.iter().filter(|(_, r)| !matches!(r.header.body, Body::Member(_))).collect();
    // `key` records first: versions need the accepted epochs.
    by_kind.sort_by_key(|(_, r)| !matches!(r.header.body, Body::Key(_)));
    for (hash, r) in by_kind {
        let h = &r.header;
        let verdict = match &h.body {
            Body::Key(_) => {
                if h.parents.is_empty() || !h.parents.iter().all(|p| state.knows_admin_file(p)) {
                    state.held.push(r.uuid);
                    continue;
                }
                match state.membership_at(&h.parents).admins.contains(&h.signer) {
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
                    // A device describes itself before it is approved, so it is judged at this device's head.
                    Body::Device(fields) => {
                        if !state.roll.members.contains(&h.signer) && !state.roll.removed.contains_key(&h.signer) {
                            state.requests.insert(h.signer, fields.clone());
                            continue;
                        }
                        let verdict = state.roll.admit(h);
                        if verdict.is_ok() {
                            state.devices.insert(h.signer, fields.clone());
                        }
                        verdict
                    }
                    Body::Version(fields) => state.membership_at(&[head]).admit(h).and_then(|()| {
                        let epoch_held = state.epochs.iter().any(|e| {
                            r.opened_by.as_ref() == Some(&(e.number, e.recipient().to_string())) && e.number == fields.epoch
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
    state.files = histories(&state.versions);
    state
}

/// The records of a removed device that stay accepted: its cutoff, and what the cutoff's `prev` chain
/// reaches with `seq` falling by one each step. A record the device signs later can't join that chain,
/// whatever `seq` it claims, because the cutoff hash already fixes every link. A missing link ends it.
fn kept_chain(records: &HashMap<Hash, Record>, device: PublicKey, cutoff: Option<Hash>) -> HashSet<Hash> {
    let mut kept = HashSet::new();
    let (mut next, mut want_seq) = (cutoff, None);
    while let Some(hash) = next {
        let record = records.get(&hash).map(|r| &r.header).filter(|h| h.signer == device);
        if want_seq.is_some_and(|seq| record.is_some_and(|h| h.seq != seq)) {
            break;
        }
        kept.insert(hash);
        let Some(h) = record else { break };
        (next, want_seq) = (h.prev, h.seq.checked_sub(1));
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

    /// The link's epoch recipient goes to the joiner out of band; the joiner requests; an admin approves.
    fn join(admin: &Laptop, joiner: &Laptop, id: Hash, t: &Path, name: &str) {
        let link = admin.on(id, t).sync().unwrap().current_epoch().unwrap().recipient();
        joiner.on(id, t).request(&link, name).unwrap();
        admin.on(id, t).approve(joiner.keys.signer(), false).unwrap();
    }

    /// Ana creates a briefcase; every other laptop joins it.
    fn team(n: usize) -> (tempfile::TempDir, Hash, Vec<Laptop>) {
        let t = tempfile::tempdir().unwrap();
        let laptops: Vec<Laptop> = (0..n).map(|_| Laptop::new()).collect();
        let id = Briefcase::create(&laptops[0].keys, t.path(), laptops[0].data.path(), "Ana Lim").unwrap().id;
        for (i, joiner) in laptops.iter().enumerate().skip(1) {
            join(&laptops[0], joiner, id, t.path(), &format!("Member {i}"));
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
        let link = ana.on(id, t).sync().unwrap().current_epoch().unwrap().recipient();

        // Ben opens the link, requests and stops; he can open nothing yet.
        ben.on(id, t).request(&link, "Ben Tan").unwrap();
        assert!(ben.on(id, t).sync().unwrap().versions.is_empty());

        // Ana starts, sees the request, approves and stops.
        let state = ana.on(id, t).sync().unwrap();
        assert_eq!(state.requests[&ben.keys.signer()].display_name, "Ben Tan");
        ana.on(id, t).approve(ben.keys.signer(), false).unwrap();

        // Ben starts, is in, and saves v2 on top of v1.
        assert!(ben.on(id, t).sync().unwrap().is_member(&ben.keys.signer()));
        let v2 = ben.save(id, t, b"NDA v2 by Ben", vec![v1]);

        // Ana starts and sees v2 by Ben.
        let state = ana.on(id, t).sync().unwrap();
        assert_eq!(state.files[&FILE].heads, BTreeSet::from([v2]));
        assert_eq!(state.devices[&state.versions[&v2].signer].display_name, "Ben Tan");

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
        let forged = forge(t.path(), &outsider, id, 1, None, state.admin_head(), state.current_epoch().unwrap());
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
        let forged = forge(t.path(), &ben.keys, id, before_removal.versions[&v1].seq, Some(first), before_removal.admin_head(), before_removal.current_epoch().unwrap());
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

    #[test]
    fn transport_names_that_are_not_records_are_ignored() {
        let ok = Uuid::new_v4().to_string();
        let names = [format!("{ok}.rumble"), format!("{ok}-0a1b2c3d.rumble")];
        let junk = [format!("{ok} (1).rumble"), format!("{ok}.rumble.tmp"), "desktop.ini".into(), format!("{ok}-0A1B2C3D.rumble"), format!("{}.rumble", ok.to_uppercase())];
        assert!(names.iter().all(|n| claimed_uuid(n).is_some()) && junk.iter().all(|n| claimed_uuid(n).is_none()));
    }
}
