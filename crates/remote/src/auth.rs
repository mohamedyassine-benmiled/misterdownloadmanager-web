//! Single-owner browser authentication. Device bearer tokens remain a separate boundary.
//! Passwords and sessions never appear in settings responses; corrupt state fails closed.

use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::path::PathBuf;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use sha2::Digest;

const MAX_CLIENTS: usize = 4096;
const MAX_SESSIONS: usize = 128;
const ABSOLUTE_SESSION: i64 = 86400;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub bypass_localhost: bool,
    pub bypass_subnets: bool,
    pub subnets: Vec<String>,
    pub trusted_proxies: Vec<String>,
    pub max_failures: u32,
    pub ban_seconds: u64,
    pub session_timeout: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            bypass_localhost: false,
            bypass_subnets: false,
            subnets: Vec::new(),
            trusted_proxies: Vec::new(),
            max_failures: 5,
            ban_seconds: 3600,
            session_timeout: 3600,
        }
    }
}

impl Policy {
    pub fn validate(&mut self, confirm_unsafe: bool) -> Result<(), Error> {
        if !(1..=100).contains(&self.max_failures)
            || !(1..=604800).contains(&self.ban_seconds)
            || !(60..=86400).contains(&self.session_timeout)
        {
            return Err(Error::InvalidPolicy);
        }
        for entries in [&mut self.subnets, &mut self.trusted_proxies] {
            if entries.len() > 100 {
                return Err(Error::InvalidPolicy);
            }
            for entry in entries.iter_mut() {
                let net = network(entry).ok_or(Error::InvalidNetwork)?;
                if net.prefix_len() == 0 && !confirm_unsafe {
                    return Err(Error::UnsafeNetwork);
                }
                *entry = net.trunc().to_string();
            }
            entries.sort();
            entries.dedup();
        }
        Ok(())
    }
}

fn network(text: &str) -> Option<IpNet> {
    if text.len() > 64 {
        return None;
    }
    let net = text
        .trim()
        .parse::<IpNet>()
        .ok()
        .or_else(|| text.trim().parse::<IpAddr>().ok().map(IpNet::from))?;
    // A mapped address is matched in the IPv4 namespace, not an alternate ban identity.
    if let IpNet::V6(v6) = net
        && let Some(v4) = v6.addr().to_ipv4_mapped()
    {
        if v6.prefix_len() < 96 {
            return None;
        }
        return ipnet::Ipv4Net::new(v4, v6.prefix_len() - 96)
            .ok()
            .map(IpNet::V4);
    }
    Some(net)
}

fn normalize(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        _ => ip,
    }
}

fn includes(entries: &[String], ip: IpAddr) -> bool {
    entries
        .iter()
        .filter_map(|n| network(n))
        .any(|n| n.contains(&normalize(ip)))
}

#[derive(Clone, Copy, Debug)]
pub struct Client {
    pub ip: IpAddr,
    pub direct_loopback: bool,
}

