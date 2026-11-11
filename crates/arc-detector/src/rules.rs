use crate::{
    budget::{Budget, Checked},
    Classification, JobClass as C, Reason as R,
};

// Only native argv uses this recognizer; shell classification stays protocol-1
// compatible. Unknown arity and option terminators are deliberately unresolved.
pub(crate) fn recognize_bun_test(words: &[String], b: &mut Budget) -> Checked<Classification> {
    let unresolved = || Classification::unknown(R::UnresolvedCommand);
    let mut index = 0;
    let (mut junit, mut reporter_outfile) = (false, false);
    while let Some(word) = words.get(index) {
        b.charge(50 + 4 * word.len())?;
        if word.is_empty() || word == "--" {
            return Ok(unresolved());
        }
        if !word.starts_with('-') {
            index += 1;
            continue;
        }
        let (option, assigned) = word
            .split_once('=')
            .map_or((word.as_str(), None), |(key, value)| (key, Some(value)));
        let numeric = matches!(
            option,
            "--timeout"
                | "--rerun-each"
                | "--retry"
                | "--seed"
                | "--max-concurrency"
                | "--parallel-delay"
        );
        let value_option = numeric
            || matches!(
                option,
                "-t" | "--test-name-pattern"
                    | "--coverage-reporter"
                    | "--coverage-dir"
                    | "--reporter"
                    | "--reporter-outfile"
                    | "--path-ignore-patterns"
                    | "--shard"
                    | "--timings"
            );
        let flag = matches!(
            option,
            "-u" | "--update-snapshots"
                | "--todo"
                | "--only"
                | "--pass-with-no-tests"
                | "--concurrent"
                | "--randomize"
                | "--coverage"
                | "--dots"
                | "--only-failures"
                | "--isolate"
                | "--no-isolate"
                | "--update-timings"
                | "--no-orphans"
                | "--check"
                | "--smol"
        );
        if flag && assigned.is_none() {
            index += 1;
            continue;
        }
        if !value_option {
            return Ok(unresolved());
        }
        if option == "-t" && assigned.is_some() {
            return Ok(unresolved());
        }
        let value = match assigned {
            Some(value) => value,
            None => {
                index += 1;
                let Some(value) = words.get(index) else {
                    return Ok(unresolved());
                };
                b.charge(1 + 4 * value.len())?;
                value.as_str()
            }
        };
        if value.is_empty()
            || value.starts_with('-')
            || (numeric
                && (!value.bytes().all(|c| c.is_ascii_digit()) || value.parse::<u64>().is_err()))
            || (option == "--coverage-reporter" && !matches!(value, "text" | "lcov"))
            || (option == "--reporter" && !matches!(value, "junit" | "dots"))
            || (option == "--shard" && !valid_shard(value))
        {
            return Ok(unresolved());
        }
        junit |= option == "--reporter" && value == "junit";
        reporter_outfile |= option == "--reporter-outfile";
        index += 1;
    }
    if junit && !reporter_outfile {
        return Ok(unresolved());
    }
    Ok(Classification::new(C::Test, R::BunTest))
}

fn valid_shard(value: &str) -> bool {
    let Some((part, total)) = value.split_once('/') else {
        return false;
    };
    if !part.bytes().all(|b| b.is_ascii_digit()) || !total.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let (Ok(part), Ok(total)) = (part.parse::<u32>(), total.parse::<u32>()) else {
        return false;
    };
    part > 0 && part <= total
}

