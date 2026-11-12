//! Fork CLI aliases. The desktop entry point still calls headless::asked/main unchanged.

pub(super) fn asked(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--web")
}

pub(super) fn enabled(args: &[String], environment: Option<&str>) -> bool {
    asked(args) || !environment.is_some_and(|value| value.eq_ignore_ascii_case("off"))
}

/// --webhost is the listener address/adapter, not the public URL or allowed HTTP Host list.
/// The new spelling wins over --listen when both are supplied. Keep existing LAN-only
/// listener selection: do not reinterpret wildcard IPs as permission to bind every interface.
pub(super) fn host(args: &[String]) -> Result<Option<String>, &'static str> {
    let mut chosen = None;
    for (index, arg) in args.iter().enumerate() {
        let value = if arg == "--webhost" {
            Some(
                args.get(index + 1)
                    .map(String::as_str)
                    .ok_or("--webhost needs an IP address or adapter name")?,
            )
        } else {
            arg.strip_prefix("--webhost=")
        };
        if let Some(value) = value {
            let value = value.trim();
            if value.is_empty() || value.starts_with('-') || value.contains("://") {
                return Err("--webhost needs an IP address or adapter name, not a URL");
            }
            if value
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_unspecified())
            {
                return Err(
                    "--webhost cannot be 0.0.0.0 or ::; choose a local IP/adapter or omit it for local-network addresses",
                );
            }
            chosen = Some(value.to_owned());
        }
    }
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn web_enables_headless_web_explicitly_without_changing_legacy_off() {
        let flags = args(&["--web", "--port=19000", "--webhost", "127.0.0.1"]);
        assert!(super::super::asked(&flags));
        assert!(enabled(&flags, Some("off")));
        assert!(!enabled(&args(&["--headless"]), Some("off")));
        assert!(super::super::asked(&args(&["--headless"])));
        assert!(!super::super::asked(&args(&["--web=false"])));
        assert_eq!(host(&flags).unwrap().as_deref(), Some("127.0.0.1"));
        assert_eq!(
            super::super::setting(&flags, "port", "MDM_TEST_UNSET_PORT").as_deref(),
            Some("19000")
        );
    }

    #[test]
    fn host_alias_handles_equal_forms_ipv6_and_missing_values() {
        assert_eq!(
            host(&args(&["--webhost=[::1]"])).unwrap().as_deref(),
            Some("[::1]")
        );
        assert_eq!(
            host(&args(&["--listen=127.0.0.2", "--webhost=127.0.0.1"]))
                .unwrap()
                .as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(
            host(&args(&["--webhost=eth0"])).unwrap().as_deref(),
            Some("eth0")
        );
        assert_eq!(host(&args(&["--listen=127.0.0.1"])).unwrap(), None);
        for flags in [
            &["--webhost"][..],
            &["--webhost="],
            &["--webhost", "--port", "9000"],
            &["--webhost=https://nas/"],
            &["--webhost=0.0.0.0"],
            &["--webhost=::"],
        ] {
            assert!(host(&args(flags)).is_err(), "{flags:?}");
        }
    }
}