impl Client {
    /// Only explicitly trusted proxies may supply X-Forwarded-For. Resolve right-to-left,
    /// stopping at the first untrusted hop; untrusted client-supplied prefixes are ignored.
    pub fn resolve(
        peer: IpAddr,
        forwarded: Option<&str>,
        has_forwarding: bool,
        policy: &Policy,
    ) -> Result<Self, Error> {
        let peer = normalize(peer);
        if !includes(&policy.trusted_proxies, peer) {
            return Ok(Self {
                ip: peer,
                direct_loopback: peer.is_loopback() && !has_forwarding,
            });
        }
        let chain = forwarded
            .filter(|s| s.len() <= 2048)
            .ok_or(Error::InvalidClient)?;
        let ips = chain
            .split(',')
            .map(|s| s.trim().parse::<IpAddr>().map(normalize))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidClient)?;
        if ips.is_empty() || ips.len() > 16 {
            return Err(Error::InvalidClient);
        }
        let ip = ips
            .into_iter()
            .rev()
            .find(|ip| !includes(&policy.trusted_proxies, *ip))
            .ok_or(Error::InvalidClient)?;
        Ok(Self {
            ip,
            direct_loopback: false,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unauthorized,
    Forbidden,
    Banned(i64),
    InvalidCredentials,
    InvalidPolicy,
    InvalidNetwork,
    UnsafeNetwork,
    InvalidClient,
    AlreadyConfigured,
    NotConfigured,
    Unavailable,
    Busy,
}

impl Error {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unauthorized => "login-required",
            Self::Forbidden => "owner-login-required",
            Self::Banned(_) => "temporarily-banned",
            Self::InvalidCredentials => "invalid-credentials",
            Self::InvalidPolicy => "invalid-auth-policy",
            Self::InvalidNetwork => "invalid-subnet",
            Self::UnsafeNetwork => "confirm-unsafe-subnet",
            Self::InvalidClient => "unverified-client",
            Self::AlreadyConfigured => "already-configured",
            Self::NotConfigured => "setup-required",
            Self::Unavailable => "auth-storage-unavailable",
            Self::Busy => "auth-busy",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Stored {
    version: u32,
    revision: u64,
    username: String,
    password_hash: Option<String>,
    policy: Policy,
    bans: BTreeMap<String, i64>,
}
impl Default for Stored {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            username: String::new(),
            password_hash: None,
            policy: Policy::default(),
            bans: BTreeMap::new(),
        }
    }
}

struct Session {
    csrf: String,
    created: i64,
    active: i64,
    password: bool,
    ip: IpAddr,
}

#[derive(Clone)]
pub struct Identity {
    pub key: String,
    pub csrf: String,
    pub password: bool,
}

#[derive(Serialize)]
pub struct View {
    pub configured: bool,
    pub authenticated: bool,
    pub owner: bool,
    pub username: String,
    pub csrf: Option<String>,
}

pub struct Auth {
    path: PathBuf,
    stored: Stored,
    healthy: bool,
    sessions: HashMap<String, Session>,
    failures: HashMap<IpAddr, (u32, i64)>,
}

fn digest(text: &str) -> String {
    hex::encode(sha2::Sha256::digest(text.as_bytes()))
}
fn token() -> String {
    hex::encode(crate::devices::random(32))
}
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0, |acc, (a, b)| acc | (a ^ b))
            == 0
}