pub(crate) fn recognize(words: &[String], b: &mut Budget) -> Checked<Classification> {
    let head = &words[0];
    if b.eq(head, "cargo")? {
        if let Some(sub) = words.get(1) {
            for (name, class, reason) in [
                ("build", C::Build, R::CargoBuild),
                ("test", C::Test, R::CargoTest),
                ("clippy", C::Lint, R::CargoClippy),
                ("fmt", C::Lint, R::CargoFmt),
            ] {
                if b.eq(sub, name)? {
                    return Ok(Classification::new(class, reason));
                }
            }
        }
    } else if b.eq(head, "gh")? {
        if words.len() >= 3 {
            for (first, second, reason) in [
                ("workflow", "run", R::GhWorkflowRun),
                ("run", "watch", R::GhRunWatch),
                ("run", "view", R::GhRunView),
            ] {
                if b.eq(&words[1], first)? && b.eq(&words[2], second)? {
                    return Ok(Classification::new(C::RemoteCi, reason));
                }
            }
        }
    } else if b.eq(head, "ssh")? {
        return ssh(&words[1..], b);
    } else if b.one_of(head, &["docker", "docker-compose"])? {
        return Ok(Classification::unknown(R::DockerEndpointUnknown));
    }
    // Charge suffix comparison before using the library; bounded linear check.
    b.charge(1 + head.len().min(3))?;
    let script_suffix = head.ends_with(".sh");
    let script = script_suffix
        || b.one_of(head, &["make", "just", "task"])?
        || (b.one_of(head, &["npm", "bun", "pnpm"])?
            && words.len() >= 2
            && b.eq(&words[1], "run")?);
    Ok(Classification::unknown(if script {
        R::UnresolvedScript
    } else {
        R::UnresolvedCommand
    }))
}

fn ssh(words: &[String], b: &mut Budget) -> Checked<Classification> {
    let unknown = |reason| Ok(Classification::unknown(reason));
    let mut index = 0;
    if let Some(first) = words.first() {
        if b.eq(first, "--")? {
            index = 1;
        } else if ssh_forwarding(words, b)? {
            return unknown(R::SshForwarding);
        }
    }
    let Some(destination) = words.get(index) else {
        return unknown(R::SshAmbiguous);
    };
    if !valid_destination(destination, b)? {
        return unknown(R::SshAmbiguous);
    }
    let mut command = &words[index + 1..];
    // Empty words cannot start a remote command or hide the option boundary.
    while let Some(first) = command.first() {
        b.charge(2)?; // token visit and empty-length check
        if !first.is_empty() {
            break;
        }
        command = &command[1..];
    }
    if let Some(first) = command.first() {
        if b.eq(first, "--")? {
            command = &command[1..];
        } else if index == 0 {
            b.charge(2)?; // length and leading option-byte inspection
            if first.starts_with('-') {
                return unknown(R::SshAmbiguous);
            }
        }
    }
    // Once the command starts, its remaining arguments are opaque remote data.
    let mut nonempty = false;
    for word in command {
        b.charge(2)?;
        if !word.is_empty() {
            nonempty = true;
        }
    }
    if !nonempty {
        return unknown(R::SshAmbiguous);
    }
    Ok(Classification::new(C::RemoteExec, R::SshRemoteCommand))
}

fn ssh_forwarding(words: &[String], b: &mut Budget) -> Checked<bool> {
    let mut index = 0;
    while let Some(word) = words.get(index) {
        b.charge(1)?; // option token visit
        if b.eq(word, "--")? {
            break;
        }
        b.charge(1 + word.len())?; // prefix and every possible option-byte inspection
        if !word.starts_with('-') {
            break;
        }
        for (offset, byte) in word.bytes().enumerate().skip(1) {
            match byte {
                b'L' | b'R' | b'D' | b'W' | b'N' => return Ok(true),
                // Known switches without values permit later flags in a cluster.
                b'4' | b'6' | b'A' | b'a' | b'C' | b'f' | b'G' | b'g' | b'K' | b'k' | b'M'
                | b'n' | b'q' | b's' | b'T' | b't' | b'V' | b'v' | b'X' | b'x' | b'Y' | b'y' => {}
                // Values are opaque, attached or in the following token. In particular,
                // letters in usernames and -o settings are never forwarding flags.
                b'B' | b'b' | b'c' | b'E' | b'e' | b'F' | b'I' | b'i' | b'J' | b'l' | b'm'
                | b'O' | b'o' | b'P' | b'p' | b'Q' | b'S' | b'w' => {
                    if offset + 1 == word.len() {
                        b.charge(1)?; // opaque value token visit
                        index += 1;
                    }
                    break;
                }
                // An unknown option's arity is unresolved: do not guess where its
                // value ends, or reinterpret any remaining bytes as flags.
                _ => return Ok(false),
            }
        }
        index += 1;
    }
    Ok(false)
}

