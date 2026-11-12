//! Process hardening at start (security spec, section 7): core dump
//! limit 0, non-dumpable on Linux.

pub fn harden() {
    let _ = nix::sys::resource::setrlimit(nix::sys::resource::Resource::RLIMIT_CORE, 0, 0);
    #[cfg(target_os = "linux")]
    {
        let _ = nix::sys::prctl::set_dumpable(false);
    }
}