impl Auth {
    pub fn load(path: PathBuf) -> Self {
        let loaded = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Stored>(&bytes).map_err(|_| ()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Stored::default()),
            Err(_) => Err(()),
        };
        let (mut stored, mut healthy) = loaded
            .map(|s| (s, true))
            .unwrap_or_else(|_| (Stored::default(), false));
        healthy &= stored.version == 1
            && stored.policy.validate(true).is_ok()
            && stored.bans.len() <= MAX_CLIENTS
            && stored
                .password_hash
                .as_ref()
                .is_none_or(|h| PasswordHash::new(h).is_ok());
        Self {
            path,
            stored,
            healthy,
            sessions: HashMap::new(),
            failures: HashMap::new(),
        }
    }
    fn ready(&self) -> Result<(), Error> {
        if self.healthy {
            Ok(())
        } else {
            Err(Error::Unavailable)
        }
    }
    fn commit(&mut self, mut stored: Stored) -> Result<(), Error> {
        stored.revision += 1;
        let bytes = serde_json::to_vec_pretty(&stored).map_err(|_| Error::Unavailable)?;
        mdm_ipc::private_dir(self.path.parent().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        mdm_ipc::private_write(&self.path, &bytes).map_err(|_| Error::Unavailable)?;
        self.stored = stored;
        Ok(())
    }
    pub fn policy(&self) -> Result<&Policy, Error> {
        self.ready()?;
        Ok(&self.stored.policy)
    }
    pub fn configured(&self) -> Result<bool, Error> {
        self.ready()?;
        Ok(self.stored.password_hash.is_some())
    }
    pub fn banned(&self, client: Client, now: i64) -> Result<(), Error> {
        self.ready()?;
        if let Some(until) = self
            .stored
            .bans
            .get(&client.ip.to_string())
            .filter(|until| **until > now)
        {
            return Err(Error::Banned(*until - now));
        }
        Ok(())
    }
    fn bypass(&self, client: Client) -> bool {
        self.stored.policy.bypass_localhost && client.direct_loopback
            || self.stored.policy.bypass_subnets && includes(&self.stored.policy.subnets, client.ip)
    }
    fn prune(&mut self, now: i64) {
        let idle = self.stored.policy.session_timeout as i64;
        self.sessions.retain(|_, s| {
            now >= s.created && now - s.created < ABSOLUTE_SESSION && now - s.active < idle
        });
        self.failures.retain(|_, (_, at)| now - *at < 3600);
    }
    fn issue(
        &mut self,
        password: bool,
        client: Client,
        now: i64,
    ) -> Result<(String, Identity), Error> {
        self.prune(now);
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(Error::Busy);
        }
        let raw = token();
        let key = digest(&raw);
        let csrf = token();
        self.sessions.insert(
            key.clone(),
            Session {
                csrf: csrf.clone(),
                created: now,
                active: now,
                password,
                ip: client.ip,
            },
        );
        Ok((
            raw,
            Identity {
                key,
                csrf,
                password,
            },
        ))
    }
    pub(crate) fn validate_credentials(username: &str, password: &str) -> Result<(), Error> {
        if username.trim().is_empty()
            || username.chars().count() > 64
            || username.chars().any(char::is_control)
            || password.len() < 12
            || password.len() > 1024
        {
            return Err(Error::InvalidCredentials);
        }
        Ok(())
    }
    pub fn enroll(
        &mut self,
        username: &str,
        password: &str,
        client: Client,
        now: i64,
    ) -> Result<(String, Identity), Error> {
        if self.configured()? {
            return Err(Error::AlreadyConfigured);
        }
        self.banned(client, now)?;
        Self::validate_credentials(username, password)?;
        let salt =
            SaltString::encode_b64(&crate::devices::random(16)).map_err(|_| Error::Unavailable)?;
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| Error::Unavailable)?
            .to_string();
        let mut stored = self.stored.clone();
        stored.username = username.trim().into();
        stored.password_hash = Some(hash);
        self.commit(stored)?;
        self.issue(true, client, now)
    }
    fn password_matches(&self, username: &str, password: &str) -> bool {
        let Some(hash) = self
            .stored
            .password_hash
            .as_ref()
            .and_then(|h| PasswordHash::new(h).ok())
        else {
            return false;
        };
        let valid = password.len() <= 1024
            && Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok();
        valid && same(username.trim(), &self.stored.username)
    }
    fn failed(&mut self, client: Client, now: i64) -> Result<(), Error> {
        self.prune(now);
        if self.failures.len() >= MAX_CLIENTS && !self.failures.contains_key(&client.ip) {
            return Err(Error::Busy);
        }
        let f = self.failures.entry(client.ip).or_insert((0, now));
        f.0 += 1;
        f.1 = now;
        if f.0 >= self.stored.policy.max_failures {
            let mut stored = self.stored.clone();
            stored.bans.retain(|_, until| *until > now);
            if stored.bans.len() >= MAX_CLIENTS && !stored.bans.contains_key(&client.ip.to_string())
            {
                return Err(Error::Busy);
            }
            stored.bans.insert(
                client.ip.to_string(),
                now + self.stored.policy.ban_seconds as i64,
            );
            self.commit(stored)?;
            self.failures.remove(&client.ip);
            return Err(Error::Banned(self.stored.policy.ban_seconds as i64));
        }
        Err(Error::InvalidCredentials)
    }
    pub fn login(
        &mut self,
        username: &str,
        password: &str,
        client: Client,
        now: i64,
    ) -> Result<(String, Identity), Error> {
        self.banned(client, now)?;
        if !self.configured()? {
            return Err(Error::NotConfigured);
        }
        if !self.password_matches(username, password) {
            self.failed(client, now)?;
        }
        self.failures.remove(&client.ip);
        self.issue(true, client, now)
    }
    pub fn authenticate(
        &mut self,
        raw: Option<&str>,
        client: Client,
        now: i64,
    ) -> Result<Identity, Error> {
        self.banned(client, now)?;
        self.prune(now);
        let key = digest(raw.filter(|r| r.len() == 64).ok_or(Error::Unauthorized)?);
        let s = self.sessions.get(&key).ok_or(Error::Unauthorized)?;
        if !s.password && (s.ip != client.ip || !self.bypass(client)) {
            return Err(Error::Unauthorized);
        }
        Ok(Identity {
            key,
            csrf: s.csrf.clone(),
            password: s.password,
        })
    }
    pub fn session(
        &mut self,
        raw: Option<&str>,
        client: Client,
        now: i64,
    ) -> Result<(View, Option<String>), Error> {
        self.banned(client, now)?;
        let configured = self.configured()?;
        let existing = self.authenticate(raw, client, now).ok();
        let (identity, cookie) = if let Some(id) = existing {
            (Some(id), None)
        } else if configured && self.bypass(client) {
            let (raw, id) = self.issue(false, client, now)?;
            (Some(id), Some(raw))
        } else {
            (None, None)
        };
        Ok((
            View {
                configured,
                authenticated: identity.is_some(),
                owner: identity.as_ref().is_some_and(|i| i.password),
                username: if identity.is_some() {
                    self.stored.username.clone()
                } else {
                    String::new()
                },
                csrf: identity.map(|i| i.csrf),
            },
            cookie,
        ))
    }
    pub fn activity(&mut self, id: &Identity, now: i64) {
        if let Some(s) = self.sessions.get_mut(&id.key) {
            s.active = now;
        }
    }
    pub fn logout(&mut self, id: &Identity) {
        self.sessions.remove(&id.key);
    }
    pub fn settings(&self) -> Result<serde_json::Value, Error> {
        self.ready()?;
        Ok(
            serde_json::json!({ "username": self.stored.username, "policy": self.stored.policy, "revision": self.stored.revision, "bans": self.stored.bans }),
        )
    }
    pub fn confirm(
        &mut self,
        id: &Identity,
        password: &str,
        client: Client,
        now: i64,
    ) -> Result<(), Error> {
        self.banned(client, now)?;
        self.prune(now);
        if !id.password || !self.sessions.get(&id.key).is_some_and(|s| s.password) {
            return Err(Error::Forbidden);
        }
        if !self.password_matches(&self.stored.username, password) {
            self.failed(client, now)?;
        }
        self.failures.remove(&client.ip);
        Ok(())
    }
    pub fn save_policy(
        &mut self,
        mut policy: Policy,
        confirm_unsafe: bool,
        revision: u64,
    ) -> Result<(), Error> {
        self.ready()?;
        if revision != self.stored.revision {
            return Err(Error::InvalidPolicy);
        }
        policy.validate(confirm_unsafe)?;
        let mut stored = self.stored.clone();
        stored.policy = policy;
        self.commit(stored)?;
        self.sessions.retain(|_, s| s.password);
        Ok(())
    }
    pub fn credentials(&mut self, username: &str, password: &str) -> Result<(), Error> {
        Self::validate_credentials(username, password)?;
        let salt =
            SaltString::encode_b64(&crate::devices::random(16)).map_err(|_| Error::Unavailable)?;
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| Error::Unavailable)?
            .to_string();
        let mut stored = self.stored.clone();
        stored.username = username.trim().into();
        stored.password_hash = Some(hash);
        self.commit(stored)?;
        self.sessions.clear();
        Ok(())
    }
    pub fn recover(&mut self, operation: &str) -> Result<serde_json::Value, Error> {
        self.ready()?;
        if operation == "status" {
            return self.settings();
        }
        let mut stored = self.stored.clone();
        match operation {
            "unban" => {
                stored.bans.clear();
                self.failures.clear();
            }
            "disable-bypass" => {
                stored.policy.bypass_localhost = false;
                stored.policy.bypass_subnets = false;
            }
            "reset-proxies" => {
                stored.policy.trusted_proxies.clear();
            }
            "reset-owner" => {
                stored.password_hash = None;
                stored.username.clear();
                stored.policy = Policy::default();
                stored.bans.clear();
                self.failures.clear();
            }
            _ => return Err(Error::InvalidPolicy),
        }
        self.commit(stored)?;
        self.sessions.clear();
        Ok(serde_json::json!({ "ok": true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn client(ip: &str) -> Client {
        Client {
            ip: ip.parse().unwrap(),
            direct_loopback: false,
        }
    }
    #[test]
    fn networks_and_proxy_identity() {
        let mut p = Policy {
            subnets: vec!["0.0.0.0/0".into()],
            ..Policy::default()
        };
        assert_eq!(p.validate(false), Err(Error::UnsafeNetwork));
        assert!(p.validate(true).is_ok());
        p.trusted_proxies = vec!["127.0.0.1/32".into()];
        let c = Client::resolve(
            "127.0.0.1".parse().unwrap(),
            Some("1.2.3.4, 10.0.0.2"),
            true,
            &p,
        )
        .unwrap();
        assert_eq!(c.ip, "10.0.0.2".parse::<IpAddr>().unwrap());
        assert!(!c.direct_loopback);
        assert!(Client::resolve("127.0.0.1".parse().unwrap(), None, false, &p).is_err());
        let c = Client::resolve("10.0.0.2".parse().unwrap(), Some("127.0.0.1"), true, &p).unwrap();
        assert_eq!(c.ip, "10.0.0.2".parse::<IpAddr>().unwrap());
        assert!(includes(
            &["192.168.1.0/24".into()],
            "::ffff:192.168.1.2".parse().unwrap()
        ));
        assert_eq!(
            network("::ffff:192.168.1.0/120").unwrap().to_string(),
            "192.168.1.0/24"
        );
        assert_eq!(network("::/0").unwrap().prefix_len(), 0);
    }
    #[test]
    fn owner_sessions_bans_and_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let mut a = Auth::load(path.clone());
        let c = client("10.0.0.2");
        let now = 10000;
        assert!(!a.session(None, c, now).unwrap().0.authenticated);
        let (raw, id) = a
            .enroll("owner", "a sufficiently long password", c, now)
            .unwrap();
        assert!(id.password);
        assert!(a.authenticate(Some(&raw), c, now + 3599).is_ok());
        assert!(a.authenticate(Some(&raw), c, now + 3600).is_err());
        // Polling/authentication does not renew the idle deadline.
        let (raw, id) = a
            .login("owner", "a sufficiently long password", c, now + 4000)
            .unwrap();
        a.activity(&id, now + 4500);
        assert!(a.authenticate(Some(&raw), c, now + 8000).is_ok());
        a.logout(&id);
        assert!(a.authenticate(Some(&raw), c, now + 8001).is_err());
        for _ in 0..4 {
            assert!(matches!(
                a.login("bad", "incorrect", c, now + 9000),
                Err(Error::InvalidCredentials)
            ));
        }
        assert!(matches!(
            a.login("owner", "incorrect", c, now + 9000),
            Err(Error::Banned(3600))
        ));
        let mut a = Auth::load(path.clone());
        assert!(matches!(
            a.login("owner", "a sufficiently long password", c, now + 9001),
            Err(Error::Banned(_))
        ));
        assert!(
            a.login(
                "owner",
                "a sufficiently long password",
                client("10.0.0.3"),
                now + 9001
            )
            .is_ok()
        );
        assert!(
            a.login("owner", "a sufficiently long password", c, now + 12600)
                .is_ok()
        );
        let saved = std::fs::read_to_string(path).unwrap();
        assert!(!saved.contains("sufficiently"));
        assert!(!saved.contains(&raw));
    }
    #[test]
    fn bypass_is_not_password_authority() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Auth::load(dir.path().join("a.json"));
        let c = client("10.0.0.2");
        a.enroll("owner", "a sufficiently long password", c, 10000)
            .unwrap();
        let p = Policy {
            bypass_subnets: true,
            subnets: vec!["10.0.0.0/24".into()],
            ..Policy::default()
        };
        a.save_policy(p, false, a.stored.revision).unwrap();
        let (_, raw) = a.session(None, c, 10001).unwrap();
        let id = a.authenticate(raw.as_deref(), c, 10002).unwrap();
        assert!(!id.password);
        assert_eq!(
            a.confirm(&id, "a sufficiently long password", c, 10002),
            Err(Error::Forbidden)
        );
        assert!(
            a.authenticate(raw.as_deref(), client("10.0.0.3"), 10002)
                .is_err()
        );
        a.recover("disable-bypass").unwrap();
        assert!(a.authenticate(raw.as_deref(), c, 10003).is_err());
    }
    #[test]
    fn corrupt_storage_never_reopens_setup() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.json");
        std::fs::write(&p, b"broken").unwrap();
        let a = Auth::load(p);
        assert_eq!(a.configured(), Err(Error::Unavailable));
    }

    #[test]
    fn policy_bounds_and_proxy_chains_fail_closed() {
        let defaults = serde_json::to_value(Policy::default()).unwrap();
        for (key, invalid) in [
            ("max_failures", 0),
            ("max_failures", 101),
            ("ban_seconds", 0),
            ("ban_seconds", 604801),
            ("session_timeout", 59),
            ("session_timeout", 86401),
        ] {
            let mut value = defaults.clone();
            value[key] = invalid.into();
            let mut policy: Policy = serde_json::from_value(value).unwrap();
            assert_eq!(policy.validate(false), Err(Error::InvalidPolicy), "{key}");
        }
        for entry in ["bad", "192.168.1.0/33", "::/129", "::ffff:192.168.1.0/95"] {
            assert!(network(entry).is_none(), "{entry}");
        }
        let policy = Policy {
            trusted_proxies: vec!["127.0.0.1/32".into(), "10.0.0.0/24".into()],
            ..Policy::default()
        };
        let peer = "127.0.0.1".parse().unwrap();
        // A forged prefix on the left cannot choose the effective client identity.
        assert_eq!(
            Client::resolve(
                peer,
                Some("127.0.0.1, 203.0.113.9, 10.0.0.2"),
                true,
                &policy
            )
            .unwrap()
            .ip,
            "203.0.113.9".parse::<IpAddr>().unwrap()
        );
        for chain in [None, Some(""), Some("bad"), Some("127.0.0.1, 10.0.0.2")] {
            assert!(Client::resolve(peer, chain, true, &policy).is_err());
        }
    }

    #[test]
    fn success_resets_failures_absolute_expiry_and_recovery_revoke_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Auth::load(dir.path().join("auth.json"));
        let c = client("10.0.0.2");
        let password = "a sufficiently long password";
        a.enroll("owner", password, c, 10000).unwrap();
        for _ in 0..4 {
            assert!(matches!(
                a.login("owner", "wrong", c, 10001),
                Err(Error::InvalidCredentials)
            ));
        }
        let (raw, id) = a.login("owner", password, c, 10002).unwrap();
        assert!(matches!(
            a.login("owner", "wrong", c, 10003),
            Err(Error::InvalidCredentials)
        ));
        for offset in (1000..86400).step_by(1000) {
            assert!(a.authenticate(Some(&raw), c, 10002 + offset).is_ok());
            a.activity(&id, 10002 + offset);
        }
        assert!(matches!(
            a.authenticate(Some(&raw), c, 10002 + 86400),
            Err(Error::Unauthorized)
        ));
        let (raw, _) = a.login("owner", password, c, 100000).unwrap();
        let policy = Policy {
            trusted_proxies: vec!["127.0.0.1/32".into()],
            ..Policy::default()
        };
        a.save_policy(policy, false, a.stored.revision).unwrap();
        a.recover("reset-proxies").unwrap();
        assert!(a.policy().unwrap().trusted_proxies.is_empty());
        assert!(matches!(
            a.authenticate(Some(&raw), c, 100001),
            Err(Error::Unauthorized)
        ));
    }
}