fn valid_destination(destination: &str, b: &mut Budget) -> Checked<bool> {
    // Precharge the complete scan for each library operation, even on early exit.
    b.charge(destination.len() + 1)?;
    let (user, host) = match destination.split_once('@') {
        Some((user, host)) => (Some(user), host),
        None => (None, destination),
    };
    if let Some(user) = user {
        b.charge(1 + user.len())?;
        if user.is_empty()
            || !user
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
        {
            return Ok(false);
        }
    }
    b.charge(2 + host.len() * 2)?;
    if host.is_empty() {
        return Ok(false);
    }
    let lower = host.to_ascii_lowercase();
    b.charge(1 + lower.len().min(10))?;
    if b.eq(&lower, "localhost")? || lower.ends_with(".localhost") {
        return Ok(false);
    }
    b.charge(1 + lower.len().min(4))?;
    if lower.starts_with("127.") || b.one_of(&lower, &["::1", "[::1]"])? {
        return Ok(false);
    }
    b.charge(2)?;
    if host.starts_with('[') {
        b.charge(1)?;
        if !host.ends_with(']') {
            return Ok(false);
        }
        let inner = &host[1..host.len() - 1];
        return ipv6(inner, b);
    }
    b.charge(host.len() + 1)?;
    Ok(host.as_bytes()[0].is_ascii_alphanumeric()
        && host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-')))
}

/// Numeric IPv6 parsing only; each input inspection is charged, with no resolver API.
fn ipv6(inner: &str, b: &mut Budget) -> Checked<bool> {
    let bytes = inner.as_bytes();
    b.charge(1)?;
    if bytes.is_empty() {
        return Ok(false);
    }
    let mut groups = [0u16; 8];
    let (mut pos, mut count, mut gap) = (0, 0, None);
    b.charge(1)?;
    if bytes[0] == b':' {
        b.charge(1)?;
        if bytes.get(1) != Some(&b':') {
            return Ok(false);
        }
        gap = Some(0);
        pos = 2;
    }
    while pos < bytes.len() {
        if count == 8 {
            return Ok(false);
        }
        let (mut value, mut digits) = (0, 0);
        while pos < bytes.len() {
            b.charge(1)?;
            let ch = bytes[pos];
            if ch == b':' {
                break;
            }
            let digit = match ch {
                b'0'..=b'9' => ch - b'0',
                b'a'..=b'f' => ch - b'a' + 10,
                b'A'..=b'F' => ch - b'A' + 10,
                _ => return Ok(false),
            };
            digits += 1;
            if digits > 4 {
                return Ok(false);
            }
            value = value * 16 + u16::from(digit);
            pos += 1;
        }
        if digits == 0 {
            return Ok(false);
        }
        groups[count] = value;
        count += 1;
        if pos == bytes.len() {
            break;
        }
        pos += 1; // separator already inspected
        if pos == bytes.len() {
            return Ok(false);
        }
        b.charge(1)?;
        if bytes[pos] == b':' {
            if gap.is_some() {
                return Ok(false);
            }
            gap = Some(count);
            pos += 1;
        }
    }
    if let Some(index) = gap {
        if count >= 8 {
            return Ok(false);
        }
        let missing = 8 - count;
        for at in (index..count).rev() {
            groups[at + missing] = groups[at];
        }
        groups[index..index + missing].fill(0);
    } else if count != 8 {
        return Ok(false);
    }
    let zero_prefix = groups[..7].iter().all(|group| *group == 0);
    Ok(!(zero_prefix && groups[7] <= 1))
}
