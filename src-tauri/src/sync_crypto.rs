//! ABOUTME: Team sync record crypto: device and epoch keys, signed age records, verify-before-release.
//! ABOUTME: Canonical header encoding and the v1 safety number. Not yet wired into the app (#21 spike).
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::str::FromStr;

use age::secrecy::ExposeSecret;
use age::x25519;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Opens every canonical header, so a header can never verify as another protocol's message.
const RECORD_MAGIC: &[u8; 16] = b"rumble-record-v1";
/// Prefix of the plaintext's first line. `#` makes a decrypted `key` file a valid age identity file.
const HEADER_LINE_PREFIX: &[u8] = b"# rumble-v1 ";
/// Bounds memory spent on the header line before its signature is checked.
const MAX_HEADER_LINE_BYTES: u64 = 16 * 1024;
const MAX_TEXT_BYTES: usize = 1024;
const MAX_PARENTS: usize = 64;
const COPY_CHUNK_BYTES: usize = 64 * 1024;
/// Decrypted plaintext is readable by the owner only while it waits in the temp folder.
#[cfg(unix)]
const TEMP_FILE_MODE: u32 = 0o600;
/// Opens every invite link, so link bytes can never verify as a record header.
const LINK_MAGIC: &[u8; 14] = b"rumble-link-v1";
const SIGNATURE_BYTES: usize = 64;
/// Bounds a signed link carried inside an `invite` or `request` record.
const MAX_LINK_BYTES: usize = 4 * 1024;
const SAFETY_DOMAIN: &[u8] = b"rumble-safety-v1";
const SAFETY_DIGITS: usize = 60;
const SAFETY_GROUP_DIGITS: usize = 5;

const KIND_VERSION: u8 = 1;
const KIND_KEY: u8 = 2;
const KIND_MEMBER: u8 = 3;
const KIND_DEVICE: u8 = 4;
const KIND_REQUEST: u8 = 5;
const MEMBER_APPROVE: u8 = 1;
const MEMBER_REMOVE: u8 = 2;
const MEMBER_INVITE: u8 = 3;
const MEMBER_REVOKE: u8 = 4;

pub type Hash = [u8; 32];
pub type PublicKey = [u8; 32];

#[derive(Debug)]
pub enum RecordError {
    Io(io::Error),
    Decrypt(age::DecryptError),
    Encrypt(age::EncryptError),
    Malformed(&'static str),
    BadSignature,
    UuidMismatch,
    ForeignMatter,
    WrongKind,
    SignerMismatch,
    NotMember,
    NotAdmin,
    AfterRemovalCutoff,
    RemovesTheLastAdmin,
    NeedsSecondAdmin,
    InviteNotOpen,
    SafetyNumberMismatch,
    NotConfirmed,
    WaitingForMembershipUpdate,
    EpochKeyMismatch,
    WrongEpoch,
    ContentMismatch,
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordError::Io(e) => write!(f, "record io: {e}"),
            RecordError::Decrypt(e) => write!(f, "record decrypt: {e}"),
            RecordError::Encrypt(e) => write!(f, "record encrypt: {e}"),
            other => write!(f, "record rejected: {other:?}"),
        }
    }
}

impl From<io::Error> for RecordError {
    fn from(e: io::Error) -> Self {
        RecordError::Io(e)
    }
}

pub struct DeviceKeys {
    signing: SigningKey,
    receiving: x25519::Identity,
}

impl DeviceKeys {
    pub fn generate() -> Self {
        DeviceKeys {
            signing: SigningKey::generate(&mut rand_core::OsRng),
            receiving: x25519::Identity::generate(),
        }
    }

    pub fn signer(&self) -> PublicKey {
        self.signing.verifying_key().to_bytes()
    }

    pub fn recipient(&self) -> x25519::Recipient {
        self.receiving.to_public()
    }

    pub fn identity(&self) -> &x25519::Identity {
        &self.receiving
    }
}

/// A matter key epoch: an age identity that versions are encrypted to.
#[derive(Clone)]
pub struct Epoch {
    pub number: u64,
    pub identity: x25519::Identity,
}

impl Epoch {
    pub fn generate(number: u64) -> Self {
        Epoch { number, identity: x25519::Identity::generate() }
    }

    pub fn recipient(&self) -> x25519::Recipient {
        self.identity.to_public()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VersionFields {
    pub file_id: Uuid,
    pub version_id: Uuid,
    pub epoch: u64,
    pub file_name: String,
    pub media_type: String,
    pub size: u64,
    pub created_at_ms: i64,
    pub content_sha256: Hash,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyFields {
    pub epoch: u64,
    pub epoch_recipient: String,
}

/// An admin's change to who may sign: approve a device (closing the invite it answered, or none to make
/// a member an admin), remove it after its last accepted record, or issue or revoke an invite.
#[derive(Clone, Debug, PartialEq)]
pub enum Member {
    Approve { device: PublicKey, admin: bool, invite: Option<Uuid> },
    Remove { device: PublicKey, cutoff: Option<Hash> },
    /// The signed link bytes, kept so every admin computes the safety number from its own copy.
    Invite { link: Vec<u8> },
    Revoke { invite: Uuid },
}

/// A device describing itself; from a device not yet approved, it is a join request.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceFields {
    pub display_name: String,
    pub recipient: String,
}

/// A join request: the joiner's self-description and the signed link it answers. Never matter state.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestFields {
    pub device: DeviceFields,
    pub link: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Version(VersionFields),
    Key(KeyFields),
    Member(Member),
    Device(DeviceFields),
    Request(RequestFields),
}

/// Every signed field of a record. `encode` is the one canonical form the signature covers.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub briefcase_id: Hash,
    pub uuid: Uuid,
    pub signer: PublicKey,
    pub seq: u64,
    /// The signer's own previous record, so a removal cutoff fixes every record before it.
    pub prev: Option<Hash>,
    pub parents: Vec<Hash>,
    pub admin_head: Option<Hash>,
    pub body: Body,
}

