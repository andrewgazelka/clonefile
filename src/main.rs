//! `clonefile` CLI. Thin shell over the library.
//!
//! Usage: clonefile [--nofollow] [--no-owner-copy] [--acl] SRC DST

use std::path::PathBuf;
use std::process::ExitCode;

use clonefile::{Flags, clone};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let mut flags = Flags::empty();
    let mut positional: Vec<PathBuf> = Vec::new();

    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => {
                print_help();
                return ExitCode::SUCCESS;
            }
            Some("--nofollow") => flags |= Flags::NOFOLLOW,
            Some("--no-owner-copy") => flags |= Flags::NO_OWNER_COPY,
            Some("--acl") => flags |= Flags::ACL,
            Some("--nofollow-any") => flags |= Flags::NOFOLLOW_ANY,
            Some("--resolve-beneath") => flags |= Flags::RESOLVE_BENEATH,
            Some(s) if s.starts_with("--") => {
                eprintln!("clonefile: unknown flag: {s}");
                return ExitCode::from(2);
            }
            _ => positional.push(PathBuf::from(arg)),
        }
    }

    let [src, dst] = match positional.as_slice() {
        [a, b] => [a.clone(), b.clone()],
        _ => {
            print_help();
            return ExitCode::from(2);
        }
    };

    match clone(&src, &dst, flags) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("clonefile: {}: {e}", src.display());
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    eprintln!(
        "clonefile — APFS copy-on-write clone\n\n\
         Usage: clonefile [FLAGS] SRC DST\n\n\
         Flags:\n  \
           --nofollow         do not follow symlink at SRC\n  \
           --no-owner-copy    do not copy ownership from SRC\n  \
           --acl              copy ACLs from SRC\n  \
           --nofollow-any     error on any symlink during resolution\n  \
           --resolve-beneath  require SRC and DST beneath CWD\n  \
           -h, --help         show this help"
    );
}
