# Web UI authentication — required behavior

Status: **core implementation present; release/deployment acceptance still in progress**.
The Authentication screen uses the controls requested from qBittorrent as a functional
reference, with the web app's own components. Code lives in `crates/remote/src/auth.rs`,
`web_auth.rs` and `crates/remote/web/`. See [verification status](web-development.md).

Implemented bounds: username 1–64 characters, password 12–1024 bytes, failure threshold 1–100,
ban duration 1–604800 seconds, idle timeout 60–86400 seconds. Sessions have a 24-hour absolute
lifetime and are invalidated on process restart or credential changes. Progress polling does
not renew inactivity; deliberate browser activity renews it through a CSRF-protected endpoint.

New extension tokens default to `capture` (add URLs/torrents, read capture rules/status, unpair).
Existing tokens retain `control` (download list/actions/queues), never owner administration.
Only password-authenticated owner sessions can manage settings, categories, queues or devices;
network bypass grants basic download workspace access, not owner settings.

## Local recovery

Run as the service user with its data directory, for example:

```sh
sudo -u mdm env MDM_DATA_DIR=/var/lib/mdm misterdownloadmanager-cli web-auth status
sudo -u mdm env MDM_DATA_DIR=/var/lib/mdm misterdownloadmanager-cli web-auth unban
```

Other operations: `disable-bypass`, `reset-proxies` (clear a mistaken proxy trust list), and
`reset-owner` (clear credentials/policy/bans and invalidate browser sessions, then run `pair`
and enroll again). Recovery uses protected local IPC, not an HTTP endpoint. Device tokens are
separate and are not removed by an owner reset. Corrupt authentication storage fails closed;
stop the service and restore a known-good protected backup rather than deleting it blindly.

Trusted proxies must overwrite `X-Forwarded-For`. Resolution walks that chain right-to-left
and chooses the first untrusted hop; missing/malformed/all-trusted chains fail closed. A
trusted proxy is never direct localhost. Untrusted forwarding headers cannot select a client
identity and also disqualify direct-loopback bypass. Reverse-proxy deployment is not yet
validated end to end.

## Settings → Web UI → Authentication

| Control | Initial value | Behavior |
| --- | --- | --- |
| Username | Owner chooses during setup | Show the current name; changing it requires the current password |
| Password | Owner chooses during setup | Masked **Change password** flow with current/new/confirm fields; blank input never silently clears it |
| Bypass authentication for clients on localhost | Off | Explicit owner opt-in for verified direct loopback connections only |
| Bypass authentication for clients in whitelisted IP subnets | Off | Separate opt-in; use only the validated allowlist below |
| IP/subnet whitelist | Empty | Multiline IPv4/IPv6 addresses and CIDRs; one entry per line, with per-entry errors |
| Ban client after consecutive failures | 5 | Count failed username/password logins per effective client IP |
| Ban for | 3600 seconds | Temporary, server-enforced ban; show seconds beside the input |
| Session timeout | 3600 seconds | Server-enforced inactivity timeout; explain that this is idle time |

Numeric controls must accept only positive, bounded integers. Define shared bounds in the
backend contract and use those bounds in the UI; reject zero, negative, fractional, overflow
and out-of-range values instead of interpreting them as “disable protection.” If disabling a
protection is supported later, expose that as a separate, explicit option with a warning.

## Setup, credentials and sessions

- Claim the single owner using a one-time enrollment code obtained through local CLI/container
  logs. Require a chosen username/password before enabling management access. Never ship
  `admin/admin`, an empty password or a public anonymous setup endpoint.
- Hash passwords with a vetted Argon2id implementation, independent random salts and versioned
  parameters. Do not encrypt passwords for later retrieval, log them or expose their hashes.
- Return the username and a password-configured indicator from settings reads, never a reusable
  secret. Use a dedicated credential-change operation rather than a generic settings patch.
- Use server-side, revocable browser sessions with `Secure`, `HttpOnly`, appropriately scoped
  same-site cookies and CSRF protection. Scope cookie paths for supported reverse-proxy prefixes.
- Renew session identifiers after login and sensitive authentication transitions. Enforce idle
  expiry on the server, with an absolute lifetime as an additional bound. Define both in the
  contract and test them using an injectable clock.
- Background polling/SSE/progress traffic must not keep a session alive indefinitely. Renew the
  idle deadline only on deliberate user interaction through a bounded, CSRF-protected activity
  mechanism; avoid claiming this proves human presence.
- Logout revokes the session. Username/password changes revoke existing browser sessions and
  require re-login with the updated credentials. Give clear feedback before applying the change.