impl Header {
    /// Fixed order; fixed-size fields raw, integers big-endian, text and lists u32-length-prefixed.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.extend_from_slice(RECORD_MAGIC);
        out.extend_from_slice(&self.briefcase_id);
        out.push(match self.body {
            Body::Version(_) => KIND_VERSION,
            Body::Key(_) => KIND_KEY,
            Body::Member(_) => KIND_MEMBER,
            Body::Device(_) => KIND_DEVICE,
            Body::Request(_) => KIND_REQUEST,
        });
        out.extend_from_slice(self.uuid.as_bytes());
        out.extend_from_slice(&self.signer);
        out.extend_from_slice(&self.seq.to_be_bytes());
        put_hash_option(&mut out, &self.prev);
        out.extend_from_slice(&(self.parents.len() as u32).to_be_bytes());
        self.parents.iter().for_each(|p| out.extend_from_slice(p));
        put_hash_option(&mut out, &self.admin_head);
        match &self.body {
            Body::Version(v) => {
                out.extend_from_slice(v.file_id.as_bytes());
                out.extend_from_slice(v.version_id.as_bytes());
                out.extend_from_slice(&v.epoch.to_be_bytes());
                put_text(&mut out, &v.file_name);
                put_text(&mut out, &v.media_type);
                out.extend_from_slice(&v.size.to_be_bytes());
                out.extend_from_slice(&v.created_at_ms.to_be_bytes());
                out.extend_from_slice(&v.content_sha256);
            }
            Body::Key(k) => {
                out.extend_from_slice(&k.epoch.to_be_bytes());
                put_text(&mut out, &k.epoch_recipient);
            }
            Body::Member(Member::Approve { device, admin, invite }) => {
                out.push(MEMBER_APPROVE);
                out.extend_from_slice(device);
                out.push(u8::from(*admin));
                put_uuid_option(&mut out, invite);
            }
            Body::Member(Member::Remove { device, cutoff }) => {
                out.push(MEMBER_REMOVE);
                out.extend_from_slice(device);
                put_hash_option(&mut out, cutoff);
            }
            Body::Member(Member::Invite { link }) => {
                out.push(MEMBER_INVITE);
                put_text(&mut out, link);
            }
            Body::Member(Member::Revoke { invite }) => {
                out.push(MEMBER_REVOKE);
                out.extend_from_slice(invite.as_bytes());
            }
            Body::Device(d) => {
                put_text(&mut out, &d.display_name);
                put_text(&mut out, &d.recipient);
            }
            Body::Request(r) => {
                put_text(&mut out, &r.device.display_name);
                put_text(&mut out, &r.device.recipient);
                put_text(&mut out, &r.link);
            }
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RecordError> {
        let mut r = Cursor(bytes);
        if r.take(RECORD_MAGIC.len())? != RECORD_MAGIC {
            return Err(RecordError::Malformed("magic"));
        }
        let briefcase_id = r.array()?;
        let kind = r.take(1)?[0];
        let uuid = Uuid::from_bytes(r.array()?);
        let signer = r.array()?;
        let seq = r.u64()?;
        let prev = r.hash_option()?;
        let count = r.u32()? as usize;
        if count > MAX_PARENTS {
            return Err(RecordError::Malformed("parents"));
        }
        let parents = (0..count).map(|_| r.array()).collect::<Result<_, _>>()?;
        let admin_head = r.hash_option()?;
        let body = match kind {
            KIND_VERSION => Body::Version(VersionFields {
                file_id: Uuid::from_bytes(r.array()?),
                version_id: Uuid::from_bytes(r.array()?),
                epoch: r.u64()?,
                file_name: r.text()?,
                media_type: r.text()?,
                size: r.u64()?,
                created_at_ms: r.u64()? as i64,
                content_sha256: r.array()?,
            }),
            KIND_KEY => Body::Key(KeyFields { epoch: r.u64()?, epoch_recipient: r.text()? }),
            KIND_MEMBER => Body::Member(match r.take(1)?[0] {
                MEMBER_APPROVE => Member::Approve { device: r.array()?, admin: r.flag()?, invite: r.uuid_option()? },
                MEMBER_REMOVE => Member::Remove { device: r.array()?, cutoff: r.hash_option()? },
                MEMBER_INVITE => Member::Invite { link: r.bytes(MAX_LINK_BYTES, "link length")? },
                MEMBER_REVOKE => Member::Revoke { invite: Uuid::from_bytes(r.array()?) },
                _ => return Err(RecordError::Malformed("member action")),
            }),
            KIND_DEVICE => Body::Device(DeviceFields { display_name: r.text()?, recipient: r.text()? }),
            KIND_REQUEST => Body::Request(RequestFields {
                device: DeviceFields { display_name: r.text()?, recipient: r.text()? },
                link: r.bytes(MAX_LINK_BYTES, "link length")?,
            }),
            _ => return Err(RecordError::Malformed("kind")),
        };
        if !r.0.is_empty() {
            return Err(RecordError::Malformed("trailing bytes"));
        }
        Ok(Header { briefcase_id, uuid, signer, seq, prev, parents, admin_head, body })
    }

    /// The record's identity: independent of encryption, so an honest re-encryption is the same record.
    pub fn record_hash(&self) -> Hash {
        Sha256::digest(self.encode()).into()
    }
}

fn put_hash_option(out: &mut Vec<u8>, h: &Option<Hash>) {
    match h {
        Some(h) => {
            out.push(1);
            out.extend_from_slice(h);
        }
        None => out.push(0),
    }
}

fn put_uuid_option(out: &mut Vec<u8>, u: &Option<Uuid>) {
    match u {
        Some(u) => {
            out.push(1);
            out.extend_from_slice(u.as_bytes());
        }
        None => out.push(0),
    }
}

fn put_text(out: &mut Vec<u8>, s: impl AsRef<[u8]>) {
    let s = s.as_ref();
    out.extend_from_slice(&(s.len() as u32).to_be_bytes());
    out.extend_from_slice(s);
}

struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RecordError> {
        if self.0.len() < n {
            return Err(RecordError::Malformed("truncated"));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], RecordError> {
        Ok(self.take(N)?.try_into().expect("take returns N bytes"))
    }
    fn u32(&mut self) -> Result<u32, RecordError> {
        Ok(u32::from_be_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, RecordError> {
        Ok(u64::from_be_bytes(self.array()?))
    }
    fn flag(&mut self) -> Result<bool, RecordError> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(RecordError::Malformed("flag")),
        }
    }
    fn hash_option(&mut self) -> Result<Option<Hash>, RecordError> {
        match self.flag()? {
            false => Ok(None),
            true => Ok(Some(self.array()?)),
        }
    }
    fn uuid_option(&mut self) -> Result<Option<Uuid>, RecordError> {
        match self.flag()? {
            false => Ok(None),
            true => Ok(Some(Uuid::from_bytes(self.array()?))),
        }
    }
    fn bytes(&mut self, max: usize, what: &'static str) -> Result<Vec<u8>, RecordError> {
        let len = self.u32()? as usize;
        if len > max {
            return Err(RecordError::Malformed(what));
        }
        Ok(self.take(len)?.to_vec())
    }
    fn text(&mut self) -> Result<String, RecordError> {
        String::from_utf8(self.bytes(MAX_TEXT_BYTES, "text length")?).map_err(|_| RecordError::Malformed("utf8"))
    }
}

/// Who may sign, as the admin chain says. A removed device keeps its records up to its cutoff.
#[derive(Clone, Debug, Default)]
pub struct Membership {
    pub admins: HashSet<PublicKey>,
    pub members: HashSet<PublicKey>,
    /// Removed device → the record hashes on its own `prev` chain, up to and including its cutoff:
    /// the only records of its still accepted. Empty if it had none.
    pub removed: HashMap<PublicKey, HashSet<Hash>>,
}

impl Membership {
    pub fn admit(&self, h: &Header) -> Result<(), RecordError> {
        if let Some(kept) = self.removed.get(&h.signer) {
            return match kept.contains(&h.record_hash()) {
                true => Ok(()),
                false => Err(RecordError::AfterRemovalCutoff),
            };
        }
        if !self.members.contains(&h.signer) {
            return Err(RecordError::NotMember);
        }
        Ok(())
    }
}

/// Sign the header, then encrypt `header line ‖ payload` to `recipients`.
pub fn seal(
    signer: &DeviceKeys,
    header: &Header,
    mut payload: impl Read,
    recipients: &[&dyn age::Recipient],
    out: impl Write,
) -> Result<(), RecordError> {
    if header.signer != signer.signer() {
        return Err(RecordError::SignerMismatch);
    }
    let bytes = header.encode();
    // Refuse to sign what no peer could decode (oversized text or parent lists).
    Header::decode(&bytes)?;
    let signature = signer.signing.sign(&bytes);
    let encryptor = age::Encryptor::with_recipients(recipients.iter().copied())
        .map_err(RecordError::Encrypt)?;
    let mut w = encryptor.wrap_output(out)?;
    w.write_all(HEADER_LINE_PREFIX)?;
    w.write_all(format!("{} {}\n", hex::encode(&bytes), hex::encode(signature.to_bytes())).as_bytes())?;
    io::copy(&mut payload, &mut w)?;
    w.finish()?;
    Ok(())
}

/// A `key` file: the epoch identity, signed by an admin and encrypted to every member device.
pub fn seal_key_file(
    admin: &DeviceKeys,
    header: &Header,
    epoch: &Epoch,
    devices: &[x25519::Recipient],
    out: impl Write,
) -> Result<(), RecordError> {
    let recipients: Vec<&dyn age::Recipient> = devices.iter().map(|r| r as _).collect();
    let line = format!("{}\n", epoch.identity.to_string().expose_secret());
    seal(admin, header, line.as_bytes(), &recipients, out)
}

/// Decrypt with the first of `identities` that opens it, then check the signed header against the
/// file name and the matter. Returns which identity opened it, so the caller can bind the signed epoch.
pub fn open<'r, R: Read + Seek>(
    ciphertext: &'r mut R,
    file_uuid: Uuid,
    briefcase_id: &Hash,
    identities: &[&dyn age::Identity],
) -> Result<(Header, impl BufRead + 'r, usize), RecordError> {
    let mut decrypt_with = |identity: &dyn age::Identity| {
        ciphertext.seek(SeekFrom::Start(0))?;
        let decryptor = age::Decryptor::new(&mut *ciphertext).map_err(RecordError::Decrypt)?;
        decryptor.decrypt(std::iter::once(identity)).map(drop).map_err(RecordError::Decrypt)
    };
    let index = identities
        .iter()
        .position(|identity| !matches!(decrypt_with(*identity), Err(RecordError::Decrypt(age::DecryptError::NoMatchingKeys))))
        .ok_or(RecordError::Decrypt(age::DecryptError::NoMatchingKeys))?;
    ciphertext.seek(SeekFrom::Start(0))?;
    let decryptor = age::Decryptor::new(ciphertext).map_err(RecordError::Decrypt)?;
    let mut plain = BufReader::new(
        decryptor.decrypt(std::iter::once(identities[index])).map_err(RecordError::Decrypt)?,
    );
    let mut line = Vec::new();
    (&mut plain).take(MAX_HEADER_LINE_BYTES).read_until(b'\n', &mut line)?;
    let fields = line
        .strip_prefix(HEADER_LINE_PREFIX)
        .and_then(|l| l.strip_suffix(b"\n"))
        .ok_or(RecordError::Malformed("header line"))?;
    let (bytes_hex, sig_hex) = fields
        .split(|b| *b == b' ')
        .collect::<Vec<_>>()
        .try_into()
        .map(|[a, b]: [&[u8]; 2]| (a, b))
        .map_err(|_| RecordError::Malformed("header line"))?;
    let bytes = hex::decode(bytes_hex).map_err(|_| RecordError::Malformed("header hex"))?;
    let sig: [u8; 64] = hex::decode(sig_hex)
        .ok()
        .and_then(|s| s.try_into().ok())
        .ok_or(RecordError::Malformed("signature"))?;
    let header = Header::decode(&bytes)?;
    VerifyingKey::from_bytes(&header.signer)
        .and_then(|k| k.verify_strict(&bytes, &Signature::from_bytes(&sig)))
        .map_err(|_| RecordError::BadSignature)?;
    if header.uuid != file_uuid {
        return Err(RecordError::UuidMismatch);
    }
    if &header.briefcase_id != briefcase_id {
        return Err(RecordError::ForeignMatter);
    }
    Ok((header, plain, index))
}

