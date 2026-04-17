# clonefile

Thin Rust wrapper around macOS `clonefile(2)`, `clonefileat(2)`, and `fclonefileat(2)`. APFS copy-on-write: cloning a multi-gigabyte tree takes milliseconds and zero extra disk until something writes.

```rust
use clonefile::{clone, Flags};

clone("/path/to/repo", "/tmp/agent-sandbox-42", Flags::empty())?;
```

## Why this crate exists

The main use case is **per-agent sandbox environments**: spin up a fresh CoW clone of a working tree for each agent to scribble in, throw it away when the task is done. On APFS this is basically free — the syscall returns in milliseconds and the only disk cost is divergence from the original.

`std::fs::copy` and `cp -R` traverse and re-read every byte. `clonefile` tells the filesystem to share blocks until one side mutates them.

## Caveats — read `man 2 clonefile` before using in anger

The man page calls out several sharp edges. For short-lived agent sandboxes they're mostly a non-issue, but you should know they exist:

- **Apple discourages cloning directories.** Verbatim: *"the use of clonefile(2) to clone directory hierarchies is strongly discouraged. Use copyfile(3) instead."* They don't elaborate on why, and for ephemeral sandboxes (clone → run agent → delete) this is fine in practice. If you're cloning directories into long-lived production storage, prefer `copyfile(3)` with `COPYFILE_CLONE`.
- **APFS only.** Returns `ENOTSUP` on other filesystems, `EXDEV` if src and dst are on different volumes.
- **Destination must not exist** — `EEXIST` otherwise. No atomic replace.
- **Deferred allocation means deferred failure.** Because blocks are shared, a later overwrite can return `ENOSPC` even though the clone itself succeeded.
- **`setuid`/`setgid` stripped** from regular files. Ownership resets unless the caller is privileged or `CLONE_NOOWNERCOPY` is passed.
- **Not POSIX.** Darwin only; this crate is gated on `target_os = "macos"`.

## API

```rust
pub fn clone<P, Q>(src: P, dst: Q, flags: Flags) -> io::Result<()>;
pub fn clone_at<P, Q>(src_dirfd: Option<RawFd>, src: P, dst_dirfd: Option<RawFd>, dst: Q, flags: Flags) -> io::Result<()>;
pub fn fclone_at<F: AsRawFd, Q>(src: &F, dst_dirfd: Option<RawFd>, dst: Q, flags: Flags) -> io::Result<()>;
```

Flags mirror `<sys/clonefile.h>`: `NOFOLLOW`, `NO_OWNER_COPY`, `ACL`, `NOFOLLOW_ANY`, `RESOLVE_BENEATH`.

## License

MIT OR Apache-2.0