- Keep extension device tokens separate from browser passwords/cookies. A browser password
  change does not silently grant/revoke extension scopes; expose device revocation explicitly.
- Preserve the existing low-privilege remote-device boundary: upgrading must not turn an old
  paired token into an owner session.

## Bypass behavior and source-address trust

- Treat localhost and subnet bypass as independent switches. Both are off by default, and an
  empty whitelist matches nobody. Invalid configuration must fail closed.
- Show the list while its switch is off so the owner can prepare it, but label it inactive.
  Canonicalize valid addresses/CIDRs consistently, including IPv4-mapped IPv6 addresses.
- Do not prefill `0.0.0.0/0` from the reference screenshot. This entry, and IPv6 `::/0`, would
  allow every source in the corresponding address family. Show a prominent warning and require
  an explicit owner-confirmed unsafe choice before saving either catch-all.
- Warn that localhost bypass trusts other processes/users on the host, and that subnet bypass
  trusts every reachable device in that subnet. It is not a substitute for network isolation.
- Match the TCP peer directly unless it is an explicitly configured trusted proxy. Accept
  forwarding metadata only from those proxies, with a documented, tested chain-resolution rule.
  The ingress proxy must overwrite untrusted client-supplied forwarding headers.
- A request through a reverse proxy is never eligible for **direct localhost bypass**, even
  when the proxy connects to `127.0.0.1` or `::1`. Evaluate subnet bypass against the verified
  effective client IP, not the proxy address. If that client cannot be verified, require login.
- Docker bridge addresses and RFC1918/private address space are not implicitly trusted. Explain
  that NAT can make several clients appear as one IP, affecting whitelist matches and bans.
- Bypass may grant the normal owner workspace context, but does not remove Host/Origin,
  DNS-rebinding, CSRF or permission checks. It must not authorize credential changes, bypass
  changes, trusted-proxy changes or device enrollment: require actual owner login and recent
  current-password confirmation for those operations.
- Explicit extension/API-token routes continue to require their token and scopes; a network
  bypass setting must not silently elevate an extension or accept anonymous capture requests.

## Failed-login bans and recovery

- Count consecutive credential failures by the verified effective client IP, not by a supplied
  username. Reset that client's failure count after successful login; avoid account-wide bans
  that let an attacker lock out the owner from every address.
- At the configured threshold, deny new logins from that source until the ban expires. Return
  a generic response with appropriate retry timing; do not reveal whether the username exists.
- A banned source does not evade the ban through authentication bypass. Keep ban checks before
  bypass evaluation and make the status visible to an already authenticated owner elsewhere.
- Separate temporary login bans from device-token revocation. Bad/revoked extension tokens
  have their own bounded request-rate protections; they must not increment password failures.
- Bound failure/ban tracking by TTL and entry limits. Persist active bans safely so restarting
  the service does not clear brute-force protection; avoid unbounded per-IP state or disk writes.
- Provide local CLI commands to inspect/clear a ban, reset owner credentials and disable bypass
  when locked out. These are local recovery operations, not anonymous network endpoints.
- Audit authentication-policy changes without logging passwords, session cookies or raw tokens.

## Release acceptance checklist

- [x] All requested controls exist; native process restart persistence is tested. Container
      restart acceptance is still pending Linux validation.
- [x] Owner setup, login, logout, credential changes and CLI recovery work without a desktop.
- [ ] Defaults require login from localhost, private addresses and public addresses alike.
- [ ] Direct IPv4/IPv6 loopback bypass works only when explicitly enabled.
- [ ] Allowed/denied IPv4/IPv6 IPs, CIDRs, invalid lists, empty lists and catch-all confirmation
      behave as documented; IPv4-mapped IPv6 cannot evade matching or bans.
- [ ] An external visitor through a loopback reverse proxy is not accidentally authenticated.
- [ ] Forged forwarding headers, untrusted proxy peers, missing client metadata and ambiguous
      proxy chains cannot obtain bypass or choose the ban-accounting address.
- [ ] The fifth consecutive failure bans that source by default; successful login resets the
      counter; expiry, persistence, concurrent attempts and local unban are tested.
- [ ] Bans cannot lock out the owner globally or be avoided through an enabled bypass rule.
- [x] Idle sessions expire after 3600 seconds by default, including while progress polling
      continues; expired/revoked sessions cannot mutate application state.
- [x] Host/Origin/CSRF checks still apply to bypassed clients; sensitive policy changes require
      current-password confirmation and legacy extension tokens cannot administer the server.
- [ ] Password fields are accessible, support password managers and never display stored
      secrets; validation, unsaved changes, expired sessions and lockout feedback are tested.