/// Open a `key` file addressed to `device`; returns the epoch only if an admin signed this exact identity.
pub fn open_key_file(
    mut ciphertext: impl Read + Seek,
    file_uuid: Uuid,
    briefcase_id: &Hash,
    device: &DeviceKeys,
    membership: &Membership,
) -> Result<Epoch, RecordError> {
    let (header, mut rest, _) = open(&mut ciphertext, file_uuid, briefcase_id, &[&device.receiving])?;
    let Body::Key(fields) = &header.body else {
        return Err(RecordError::WrongKind);
    };
    membership.admit(&header)?;
    if !membership.admins.contains(&header.signer) {
        return Err(RecordError::NotAdmin);
    }
    epoch_from(fields, &mut rest)
}

/// The epoch identity a `key` record carries, only if it derives the public key the admin signed.
pub fn epoch_from(fields: &KeyFields, rest: impl Read) -> Result<Epoch, RecordError> {
    let mut secret = String::new();
    rest.take(MAX_HEADER_LINE_BYTES).read_to_string(&mut secret)?;
    let identity = x25519::Identity::from_str(secret.trim_end())
        .map_err(|_| RecordError::Malformed("epoch identity"))?;
    if identity.to_public().to_string() != fields.epoch_recipient {
        return Err(RecordError::EpochKeyMismatch);
    }
    Ok(Epoch { number: fields.epoch, identity })
}

