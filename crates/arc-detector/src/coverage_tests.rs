use crate::{analyze, ClassifyRequest, JobClass, Reason, Uncertainty};

#[test]
fn unquoted_control_words_cannot_be_mistaken_for_executable_names() {
    for command in ["if cargo build", "! cargo test", "while cargo build"] {
        let input = ClassifyRequest {
            command: command.into(),
            shell_kind: "sh".into(),
        };
        let result = analyze(&input).0;
        assert_eq!(result.reason, Reason::UnsupportedSyntax);
        assert_eq!(result.classification, JobClass::Unknown);
        assert!(result.is_consistent());
    }
}

#[test]
fn empty_nested_shell_retains_unknown_in_compound_commands() {
    for command in ["sh -c ''", "cargo build && bash -lc ''", "zsh -c '  \n  '"] {
        let input = ClassifyRequest {
            command: command.into(),
            shell_kind: "sh".into(),
        };
        let (result, stats) = analyze(&input);
        assert_eq!(result.classification, JobClass::Unknown);
        assert_eq!(result.reason, Reason::EmptyCommand);
        assert_eq!(result.uncertainty, Uncertainty::Observe);
        assert!(result.is_consistent());
        assert_eq!(analyze(&input), (result, stats));
    }
}

#[test]
fn ssh_ambiguous_destination_and_option_boundaries_stay_conservative() {
    for command in [
        "ssh -v -- host make",
        "ssh @host make",
        "ssh 'bad!user@host' make",
        "ssh 'user@' make",
        "ssh '[2001:db8::1' make",
        "ssh '[1:2:3:4:5:6:7::8]' make",
    ] {
        let input = ClassifyRequest {
            command: command.into(),
            shell_kind: "sh".into(),
        };
        let (result, stats) = analyze(&input);
        assert_eq!(result.classification, JobClass::Unknown, "{command}");
        assert_eq!(result.reason, Reason::SshAmbiguous, "{command}");
        assert!(result.is_consistent());
        assert_eq!(analyze(&input), (result, stats));
    }
}