/// Decrypt a version into `temp_dir` (local app data, never the transport), check signature, signer,
/// size and hash, then rename to `dest`. On any failure the temp file is removed and `dest` is untouched.
pub fn release_version(
    mut ciphertext: impl Read + Seek,
    file_uuid: Uuid,
    briefcase_id: &Hash,
    epochs: &[Epoch],
    membership: &Membership,
    temp_dir: &Path,
    dest: &Path,
) -> Result<Header, RecordError> {
    let identities: Vec<&dyn age::Identity> = epochs.iter().map(|e| &e.identity as _).collect();
    let (header, rest, opened_by) = open(&mut ciphertext, file_uuid, briefcase_id, &identities)?;
    let Body::Version(fields) = &header.body else {
        return Err(RecordError::WrongKind);
    };
    if fields.epoch != epochs[opened_by].number {
        return Err(RecordError::WrongEpoch);
    }
    membership.admit(&header)?;
    let temp = temp_dir.join(format!("{file_uuid}.{}.tmp", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, TEMP_FILE_MODE);
    let mut file = options.open(&temp)?;
    // One byte past the signed size is enough to reject an oversized payload without writing it all.
    let written = copy_hashed(&mut rest.take(fields.size.saturating_add(1)), &mut file).and_then(|(size, hash)| {
        if size != fields.size || hash != fields.content_sha256 {
            return Err(RecordError::ContentMismatch);
        }
        file.sync_all()?;
        Ok(())
    });
    drop(file);
    match written.and_then(|()| fs::rename(&temp, dest).map_err(RecordError::from)) {
        Ok(()) => Ok(header),
        Err(e) => {
            let _ = fs::remove_file(&temp);
            Err(e)
        }
    }
}

fn copy_hashed(from: &mut impl Read, to: &mut File) -> Result<(u64, Hash), RecordError> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; COPY_CHUNK_BYTES];
    let mut size = 0u64;
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            return Ok((size, hasher.finalize().into()));
        }
        hasher.update(&buf[..n]);
        to.write_all(&buf[..n])?;
        size += n as u64;
    }
}

/// An invite link: everything a joiner needs to ask, and no key that opens anything. No matter name.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub invite: Uuid,
    pub briefcase_id: Hash,
    /// The epoch's public recipient, so the request is readable by members only.
    pub epoch_recipient: String,
    pub admin: PublicKey,
    pub admin_name: String,
    pub invitee_name: String,
    /// The issuing admin's clock; that admin's own clock decides when the invite expires.
    pub issued_at_ms: i64,
}

impl Link {
    fn encode(&self) -> Vec<u8> {
        let mut out = LINK_MAGIC.to_vec();
        out.extend_from_slice(self.invite.as_bytes());
        out.extend_from_slice(&self.briefcase_id);
        put_text(&mut out, &self.epoch_recipient);
        out.extend_from_slice(&self.admin);
        put_text(&mut out, &self.admin_name);
        put_text(&mut out, &self.invitee_name);
        out.extend_from_slice(&self.issued_at_ms.to_be_bytes());
        out
    }

    /// The signed link bytes: the canonical link, then the admin's signature over it.
    pub fn sign(&self, admin: &DeviceKeys) -> Result<Vec<u8>, RecordError> {
        if self.admin != admin.signer() {
            return Err(RecordError::SignerMismatch);
        }
        let mut bytes = self.encode();
        // Refuse to sign what no peer could decode (oversized names).
        Link::decode(&bytes)?;
        if bytes.len() + SIGNATURE_BYTES > MAX_LINK_BYTES {
            return Err(RecordError::Malformed("link length"));
        }
        let signature = admin.signing.sign(&bytes);
        bytes.extend_from_slice(&signature.to_bytes());
        Ok(bytes)
    }

    /// Decode signed link bytes, only if the admin key they name signed exactly these bytes.
    pub fn verify(signed: &[u8]) -> Result<Link, RecordError> {
        if signed.len() > MAX_LINK_BYTES || signed.len() < SIGNATURE_BYTES {
            return Err(RecordError::Malformed("link length"));
        }
        let (bytes, sig) = signed.split_at(signed.len() - SIGNATURE_BYTES);
        let link = Link::decode(bytes)?;
        let sig: [u8; SIGNATURE_BYTES] = sig.try_into().expect("split at the signature length");
        VerifyingKey::from_bytes(&link.admin)
            .and_then(|k| k.verify_strict(bytes, &Signature::from_bytes(&sig)))
            .map_err(|_| RecordError::BadSignature)?;
        Ok(link)
    }

    fn decode(bytes: &[u8]) -> Result<Link, RecordError> {
        let mut r = Cursor(bytes);
        if r.take(LINK_MAGIC.len())? != LINK_MAGIC {
            return Err(RecordError::Malformed("link magic"));
        }
        let link = Link {
            invite: Uuid::from_bytes(r.array()?),
            briefcase_id: r.array()?,
            epoch_recipient: r.text()?,
            admin: r.array()?,
            admin_name: r.text()?,
            invitee_name: r.text()?,
            issued_at_ms: r.u64()? as i64,
        };
        if !r.0.is_empty() {
            return Err(RecordError::Malformed("link trailing bytes"));
        }
        Ok(link)
    }
}

/// v1: `SHA-256("rumble-safety-v1" ‖ signed link bytes ‖ joiner's Ed25519 key)`, read as a big-endian
/// integer mod 10^60, zero-padded, in 12 groups of 5 digits.
pub fn safety_number(signed_link: &[u8], joiner: &PublicKey) -> String {
    let mut n: [u8; 32] = Sha256::new()
        .chain_update(SAFETY_DOMAIN)
        .chain_update(signed_link)
        .chain_update(joiner)
        .finalize()
        .into();
    let mut digits = [0u8; SAFETY_DIGITS];
    for d in digits.iter_mut().rev() {
        let mut rem = 0u32;
        for byte in n.iter_mut() {
            let acc = (rem << 8) | u32::from(*byte);
            *byte = (acc / 10) as u8;
            rem = acc % 10;
        }
        *d = b'0' + rem as u8;
    }
    digits
        .chunks(SAFETY_GROUP_DIGITS)
        .map(|g| std::str::from_utf8(g).expect("ascii digits"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const BRIEFCASE: Hash = [7u8; 32];

    fn version_header(signer: &DeviceKeys, uuid: Uuid, seq: u64, content: &[u8]) -> Header {
        Header {
            briefcase_id: BRIEFCASE,
            uuid,
            signer: signer.signer(),
            seq,
            prev: None,
            parents: vec![],
            admin_head: Some([9u8; 32]),
            body: Body::Version(VersionFields {
                file_id: Uuid::from_bytes([1u8; 16]),
                version_id: Uuid::from_bytes([2u8; 16]),
                epoch: 1,
                file_name: "nda.docx".into(),
                media_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
                size: content.len() as u64,
                created_at_ms: 1_763_337_600_000,
                content_sha256: Sha256::digest(content).into(),
            }),
        }
    }

    fn key_header(admin: &DeviceKeys, uuid: Uuid, epoch: &Epoch) -> Header {
        Header {
            briefcase_id: BRIEFCASE,
            uuid,
            signer: admin.signer(),
            seq: 1,
            prev: None,
            parents: vec![[3u8; 32]],
            admin_head: None,
            body: Body::Key(KeyFields { epoch: epoch.number, epoch_recipient: epoch.recipient().to_string() }),
        }
    }

    fn seal_version(signer: &DeviceKeys, header: &Header, content: &[u8], epoch: &Epoch) -> Vec<u8> {
        let mut out = Vec::new();
        seal(signer, header, content, &[&epoch.recipient()], &mut out).unwrap();
        out
    }

    /// A member who holds the epoch re-encrypts a signed header line with a payload of their choice.
    fn reencrypt(original: &[u8], epoch: &Epoch, edit: impl Fn(Vec<u8>) -> Vec<u8>) -> Vec<u8> {
        let plain = age::decrypt(&epoch.identity, original).unwrap();
        age::encrypt(&epoch.recipient(), &edit(plain)).unwrap()
    }

    fn matter(members: &[&DeviceKeys], admins: &[&DeviceKeys]) -> Membership {
        Membership {
            admins: admins.iter().map(|d| d.signer()).collect(),
            members: members.iter().map(|d| d.signer()).collect(),
            removed: HashMap::new(),
        }
    }

    struct Release {
        dir: tempfile::TempDir,
        result: Result<Header, RecordError>,
    }

    impl Release {
        fn dest(&self) -> std::path::PathBuf {
            self.dir.path().join("work/nda.docx")
        }
        fn temp_dir_is_empty(&self) -> bool {
            fs::read_dir(self.dir.path().join("tmp")).unwrap().next().is_none()
        }
    }

    fn release(ciphertext: &[u8], uuid: Uuid, epochs: &[Epoch], membership: &Membership) -> Release {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("tmp")).unwrap();
        fs::create_dir_all(dir.path().join("work")).unwrap();
        let result = release_version(
            io::Cursor::new(ciphertext),
            uuid,
            &BRIEFCASE,
            epochs,
            membership,
            &dir.path().join("tmp"),
            &dir.path().join("work/nda.docx"),
        );
        Release { dir, result }
    }

    // ---- canonical encoding and safety-number test vectors ----

    fn vector_signer() -> SigningKey {
        SigningKey::from_bytes(&[0x11; 32])
    }

    fn vector_header() -> Header {
        Header {
            briefcase_id: [0xAB; 32],
            uuid: Uuid::from_bytes([0x01; 16]),
            signer: vector_signer().verifying_key().to_bytes(),
            seq: 3,
            prev: Some([0x66; 32]),
            parents: vec![[0x22; 32]],
            admin_head: Some([0x33; 32]),
            body: Body::Version(VersionFields {
                file_id: Uuid::from_bytes([0x44; 16]),
                version_id: Uuid::from_bytes([0x55; 16]),
                epoch: 1,
                file_name: "nda.docx".into(),
                media_type: "application/pdf".into(),
                size: 5,
                created_at_ms: 1_763_337_600_000,
                content_sha256: Sha256::digest(b"hello").into(),
            }),
        }
    }

    const VECTOR_HEADER_HEX: &str = include_str!("sync_crypto_vectors/version_header.hex");
    const VECTOR_SIGNATURE_HEX: &str = include_str!("sync_crypto_vectors/version_signature.hex");

    #[test]
    fn canonical_version_header_matches_its_test_vector() {
        assert_eq!(hex::encode(vector_header().encode()), VECTOR_HEADER_HEX.trim());
    }

    #[test]
    fn signature_over_canonical_header_matches_its_test_vector() {
        let sig = vector_signer().sign(&vector_header().encode());
        assert_eq!(hex::encode(sig.to_bytes()), VECTOR_SIGNATURE_HEX.trim());
    }

    #[test]
    fn canonical_header_decodes_to_the_same_header() {
        let bytes = vector_header().encode();
        assert_eq!(Header::decode(&bytes).unwrap(), vector_header());
    }

    #[test]
    fn member_and_device_headers_decode_to_themselves() {
        let bodies = [
            Body::Member(Member::Approve { device: [0x77; 32], admin: true, invite: Some(Uuid::from_bytes([0x66; 16])) }),
            Body::Member(Member::Approve { device: [0x77; 32], admin: false, invite: None }),
            Body::Member(Member::Invite { link: vec![0x55; 300] }),
            Body::Member(Member::Revoke { invite: Uuid::from_bytes([0x66; 16]) }),
            Body::Request(RequestFields {
                device: DeviceFields { display_name: "Ben Tan".into(), recipient: "age1x".into() },
                link: vec![0x55; 300],
            }),
            Body::Member(Member::Remove { device: [0x77; 32], cutoff: Some([0x88; 32]) }),
            Body::Member(Member::Remove { device: [0x77; 32], cutoff: None }),
            Body::Device(DeviceFields { display_name: "Ben Tan".into(), recipient: "age1x".into() }),
        ];
        for body in bodies {
            let h = Header { body, ..vector_header() };
            assert_eq!(Header::decode(&h.encode()).unwrap(), h);
        }
    }

    #[test]
    fn header_with_trailing_bytes_is_rejected() {
        let mut bytes = vector_header().encode();
        bytes.push(0);
        assert!(matches!(Header::decode(&bytes), Err(RecordError::Malformed("trailing bytes"))));
    }

    #[test]
    fn safety_number_matches_its_test_vector() {
        let joiner = vector_signer().verifying_key().to_bytes();
        assert_eq!(
            safety_number(b"signed-link-bytes", &joiner),
            include_str!("sync_crypto_vectors/safety_number.txt").trim()
        );
    }

    #[test]
    fn safety_number_is_twelve_groups_of_five_digits() {
        let n = safety_number(b"", &[0u8; 32]);
        let groups: Vec<_> = n.split(' ').collect();
        assert!(groups.len() == 12 && groups.iter().all(|g| g.len() == 5 && g.bytes().all(|b| b.is_ascii_digit())));
    }

    fn link(admin: &DeviceKeys) -> Link {
        Link {
            invite: Uuid::from_bytes([0x66; 16]),
            briefcase_id: BRIEFCASE,
            epoch_recipient: "age1x".into(),
            admin: admin.signer(),
            admin_name: "Ana Lim".into(),
            invitee_name: "Ben Tan".into(),
            issued_at_ms: 1_763_337_600_000,
        }
    }

    #[test]
    fn signed_link_verifies_to_the_link_its_admin_signed() {
        let admin = DeviceKeys::generate();
        assert_eq!(Link::verify(&link(&admin).sign(&admin).unwrap()).unwrap(), link(&admin));
    }

    #[test]
    fn link_with_a_changed_byte_fails_its_signature() {
        let admin = DeviceKeys::generate();
        let mut signed = link(&admin).sign(&admin).unwrap();
        signed[LINK_MAGIC.len()] ^= 1;
        assert!(matches!(Link::verify(&signed), Err(RecordError::BadSignature)));
    }

    #[test]
    fn link_naming_an_admin_other_than_its_signer_fails_its_signature() {
        let (admin, other) = (DeviceKeys::generate(), DeviceKeys::generate());
        let mut bytes = link(&admin).encode();
        bytes.extend_from_slice(&other.signing.sign(&bytes).to_bytes());
        assert!(matches!(Link::verify(&bytes), Err(RecordError::BadSignature)));
    }

    #[test]
    fn swapped_link_gives_a_different_safety_number() {
        let joiner = vector_signer().verifying_key().to_bytes();
        assert_ne!(safety_number(b"link-a", &joiner), safety_number(b"link-b", &joiner));
    }

    // ---- the epoch chain ----

    #[test]
    fn member_device_opens_admin_signed_key_file_and_releases_a_version() {
        let (admin, ben) = (DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let key_uuid = Uuid::new_v4();
        let mut key_file = Vec::new();
        seal_key_file(&admin, &key_header(&admin, key_uuid, &epoch), &epoch, &[admin.recipient(), ben.recipient()], &mut key_file).unwrap();
        let m = matter(&[&admin, &ben], &[&admin]);
        let opened = open_key_file(io::Cursor::new(&key_file), key_uuid, &BRIEFCASE, &ben, &m).unwrap();

        let uuid = Uuid::new_v4();
        let blob = seal_version(&admin, &version_header(&admin, uuid, 1, b"v1 text"), b"v1 text", &epoch);
        let r = release(&blob, uuid, &[opened], &m);
        assert_eq!(fs::read(r.dest()).unwrap(), b"v1 text");
    }

    // ---- negative tests: each fails for its own reason ----

    #[test]
    fn key_file_signed_by_a_non_admin_member_is_rejected() {
        let (admin, mallory, ben) = (DeviceKeys::generate(), DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let mut key_file = Vec::new();
        seal_key_file(&mallory, &key_header(&mallory, uuid, &epoch), &epoch, &[ben.recipient()], &mut key_file).unwrap();
        let m = matter(&[&admin, &mallory, &ben], &[&admin]);
        assert!(matches!(open_key_file(io::Cursor::new(&key_file), uuid, &BRIEFCASE, &ben, &m), Err(RecordError::NotAdmin)));
    }

    #[test]
    fn key_file_whose_identity_differs_from_its_signed_key_is_rejected() {
        let (admin, ben) = (DeviceKeys::generate(), DeviceKeys::generate());
        let (signed_epoch, swapped_epoch) = (Epoch::generate(1), Epoch::generate(1));
        let uuid = Uuid::new_v4();
        let mut key_file = Vec::new();
        seal_key_file(&admin, &key_header(&admin, uuid, &signed_epoch), &swapped_epoch, &[ben.recipient()], &mut key_file).unwrap();
        let m = matter(&[&admin, &ben], &[&admin]);
        assert!(matches!(open_key_file(io::Cursor::new(&key_file), uuid, &BRIEFCASE, &ben, &m), Err(RecordError::EpochKeyMismatch)));
    }

    #[test]
    fn version_from_a_non_member_is_rejected_and_not_released() {
        let (ana, outsider) = (DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let blob = seal_version(&outsider, &version_header(&outsider, uuid, 1, b"x"), b"x", &epoch);
        let r = release(&blob, uuid, &[epoch], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::NotMember)) && !r.dest().exists());
    }

    #[test]
    fn removed_device_record_after_its_cutoff_is_rejected() {
        let (ana, ben) = (DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let last = version_header(&ben, Uuid::new_v4(), 4, b"x");
        let mut m = matter(&[&ana], &[&ana]);
        m.removed.insert(ben.signer(), HashSet::from([last.record_hash()]));
        let uuid = Uuid::new_v4();
        let blob = seal_version(&ben, &version_header(&ben, uuid, 5, b"late"), b"late", &epoch);
        assert!(matches!(release(&blob, uuid, &[epoch], &m).result, Err(RecordError::AfterRemovalCutoff)));
    }

    #[test]
    fn removed_device_record_at_its_cutoff_is_still_accepted() {
        let (ana, ben) = (DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let last = version_header(&ben, uuid, 4, b"x");
        let mut m = matter(&[&ana], &[&ana]);
        m.removed.insert(ben.signer(), HashSet::from([last.record_hash()]));
        let blob = seal_version(&ben, &last, b"x", &epoch);
        assert!(release(&blob, uuid, &[epoch], &m).result.is_ok());
    }

    #[test]
    fn different_record_at_a_removed_device_cutoff_seq_is_rejected() {
        let (ana, ben) = (DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let last = version_header(&ben, Uuid::new_v4(), 4, b"x");
        let mut m = matter(&[&ana], &[&ana]);
        m.removed.insert(ben.signer(), HashSet::from([last.record_hash()]));
        let uuid = Uuid::new_v4();
        let blob = seal_version(&ben, &version_header(&ben, uuid, 4, b"forged"), b"forged", &epoch);
        assert!(matches!(release(&blob, uuid, &[epoch], &m).result, Err(RecordError::AfterRemovalCutoff)));
    }

    #[test]
    fn tampered_age_header_fails_decryption() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let mut blob = seal_version(&ana, &version_header(&ana, uuid, 1, b"x"), b"x", &epoch);
        let mac_line = blob.windows(4).position(|w| w == b"--- ").unwrap();
        // Swap one full-weight base64 digit of the header MAC for another valid digit.
        let digit = &mut blob[mac_line + 5];
        *digit = if *digit == b'A' { b'B' } else { b'A' };
        let r = release(&blob, uuid, &[epoch], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::Decrypt(age::DecryptError::InvalidMac))));
    }

    #[test]
    fn signed_field_changed_by_a_member_fails_the_signature() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let header = version_header(&ana, uuid, 1, b"x");
        let blob = seal_version(&ana, &header, b"x", &epoch);
        let mut renamed = header.clone();
        let Body::Version(v) = &mut renamed.body else { unreachable!() };
        v.file_name = "nda-final.docx".into();
        let forged = reencrypt(&blob, &epoch, |plain| {
            let (orig, edit) = (hex::encode(header.encode()), hex::encode(renamed.encode()));
            String::from_utf8(plain).unwrap().replace(&orig, &edit).into_bytes()
        });
        let r = release(&forged, uuid, &[epoch], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::BadSignature)));
    }

    #[test]
    fn content_swapped_by_a_member_is_rejected_and_leaves_no_temp_file() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let blob = seal_version(&ana, &version_header(&ana, uuid, 1, b"real"), b"real", &epoch);
        let forged = reencrypt(&blob, &epoch, |plain| {
            let mut p = plain[..plain.len() - 4].to_vec();
            p.extend_from_slice(b"fake");
            p
        });
        let r = release(&forged, uuid, &[epoch], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::ContentMismatch)) && !r.dest().exists() && r.temp_dir_is_empty());
    }

    #[test]
    fn record_under_a_different_file_name_is_rejected() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let blob = seal_version(&ana, &version_header(&ana, uuid, 1, b"x"), b"x", &epoch);
        let r = release(&blob, Uuid::new_v4(), &[epoch], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::UuidMismatch)));
    }

    #[test]
    fn record_replayed_into_another_matter_is_rejected() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let mut header = version_header(&ana, uuid, 1, b"x");
        header.briefcase_id = [8u8; 32];
        let blob = seal_version(&ana, &header, b"x", &epoch);
        let r = release(&blob, uuid, &[epoch], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::ForeignMatter)));
    }

    #[test]
    fn key_file_presented_as_a_version_is_rejected() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let mut key_file = Vec::new();
        seal_key_file(&ana, &key_header(&ana, uuid, &epoch), &epoch, &[epoch.recipient()], &mut key_file).unwrap();
        let r = release(&key_file, uuid, &[epoch], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::WrongKind)));
    }

    #[test]
    fn version_labelled_with_an_epoch_other_than_the_one_that_opens_it_is_rejected() {
        let ana = DeviceKeys::generate();
        let (epoch1, epoch2) = (Epoch::generate(1), Epoch::generate(2));
        let uuid = Uuid::new_v4();
        let mut header = version_header(&ana, uuid, 1, b"x");
        let Body::Version(v) = &mut header.body else { unreachable!() };
        v.epoch = 2;
        let blob = seal_version(&ana, &header, b"x", &epoch1);
        let r = release(&blob, uuid, &[epoch2, epoch1], &matter(&[&ana], &[&ana]));
        assert!(matches!(r.result, Err(RecordError::WrongEpoch)));
    }

    #[test]
    fn version_opens_with_whichever_held_epoch_it_was_sealed_to() {
        let ana = DeviceKeys::generate();
        let (epoch1, epoch2) = (Epoch::generate(1), Epoch::generate(2));
        let uuid = Uuid::new_v4();
        let blob = seal_version(&ana, &version_header(&ana, uuid, 1, b"x"), b"x", &epoch1);
        assert!(release(&blob, uuid, &[epoch2, epoch1], &matter(&[&ana], &[&ana])).result.is_ok());
    }

    #[test]
    fn leftover_temp_file_from_a_crashed_release_does_not_block_the_next() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let blob = seal_version(&ana, &version_header(&ana, uuid, 1, b"x"), b"x", &epoch);
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(format!("{uuid}.tmp")), b"partial").unwrap();
        let result = release_version(io::Cursor::new(&blob), uuid, &BRIEFCASE, &[epoch], &matter(&[&ana], &[&ana]), dir.path(), &dir.path().join("out"));
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn released_plaintext_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let uuid = Uuid::new_v4();
        let blob = seal_version(&ana, &version_header(&ana, uuid, 1, b"x"), b"x", &epoch);
        let r = release(&blob, uuid, &[epoch], &matter(&[&ana], &[&ana]));
        assert_eq!(fs::metadata(r.dest()).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn sealing_a_header_no_peer_could_decode_is_refused() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let mut header = version_header(&ana, Uuid::new_v4(), 1, b"x");
        let Body::Version(v) = &mut header.body else { unreachable!() };
        v.file_name = "a".repeat(MAX_TEXT_BYTES + 1);
        let result = seal(&ana, &header, &b"x"[..], &[&epoch.recipient()], io::sink());
        assert!(matches!(result, Err(RecordError::Malformed("text length"))));
    }

    #[test]
    fn sealing_a_header_that_names_another_signer_is_refused() {
        let (ana, ben) = (DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let header = version_header(&ben, Uuid::new_v4(), 1, b"x");
        let result = seal(&ana, &header, &b"x"[..], &[&epoch.recipient()], io::sink());
        assert!(matches!(result, Err(RecordError::SignerMismatch)));
    }

    // ---- spike measurements and interop (run with --ignored) ----

    const LARGE_FILE_BYTES: usize = 50 * 1024 * 1024;

    #[test]
    #[ignore = "spike measurement: 50 MB stream; run in --release"]
    fn fifty_megabyte_version_streams_through_seal_and_release() {
        let ana = DeviceKeys::generate();
        let epoch = Epoch::generate(1);
        let dir = tempfile::tempdir().unwrap();
        let (src, blob_path) = (dir.path().join("big.docx"), dir.path().join("big.rumble"));
        // Written in chunks so the measured peak memory is the stream's, not the fixture's.
        let (mut file, mut hasher) = (File::create(&src).unwrap(), Sha256::new());
        let chunk: Vec<u8> = (0..COPY_CHUNK_BYTES).map(|i| (i * 31 % 251) as u8).collect();
        for _ in 0..LARGE_FILE_BYTES / COPY_CHUNK_BYTES {
            hasher.update(&chunk);
            file.write_all(&chunk).unwrap();
        }
        let hash: Hash = hasher.finalize().into();
        let uuid = Uuid::new_v4();
        let mut header = version_header(&ana, uuid, 1, b"");
        let Body::Version(v) = &mut header.body else { unreachable!() };
        (v.size, v.content_sha256) = (LARGE_FILE_BYTES as u64, hash);

        let t = std::time::Instant::now();
        seal(&ana, &header, File::open(&src).unwrap(), &[&epoch.recipient()], File::create(&blob_path).unwrap()).unwrap();
        let sealed = t.elapsed();
        let t = std::time::Instant::now();
        fs::create_dir_all(dir.path().join("tmp")).unwrap();
        release_version(File::open(&blob_path).unwrap(), uuid, &BRIEFCASE, &[epoch], &matter(&[&ana], &[&ana]), &dir.path().join("tmp"), &dir.path().join("out.docx")).unwrap();
        eprintln!("seal {sealed:?}, verify+release {:?}", t.elapsed());
        assert_eq!(fs::metadata(dir.path().join("out.docx")).unwrap().len(), LARGE_FILE_BYTES as u64);
    }

    fn go_age(args: &[&str]) -> Vec<u8> {
        let out = Command::new("age").args(args).output().expect("Go age CLI on PATH");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        out.stdout
    }

    #[test]
    #[ignore = "interop: needs the Go age CLI (`brew install age`) on PATH"]
    fn go_age_decrypts_the_epoch_chain_rust_wrote() {
        let (admin, ben) = (DeviceKeys::generate(), DeviceKeys::generate());
        let epoch = Epoch::generate(1);
        let dir = tempfile::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n).to_str().unwrap().to_owned();
        fs::write(p("device.key"), format!("{}\n", ben.receiving.to_string().expose_secret())).unwrap();
        let mut key_file = Vec::new();
        seal_key_file(&admin, &key_header(&admin, Uuid::new_v4(), &epoch), &epoch, &[ben.recipient()], &mut key_file).unwrap();
        fs::write(p("keyfile"), key_file).unwrap();
        let blob = seal_version(&admin, &version_header(&admin, Uuid::new_v4(), 1, b"v1 text"), b"v1 text", &epoch);
        fs::write(p("blob"), blob).unwrap();

        fs::write(p("epoch.key"), go_age(&["-d", "-i", &p("device.key"), &p("keyfile")])).unwrap();
        let plain = go_age(&["-d", "-i", &p("epoch.key"), &p("blob")]);
        let body = &plain[plain.iter().position(|b| *b == b'\n').unwrap() + 1..];
        assert_eq!(body, b"v1 text");
    }

    #[test]
    #[ignore = "interop: needs the Go age CLI (`brew install age`) on PATH"]
    fn rust_age_decrypts_what_go_age_encrypted_to_an_epoch() {
        let epoch = Epoch::generate(1);
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("plain");
        fs::write(&src, b"from go").unwrap();
        let ct = go_age(&["-r", &epoch.recipient().to_string(), src.to_str().unwrap()]);
        assert_eq!(age::decrypt(&epoch.identity, &ct).unwrap(), b"from go");
    }
}
